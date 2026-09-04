//! Request/response bodies. Domain models (`thaumiel_core::models::*`) are
//! returned directly as JSON where they already are the right shape; these
//! types exist for requests (which omit server-generated fields) and for the
//! handful of responses that intentionally differ from storage (e.g. a
//! freshly generated API key includes its one-time plaintext secret).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use thaumiel_core::ids::ProductId;
use thaumiel_core::models::{ApiKey, ApiKeyScope};
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

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub identity: Identity,
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
pub struct KeygenBackendInfo {
    pub id: &'static str,
    pub description: &'static str,
    pub offline_verifiable: bool,
}
