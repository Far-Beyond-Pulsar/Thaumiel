//! Domain entities shared by every storage backend, keygen backend, and the HTTP layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ids::{ActivationId, ApiKeyId, AuditLogId, LicenseId, OrganizationId, ProductId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub org_id: OrganizationId,
    pub name: String,
    /// Which registered `KeygenBackend::id()` this product issues keys with by default.
    pub default_keygen_backend: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Active,
    Suspended,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    pub id: LicenseId,
    pub org_id: OrganizationId,
    pub product_id: ProductId,
    /// Which `KeygenBackend::id()` produced (and must be used to validate) this key.
    pub backend_id: String,
    /// The user-facing key string. For DB-lookup backends this is stored directly;
    /// for offline-verifiable backends it's still stored to allow revocation/lookup,
    /// but validation does not strictly require the round-trip.
    pub key: String,
    pub status: LicenseStatus,
    pub seats: u32,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl LicenseKey {
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        if self.status != LicenseStatus::Active {
            return false;
        }
        match self.expires_at {
            Some(exp) => now < exp,
            None => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activation {
    pub id: ActivationId,
    pub license_id: LicenseId,
    pub machine_fingerprint: String,
    pub activated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyScope {
    /// Full administrative access (organizations/products/licenses/api-keys/audit-log).
    Admin,
    /// Can only call license generate/validate/revoke for its own organization.
    LicenseManager,
    /// Can only call license validate (typical embedded-in-a-shipped-app scope).
    ValidateOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: ApiKeyId,
    pub org_id: OrganizationId,
    pub name: String,
    /// SHA-256 hex digest of the secret; the plaintext key is only ever shown once,
    /// at creation time, and is never stored.
    pub key_hash: String,
    /// Short, non-secret prefix (e.g. `thm_live_ab12`) stored alongside the hash so
    /// keys can be identified/revoked in a UI without re-deriving the hash.
    pub key_prefix: String,
    pub scope: ApiKeyScope,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub org_id: OrganizationId,
    pub email: String,
    /// Argon2id PHC string (`$argon2id$v=19$...`). Never `None` for internal-auth
    /// users; reserved as `Option` so SSO-only users (future work) can omit it.
    pub password_hash: Option<String>,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: AuditLogId,
    pub org_id: OrganizationId,
    /// Free-form actor identity: `user:<id>`, `api_key:<id>`, or `system`.
    pub actor: String,
    pub action: String,
    pub target: String,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}
