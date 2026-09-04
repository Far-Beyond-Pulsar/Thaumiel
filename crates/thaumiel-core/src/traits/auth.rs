use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ids::{OrganizationId, UserId};
use crate::models::Role;

/// Credentials presented at the `/v1/auth/login` boundary. `Password` is what
/// `thaumiel-auth`'s internal provider handles today; the other variants exist
/// so future providers (OIDC, SAML, LDAP — see `docs/ARCHITECTURE.md`) can plug
/// into the same trait without changing the route handler's shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credentials {
    Password {
        org_id: OrganizationId,
        email: String,
        password: String,
    },
    OidcToken {
        id_token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: UserId,
    pub org_id: OrganizationId,
    pub email: String,
    pub role: Role,
}

/// A pluggable authentication method for the admin/dashboard login flow.
///
/// Implementations self-register via [`crate::register_auth_provider!`].
/// `thaumiel-auth` ships `InternalAuthProvider` (Argon2id password hashing +
/// JWT session issuance); it is the only one built today. API-key auth for
/// machine endpoints is handled separately (see `thaumiel-auth::apikey`) since
/// it doesn't fit the login/`Credentials` shape.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Stable identifier, e.g. `"internal"`, `"oidc"`.
    fn id(&self) -> &'static str;

    async fn authenticate(&self, credentials: Credentials) -> Result<Identity>;
}
