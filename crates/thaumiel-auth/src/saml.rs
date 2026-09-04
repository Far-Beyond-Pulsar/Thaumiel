//! SAML 2.0 Service Provider (issue #2). Unlike the other three providers,
//! this one doesn't implement `AuthProvider`/register through the generic
//! plugin registry -- SAML's redirect-based flow needs three dedicated
//! routes (metadata, login-start, ACS callback) that don't fit the
//! login/`Credentials` shape at all, so `thaumiel-server` holds an
//! `Arc<SamlAuthProvider>` directly (see `AppState::saml`) and calls its
//! inherent methods from those routes instead.
//!
//! Real XML-DSig signature verification, via `samael`'s `xmlsec` feature
//! (a binding to the xmlsec1 C library) -- not a stub. Building this
//! requires `libxml2`/`xmlsec1`/`libxslt`/`libclang`/`pkg-config` as system
//! libraries; see `docs/CONFIGURATION.md` and this crate's README for what
//! to install. That native dependency is exactly why this took longer than
//! LDAP/OIDC to land -- see the issue thread for the full reasoning.
//!
//! **Known simplification**: SP-initiated login doesn't track pending
//! request IDs, so `parse_base64_response` is called with
//! `possible_request_ids: None` -- meaning replay of an old, still-valid
//! assertion isn't rejected by `InResponseTo` matching. `SubjectConfirmationData`'s
//! own `NotOnOrAfter`/`Recipient` checks (enforced by `samael` regardless)
//! still bound how long and where a captured assertion is usable, but a
//! dedicated request-id store (e.g. in `Cache`, matching how rate limiting
//! already uses it) would close this gap further -- worth a follow-up issue
//! rather than blocking this one on it.

use std::sync::Arc;

use chrono::Utc;
use samael::metadata::{de as saml_de, EntityDescriptor, HTTP_REDIRECT_BINDING};
use samael::schema::Assertion;
use samael::service_provider::{ServiceProvider, ServiceProviderBuilder};
use samael::traits::ToXml;
use tokio::sync::OnceCell;

use thaumiel_core::ids::{OrganizationId, UserId};
use thaumiel_core::models::{Role, User};
use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{Identity, Storage};
use thaumiel_core::{Result, ThaumielError};

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

pub struct SamlAuthProvider {
    storage: Arc<dyn Storage>,
    idp_metadata_url: Option<String>,
    idp_metadata_path: Option<String>,
    sp_entity_id: Option<String>,
    acs_url: Option<String>,
    /// Building a `ServiceProvider` means fetching/reading the IdP's
    /// metadata (a network call or disk read) -- same lazy-init-from-a-sync-
    /// constructor reasoning as `OidcAuthProvider::client`, but simpler here
    /// since `ServiceProvider` isn't a generic typestate type.
    sp: OnceCell<ServiceProvider>,
}

impl SamlAuthProvider {
    pub fn new(ctx: &PluginContext) -> Self {
        let idp_metadata_url = env_var("THAUMIEL_SAML_IDP_METADATA_URL");
        let idp_metadata_path = env_var("THAUMIEL_SAML_IDP_METADATA_PATH");
        let sp_entity_id = env_var("THAUMIEL_SAML_SP_ENTITY_ID");
        let acs_url = env_var("THAUMIEL_SAML_ACS_URL");

        if (idp_metadata_url.is_none() && idp_metadata_path.is_none()) || sp_entity_id.is_none() || acs_url.is_none()
        {
            tracing::warn!(
                "SAML is not fully configured -- need THAUMIEL_SAML_SP_ENTITY_ID, \
                 THAUMIEL_SAML_ACS_URL, and either THAUMIEL_SAML_IDP_METADATA_URL or \
                 THAUMIEL_SAML_IDP_METADATA_PATH. /v1/auth/login/saml/* will fail until all are \
                 set. See docs/CONFIGURATION.md."
            );
        }

        Self { storage: ctx.storage.clone(), idp_metadata_url, idp_metadata_path, sp_entity_id, acs_url, sp: OnceCell::new() }
    }

    async fn fetch_idp_metadata(&self) -> Result<EntityDescriptor> {
        let xml = if let Some(url) = &self.idp_metadata_url {
            reqwest::get(url)
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| ThaumielError::Internal(format!("failed to fetch SAML IdP metadata from {url}: {e}")))?
                .text()
                .await
                .map_err(|e| ThaumielError::Internal(format!("failed to read SAML IdP metadata response: {e}")))?
        } else if let Some(path) = &self.idp_metadata_path {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ThaumielError::Config(format!("failed to read THAUMIEL_SAML_IDP_METADATA_PATH ({path}): {e}")))?
        } else {
            return Err(ThaumielError::Config(
                "SAML is not configured: set THAUMIEL_SAML_IDP_METADATA_URL or _PATH".into(),
            ));
        };

        saml_de::from_str(&xml).map_err(|e| ThaumielError::Config(format!("failed to parse SAML IdP metadata XML: {e}")))
    }

    async fn service_provider(&self) -> Result<&ServiceProvider> {
        self.sp
            .get_or_try_init(|| async {
                let idp_metadata = self.fetch_idp_metadata().await?;
                let entity_id = self
                    .sp_entity_id
                    .clone()
                    .ok_or_else(|| ThaumielError::Config("THAUMIEL_SAML_SP_ENTITY_ID not set".into()))?;
                let acs_url = self
                    .acs_url
                    .clone()
                    .ok_or_else(|| ThaumielError::Config("THAUMIEL_SAML_ACS_URL not set".into()))?;

                ServiceProviderBuilder::default()
                    .entity_id(entity_id)
                    .acs_url(acs_url)
                    // We don't sign our own AuthnRequests (no SP key/cert
                    // configured) -- acceptable for redirect-binding requests
                    // over HTTPS; what actually matters for us as a relying
                    // party is verifying the IdP's *response* signature,
                    // which `samael` enforces unconditionally from
                    // `idp_metadata`'s embedded signing certs, not something
                    // this omission weakens.
                    .allow_idp_initiated(true)
                    .idp_metadata(idp_metadata)
                    .build()
                    .map_err(|e| ThaumielError::Internal(format!("failed to build SAML service provider: {e}")))
            })
            .await
    }

    /// This server's own SP metadata XML, for `GET /v1/auth/login/saml/metadata` --
    /// what an IdP administrator uploads/points at to configure the other
    /// side of the trust relationship.
    pub async fn metadata_xml(&self) -> Result<String> {
        let sp = self.service_provider().await?;
        let metadata =
            sp.metadata().map_err(|e| ThaumielError::Internal(format!("failed to build SP metadata: {e}")))?;
        metadata.to_string().map_err(|e| ThaumielError::Internal(format!("failed to serialize SP metadata: {e}")))
    }

    /// Where to redirect the browser to start an SP-initiated login for
    /// `org_id`, carried through the IdP round-trip as SAML's `RelayState`
    /// (exactly what that mechanism is for) so the ACS callback knows which
    /// org this login belongs to.
    pub async fn login_redirect_url(&self, org_id: OrganizationId) -> Result<String> {
        let sp = self.service_provider().await?;
        let idp_sso_url = sp
            .sso_binding_location(HTTP_REDIRECT_BINDING)
            .ok_or_else(|| ThaumielError::Config("IdP metadata has no HTTP-Redirect SSO binding".into()))?;
        let authn_request = sp
            .make_authentication_request(&idp_sso_url)
            .map_err(|e| ThaumielError::Internal(format!("failed to build SAML AuthnRequest: {e}")))?;
        let url = authn_request
            .redirect(&org_id.to_string())
            .map_err(|e| ThaumielError::Internal(format!("failed to build SAML redirect URL: {e}")))?
            .ok_or_else(|| ThaumielError::Internal("IdP metadata has no usable SSO destination".into()))?;
        Ok(url.to_string())
    }

    /// Verifies (real XML-DSig, via `samael`'s `xmlsec` feature) and
    /// processes an ACS callback: `saml_response_b64` is the raw
    /// `SAMLResponse` form field, `relay_state` the `RelayState` field
    /// carrying the org id `login_redirect_url` embedded.
    pub async fn handle_acs(&self, saml_response_b64: &str, relay_state: &str) -> Result<Identity> {
        let invalid = || ThaumielError::Unauthenticated("invalid SAML response".into());

        let org_id: OrganizationId =
            relay_state.parse().map_err(|_| ThaumielError::InvalidInput("invalid or missing RelayState".into()))?;

        let sp = self.service_provider().await?;
        let assertion = sp.parse_base64_response(saml_response_b64, None).map_err(|e| {
            tracing::warn!(error = %e, "SAML response verification failed");
            invalid()
        })?;

        let email = extract_email(&assertion).ok_or_else(invalid)?;

        let user = match self.storage.get_user_by_email(org_id, &email).await {
            Ok(user) => user,
            Err(_) => {
                let user = User {
                    id: UserId::new(),
                    org_id,
                    email: email.clone(),
                    password_hash: None,
                    role: Role::Member,
                    created_at: Utc::now(),
                };
                self.storage.create_user(user).await?
            }
        };

        Ok(Identity { user_id: user.id, org_id: user.org_id, email: user.email, role: user.role })
    }
}

/// Prefers an explicit email-shaped attribute (several common
/// naming conventions across IdPs); falls back to `NameID`, which is
/// commonly the email address itself when the IdP is configured with
/// `EmailAddressNameIDFormat` (a very common setup).
fn extract_email(assertion: &Assertion) -> Option<String> {
    const EMAIL_ATTRIBUTE_NAMES: &[&str] = &[
        "email",
        "mail",
        "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
        "urn:oid:0.9.2342.19200300.100.1.3",
    ];

    let from_attributes = assertion.attribute_statements.as_ref().and_then(|statements| {
        statements
            .iter()
            .flat_map(|s| s.attributes.iter())
            .find(|a| {
                a.name.as_deref().is_some_and(|n| EMAIL_ATTRIBUTE_NAMES.contains(&n))
                    || a.friendly_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case("email"))
            })
            .and_then(|a| a.values.first())
            .and_then(|v| v.value.clone())
    });

    from_attributes.or_else(|| assertion.subject.as_ref()?.name_id.as_ref().map(|n| n.value.clone()))
}
