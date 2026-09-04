use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::Result;
use crate::ids::{OrganizationId, ProductId};

#[derive(Debug, Clone)]
pub struct GenerateRequest {
    pub org_id: OrganizationId,
    pub product_id: ProductId,
    pub seats: u32,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedKey {
    /// The user-facing license key string, in whatever format this backend uses.
    pub key: String,
    /// Anything backend-specific worth persisting alongside the license row
    /// (e.g. the signing key id used, for later rotation support).
    pub backend_metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ValidateContext {
    pub org_id: OrganizationId,
    pub product_id: ProductId,
    /// Backend-specific metadata captured at generation time (see
    /// `GeneratedKey::backend_metadata`), handed back for backends that need
    /// it to validate offline (e.g. which signing key to check against).
    pub backend_metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Valid,
    Invalid { reason: String },
}

/// A pluggable license-key format/algorithm.
///
/// Implementations self-register via [`crate::register_keygen_backend!`] so the
/// server discovers every linked-in backend at startup without a central list
/// (see `docs/PLUGINS.md`). Three ship in `thaumiel-keygen`: an Ed25519-signed
/// offline-verifiable format, an HMAC-checksummed human-typable format, and a
/// simple opaque random token.
#[async_trait]
pub trait KeygenBackend: Send + Sync {
    /// Stable identifier stored on `LicenseKey::backend_id` and referenced by
    /// `Product::default_keygen_backend` / config. Must never change once used
    /// in production data.
    fn id(&self) -> &'static str;

    /// Human-readable description, surfaced by the `/v1/keygen-backends` route.
    fn description(&self) -> &'static str;

    /// Whether `validate` can succeed without a storage lookup (purely
    /// cryptographic/offline verification). Informational only today; the
    /// server always double-checks status/expiry/revocation against storage
    /// regardless of this flag.
    fn offline_verifiable(&self) -> bool;

    async fn generate(&self, req: &GenerateRequest) -> Result<GeneratedKey>;
    async fn validate(&self, key: &str, ctx: &ValidateContext) -> Result<Validation>;
}
