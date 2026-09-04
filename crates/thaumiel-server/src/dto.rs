//! Request/response bodies. Domain models (`thaumiel_core::models::*`) are
//! returned directly as JSON where they already are the right shape; these
//! types exist for requests (which omit server-generated fields) and for the
//! handful of responses that intentionally differ from storage (e.g. a
//! freshly generated API key includes its one-time plaintext secret).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use thaumiel_core::ids::ProductId;
use thaumiel_core::models::{ApiKey, ApiKeyScope, Role};
use thaumiel_core::traits::Identity;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub org_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub org_id: thaumiel_core::ids::OrganizationId,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct OidcLoginRequest {
    pub org_id: thaumiel_core::ids::OrganizationId,
    /// Already obtained by the caller talking to the identity provider
    /// directly -- see `thaumiel_auth::oidc`'s module doc comment.
    pub id_token: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub identity: Identity,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    /// A temporary password the admin sets directly and shares with the new
    /// user out of band -- there's no invite-email flow yet (issue #8 in the
    /// repo's tracker covers that as a possible follow-up).
    pub password: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub name: String,
    /// Must be a registered `KeygenBackend::id()`; defaults to the server's
    /// configured `keygen.default_backend` when omitted.
    pub default_keygen_backend: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateLicenseRequest {
    pub product_id: ProductId,
    #[serde(default = "default_seats")]
    pub seats: u32,
    pub expires_at: Option<DateTime<Utc>>,
    /// Overrides the product's `default_keygen_backend` for this one key.
    pub backend_id: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_seats() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
pub struct ValidateLicenseRequest {
    pub key: String,
    pub product_id: ProductId,
    /// If present, this call also counts as (or repeats) a seat activation.
    pub machine_fingerprint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ValidateLicenseResponse {
    pub valid: bool,
    pub reason: Option<String>,
    pub seats_total: Option<u32>,
    pub seats_used: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scope: ApiKeyScope,
    /// `"live"` or `"test"`; becomes part of the key's visible prefix.
    #[serde(default = "default_env_tag")]
    pub env_tag: String,
}

fn default_env_tag() -> String {
    "live".into()
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    /// Shown exactly once -- the server never stores or re-displays this.
    pub plaintext: String,
    #[serde(flatten)]
    pub record: ApiKey,
}

#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub products: u32,
    pub licenses_total: u32,
    pub licenses_active: u32,
    pub api_keys_active: u32,
    /// Exact only up to 200 (the max page size -- see docs/CONFIGURATION.md);
    /// beyond that this undercounts. Use the paginated list endpoints
    /// (`?limit=&offset=`) for an exact total on a larger organization.
    pub counts_capped_at: u32,
    pub validate_calls_last_14_days: Vec<crate::usage::UsageDayCount>,
}

#[derive(Debug, Serialize)]
pub struct KeygenBackendInfo {
    pub id: &'static str,
    pub description: &'static str,
    pub offline_verifiable: bool,
}
