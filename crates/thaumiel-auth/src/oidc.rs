//! OIDC `AuthProvider` (issue #1). Unlike `InternalAuthProvider`/`LdapAuthProvider`,
//! this one doesn't do a login *flow* itself -- it verifies an `id_token` the
//! caller already obtained by talking to the identity provider directly
//! (e.g. a browser-side OIDC redirect in `thaumiel-ui`, or a CLI that already
//! did `az login`/`gcloud auth login`-style token acquisition). That's why
//! `POST /v1/auth/login/oidc` (see `routes/auth.rs`) is a separate route from
//! `/v1/auth/login` rather than the same endpoint dispatching on
//! `auth.provider`: a deployment can offer password login *and* OIDC login
//! side by side, not just one or the other.
//!
//! Provisioning: on first successful verification, a local `User` row is
//! JIT-provisioned (role `member`) if one doesn't already exist for the
//! token's email within the org -- same policy as `LdapAuthProvider`, for
//! the same reason (see its module doc comment).

use async_trait::async_trait;
use openidconnect::core::{CoreClient, CoreIdToken, CoreProviderMetadata};
use openidconnect::{ClientId, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, Nonce};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// What `CoreClient::from_provider_metadata` actually returns: discovery
/// always provides an authorization endpoint (`EndpointSet`) and may or may
/// not provide token/userinfo endpoints (`EndpointMaybeSet`); we use neither
/// device-auth, introspection, nor revocation endpoints at all
/// (`EndpointNotSet`). This has to be named exactly -- plain `CoreClient`
/// (all six default to `EndpointNotSet`) is a *different, incompatible*
/// concrete type from what `from_provider_metadata` hands back.
type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

use thaumiel_core::ids::UserId;
use thaumiel_core::models::{Role, User};
use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{AuthProvider, Credentials, Identity, Storage};
use thaumiel_core::{Result, ThaumielError};

pub struct OidcAuthProvider {
    storage: Arc<dyn Storage>,
    issuer_url: String,
    client_id: String,
    http_client: reqwest::Client,
    /// Discovery (fetching `.well-known/openid-configuration` + JWKS) is an
    /// async network call, but `AuthProvider` constructors are sync (see
    /// `thaumiel_core::registry`) -- so it happens lazily, once, on first
    /// use, rather than at startup. Caching the discovery *metadata* here
    /// (rather than a built `CoreClient`) sidesteps openidconnect 4.x's
    /// typestate-tracked endpoint generics on `Client<...>` -- naming that
    /// type for a struct field is more trouble than it's worth when
    /// rebuilding a `CoreClient` from already-fetched metadata is free (no
    /// network call, just wrapping data we already have).
    metadata: OnceCell<CoreProviderMetadata>,
}

impl OidcAuthProvider {
    pub fn new(ctx: &PluginContext) -> Self {
        let issuer_url = std::env::var("THAUMIEL_OIDC_ISSUER_URL").unwrap_or_default();
        let client_id = std::env::var("THAUMIEL_OIDC_CLIENT_ID").unwrap_or_default();
        if issuer_url.is_empty() || client_id.is_empty() {
            tracing::warn!(
                "THAUMIEL_OIDC_ISSUER_URL / THAUMIEL_OIDC_CLIENT_ID not set -- the 'oidc' auth \
                 provider is registered but every login through it will fail until both are set. \
                 See docs/CONFIGURATION.md."
            );
        }
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none()) // discovery/JWKS fetches should never follow redirects
            .build()
            .expect("building the OIDC discovery HTTP client should never fail");

        Self {
            storage: ctx.storage.clone(),
            issuer_url,
            client_id,
            http_client,
            metadata: OnceCell::new(),
        }
    }

    /// Discovers (once, cached thereafter) and wraps the provider metadata
    /// into a fresh `CoreClient` -- cheap; see the `metadata` field's doc
    /// comment for why the client itself isn't what's cached.
    async fn client(&self) -> Result<DiscoveredClient> {
        let metadata = self
            .metadata
            .get_or_try_init(|| async {
                let issuer = IssuerUrl::new(self.issuer_url.clone()).map_err(|e| {
                    ThaumielError::Config(format!("invalid THAUMIEL_OIDC_ISSUER_URL: {e}"))
                })?;
                CoreProviderMetadata::discover_async(issuer, &self.http_client)
                    .await
                    .map_err(|e| {
                        ThaumielError::Internal(format!(
                            "oidc discovery against {} failed: {e}",
                            self.issuer_url
                        ))
                    })
            })
            .await?;
        Ok(CoreClient::from_provider_metadata(
            metadata.clone(),
            ClientId::new(self.client_id.clone()),
            None,
        ))
    }
}

#[async_trait]
impl AuthProvider for OidcAuthProvider {
    fn id(&self) -> &'static str {
        "oidc"
    }

    async fn authenticate(&self, credentials: Credentials) -> Result<Identity> {
        let Credentials::OidcToken { org_id, id_token } = credentials else {
            return Err(ThaumielError::InvalidInput(
                "the 'oidc' auth provider only supports OIDC tokens".into(),
            ));
        };
        let invalid = || ThaumielError::Unauthenticated("invalid OIDC token".into());

        let client: DiscoveredClient = self.client().await.map_err(|e| {
            tracing::error!(error = %e, "oidc provider unavailable");
            invalid()
        })?;

        let token: CoreIdToken = id_token.parse().map_err(|_| invalid())?;
        let verifier = client.id_token_verifier();
        // We didn't initiate this token's original auth request (the caller
        // obtained it directly from the IdP), so there's no nonce of our own
        // to check it against -- signature/issuer/audience/expiry are still
        // fully verified regardless.
        let claims = token
            .claims(&verifier, |_nonce: Option<&Nonce>| Ok(()))
            .map_err(|e| {
                tracing::warn!(error = %e, "oidc id_token verification failed");
                invalid()
            })?;

        let email = claims
            .email()
            .map(|e| e.as_str().to_string())
            .ok_or_else(invalid)?;

        let user = match self.storage.get_user_by_email(org_id, &email).await {
            Ok(user) => user,
            Err(_) => {
                let user = User {
                    id: UserId::new(),
                    org_id,
                    email: email.clone(),
                    password_hash: None,
                    role: Role::Member,
                    created_at: chrono::Utc::now(),
                };
                self.storage.create_user(user).await?
            }
        };

        Ok(Identity {
            user_id: user.id,
            org_id: user.org_id,
            email: user.email,
            role: user.role,
        })
    }
}

thaumiel_core::register_auth_provider!(OidcAuthProvider::new);
