//! LDAP/Active Directory `AuthProvider` (issue #3). Reuses
//! `Credentials::Password` as-is -- an LDAP bind is fundamentally
//! username+password against a different backend than Argon2id, so no new
//! `Credentials` variant or route changes were needed, unlike OIDC/SAML.
//!
//! Uses the standard "search then bind" pattern: bind as a configured
//! service account, search for the entry matching the presented email, then
//! open a *second* connection and bind as that entry's own DN with the
//! presented password -- that second bind is the actual credential check.
//! A service-account-only bind (skipping the second step) would only prove
//! the service account's credentials are valid, not the end user's.

use async_trait::async_trait;
use ldap3::{LdapConnAsync, Scope, SearchEntry};
use std::sync::Arc;

use thaumiel_core::ids::UserId;
use thaumiel_core::models::{Role, User};
use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{AuthProvider, Credentials, Identity, Storage};
use thaumiel_core::{Result, ThaumielError};

pub struct LdapAuthProvider {
    storage: Arc<dyn Storage>,
    url: String,
    /// DN of the service account used for the search phase, e.g.
    /// `cn=readonly,dc=example,dc=com`.
    bind_dn: String,
    bind_password: String,
    /// Where to search for user entries, e.g. `ou=people,dc=example,dc=com`.
    base_dn: String,
    /// `{email}` is replaced with the (escaped) presented email. Default
    /// matches a common `mail` attribute convention; Active Directory
    /// deployments typically want `(userPrincipalName={email})` instead.
    user_filter: String,
}

fn env_var(key: &str) -> String {
    std::env::var(key).unwrap_or_default()
}

impl LdapAuthProvider {
    pub fn new(ctx: &PluginContext) -> Self {
        let url = env_var("THAUMIEL_LDAP_URL");
        if url.is_empty() {
            tracing::warn!(
                "THAUMIEL_LDAP_URL not set -- the 'ldap' auth provider is registered but every \
                 login through it will fail until THAUMIEL_LDAP_URL, _BIND_DN, _BIND_PASSWORD, \
                 and _BASE_DN are set. See docs/CONFIGURATION.md."
            );
        }
        Self {
            storage: ctx.storage.clone(),
            url,
            bind_dn: env_var("THAUMIEL_LDAP_BIND_DN"),
            bind_password: env_var("THAUMIEL_LDAP_BIND_PASSWORD"),
            base_dn: env_var("THAUMIEL_LDAP_BASE_DN"),
            user_filter: {
                let f = env_var("THAUMIEL_LDAP_USER_FILTER");
                if f.is_empty() {
                    "(mail={email})".to_string()
                } else {
                    f
                }
            },
        }
    }

    /// One-shot connect + simple_bind, used for both the service-account
    /// search connection and the end-user credential-check connection.
    async fn bind(&self, dn: &str, password: &str) -> std::result::Result<ldap3::Ldap, ()> {
        let (conn, mut ldap) = LdapConnAsync::new(&self.url).await.map_err(|e| {
            tracing::error!(error = %e, url = %self.url, "ldap connection failed");
        })?;
        ldap3::drive!(conn);
        ldap.simple_bind(dn, password)
            .await
            .map_err(|_| ())?
            .success()
            .map_err(|_| ())?;
        Ok(ldap)
    }
}

#[async_trait]
impl AuthProvider for LdapAuthProvider {
    fn id(&self) -> &'static str {
        "ldap"
    }

    async fn authenticate(&self, credentials: Credentials) -> Result<Identity> {
        let Credentials::Password {
            org_id,
            email,
            password,
        } = credentials
        else {
            return Err(ThaumielError::InvalidInput(
                "the 'ldap' auth provider only supports password credentials".into(),
            ));
        };
        let invalid = || ThaumielError::Unauthenticated("invalid email or password".into());

        // 1. Service-account bind, then search for the user's own DN.
        let mut search_conn = self
            .bind(&self.bind_dn, &self.bind_password)
            .await
            .map_err(|_| invalid())?;
        let filter = self
            .user_filter
            .replace("{email}", &ldap3::ldap_escape(&email));
        let (entries, _) = search_conn
            .search(&self.base_dn, Scope::Subtree, &filter, vec!["dn"])
            .await
            .map_err(|_| invalid())?
            .success()
            .map_err(|_| invalid())?;
        let _ = search_conn.unbind().await;
        let entry = entries.into_iter().next().ok_or_else(invalid)?;
        let user_dn = SearchEntry::construct(entry).dn;

        // 2. The real credential check: bind as the user's own DN.
        let mut user_conn = self
            .bind(&user_dn, &password)
            .await
            .map_err(|_| invalid())?;
        let _ = user_conn.unbind().await;

        // 3. Just-in-time provision a local User row on first successful
        // login -- see docs/ARCHITECTURE.md's roadmap entry on this. New
        // users default to `Member`; promote them via `PATCH`-style admin
        // tooling once that exists (issue #8 covers user management).
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

thaumiel_core::register_auth_provider!(LdapAuthProvider::new);

#[cfg(test)]
mod tests {
    // No real LDAP server is available to test the actual bind/search flow
    // against (same limitation noted on MssqlStorage) -- this covers the one
    // piece that's meaningfully testable in isolation: filter templating.
    #[test]
    fn user_filter_template_substitutes_escaped_email() {
        let filter = "(mail={email})".replace("{email}", &ldap3::ldap_escape("a*b(c)@example.com"));
        assert_eq!(filter, "(mail=a\\2ab\\28c\\29@example.com)");
    }
}
