use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;

use thaumiel_core::ids::{ActivationId, LicenseId};
use thaumiel_core::models::{Activation, LicenseKey, LicenseStatus};
use thaumiel_core::traits::{GenerateRequest, Pagination, ValidateContext, Validation};
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::{GenerateLicenseRequest, ValidateLicenseRequest, ValidateLicenseResponse};
use crate::error::ApiResult;
use crate::extractors::{AdminAuth, ApiKeyAuth};
use crate::rate_limit;
use crate::state::AppState;

/// Backend-specific data returned alongside a generated key (e.g. which
/// Ed25519 public key signed it) is stored on the same `metadata` map as
/// user-supplied metadata, under this prefix, so it survives round-trips
/// through storage without a dedicated column.
const BACKEND_METADATA_PREFIX: &str = "__backend_";

fn merge_backend_metadata(user: HashMap<String, String>, backend: HashMap<String, String>) -> HashMap<String, String> {
    let mut merged = user;
    for (k, v) in backend {
        merged.insert(format!("{BACKEND_METADATA_PREFIX}{k}"), v);
    }
    merged
}

fn extract_backend_metadata(all: &HashMap<String, String>) -> HashMap<String, String> {
    all.iter()
        .filter_map(|(k, v)| k.strip_prefix(BACKEND_METADATA_PREFIX).map(|k| (k.to_string(), v.clone())))
        .collect()
}

pub async fn generate(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Json(req): Json<GenerateLicenseRequest>,
) -> ApiResult<Json<LicenseKey>> {
    let product = state.storage.get_product(req.product_id).await?;
    if product.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("product '{}'", req.product_id)).into());
    }

    let backend_id = req.backend_id.clone().unwrap_or_else(|| product.default_keygen_backend.clone());
    let backend = state.keygen.get(&backend_id)?;

    let gen_req = GenerateRequest {
        org_id: identity.org_id,
        product_id: product.id,
        seats: req.seats,
        expires_at: req.expires_at,
        metadata: req.metadata.clone(),
    };
    let generated = backend.generate(&gen_req).await?;

    let license = LicenseKey {
        id: LicenseId::new(),
        org_id: identity.org_id,
        product_id: product.id,
        backend_id,
        key: generated.key,
        status: LicenseStatus::Active,
        seats: req.seats,
        expires_at: req.expires_at,
        metadata: merge_backend_metadata(req.metadata, generated.backend_metadata),
        created_at: Utc::now(),
        revoked_at: None,
    };
    let license = state.storage.create_license(license).await?;
    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "license.generate", format!("license:{}", license.id)).await;
    Ok(Json(license))
}

pub async fn list(State(state): State<AppState>, AdminAuth(identity): AdminAuth) -> ApiResult<Json<Vec<LicenseKey>>> {
    let licenses = state.storage.list_licenses(identity.org_id, Pagination::default()).await?;
    Ok(Json(licenses))
}

pub async fn get(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path(id): Path<LicenseId>,
) -> ApiResult<Json<LicenseKey>> {
    let license = state.storage.get_license(id).await?;
    if license.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("license '{id}'")).into());
    }
    Ok(Json(license))
}

pub async fn revoke(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path(id): Path<LicenseId>,
) -> ApiResult<Json<LicenseKey>> {
    let existing = state.storage.get_license(id).await?;
    if existing.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("license '{id}'")).into());
    }
    let license = state.storage.set_license_status(id, LicenseStatus::Revoked).await?;
    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "license.revoke", format!("license:{id}")).await;
    Ok(Json(license))
}

fn invalid(reason: impl Into<String>) -> ValidateLicenseResponse {
    ValidateLicenseResponse { valid: false, reason: Some(reason.into()), seats_total: None, seats_used: None }
}

/// The one route meant to be called from a shipped application (via
/// [`ApiKeyAuth`], not an admin session) -- so it's rate-limited per calling
/// API key on top of normal auth.
pub async fn validate(
    State(state): State<AppState>,
    ApiKeyAuth(api_key): ApiKeyAuth,
    Json(req): Json<ValidateLicenseRequest>,
) -> ApiResult<Json<ValidateLicenseResponse>> {
    rate_limit::check(state.cache.as_ref(), &format!("validate:{}", api_key.key_prefix), 120, Duration::from_secs(60)).await?;

    let Ok(license) = state.storage.get_license_by_key(&req.key).await else {
        return Ok(Json(invalid("license not found")));
    };
    if license.org_id != api_key.org_id || license.product_id != req.product_id {
        return Ok(Json(invalid("license does not match this product")));
    }
    if !license.is_usable(Utc::now()) {
        return Ok(Json(invalid(format!("license is not active (status: {:?})", license.status))));
    }

    let backend = state.keygen.get(&license.backend_id)?;
    let ctx = ValidateContext {
        org_id: license.org_id,
        product_id: license.product_id,
        backend_metadata: extract_backend_metadata(&license.metadata),
    };
    if let Validation::Invalid { reason } = backend.validate(&license.key, &ctx).await? {
        return Ok(Json(invalid(reason)));
    }

    let mut seats_used = state.storage.count_activations(license.id).await?;
    if let Some(fingerprint) = req.machine_fingerprint {
        let already_activated =
            state.storage.list_activations(license.id).await?.iter().any(|a| a.machine_fingerprint == fingerprint);
        if !already_activated {
            if seats_used >= license.seats {
                return Ok(Json(ValidateLicenseResponse {
                    valid: false,
                    reason: Some("seat limit reached".into()),
                    seats_total: Some(license.seats),
                    seats_used: Some(seats_used),
                }));
            }
            state
                .storage
                .create_activation(Activation {
                    id: ActivationId::new(),
                    license_id: license.id,
                    machine_fingerprint: fingerprint,
                    activated_at: Utc::now(),
                })
                .await?;
            seats_used += 1;
        }
    }

    Ok(Json(ValidateLicenseResponse { valid: true, reason: None, seats_total: Some(license.seats), seats_used: Some(seats_used) }))
}
