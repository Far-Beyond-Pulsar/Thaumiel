use async_trait::async_trait;
use std::sync::Arc;

use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{AuthProvider, Credentials, Identity, Storage};
use thaumiel_core::{Result, ThaumielError};

use crate::password::verify_password;

/// The built-in `AuthProvider`: looks a user up by org + email in `Storage`
/// and verifies their Argon2id password hash. Registered under the id
/// `"internal"` (see `thaumiel_config::AuthConfig::provider`, default).
pub struct InternalAuthProvider {
    storage: Arc<dyn Storage>,
}

impl InternalAuthProvider {
    pub fn new(ctx: &PluginContext) -> Self {
        Self {
            storage: ctx.storage.clone(),
        }
    }
}

#[async_trait]
impl AuthProvider for InternalAuthProvider {
    fn id(&self) -> &'static str {
        "internal"
    }

    async fn authenticate(&self, credentials: Credentials) -> Result<Identity> {
        match credentials {
            Credentials::Password {
                org_id,
                email,
                password,
            } => {
                // Same error for "no such user" and "wrong password" so the
                // login endpoint never reveals whether an email is registered.
                let invalid = || ThaumielError::Unauthenticated("invalid email or password".into());

                let user = self
                    .storage
                    .get_user_by_email(org_id, &email)
                    .await
                    .map_err(|_| invalid())?;
                let hash = user.password_hash.as_deref().ok_or_else(invalid)?;
                if !verify_password(&password, hash)? {
                    return Err(invalid());
                }
                Ok(Identity {
                    user_id: user.id,
                    org_id: user.org_id,
                    email: user.email,
                    role: user.role,
                })
            }
            Credentials::OidcToken { .. } => Err(ThaumielError::InvalidInput(
                "the 'internal' auth provider does not support OIDC tokens".into(),
            )),
        }
    }
}

thaumiel_core::register_auth_provider!(InternalAuthProvider::new);
