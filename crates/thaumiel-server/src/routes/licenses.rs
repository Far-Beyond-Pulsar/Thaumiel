use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::Utc;

use thaumiel_core::ids::{ActivationId, LicenseId};
use thaumiel_core::models::{Activation, LicenseKey, LicenseStatus};
use thaumiel_core::traits::{GenerateRequest, ValidateContext, Validation};
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::{GenerateLicenseRequest, ValidateLicenseRequest, ValidateLicenseResponse};
use crate::error::ApiResult;
use crate::extractors::{AdminAuth, ApiKeyAuth, LicenseManagerAuth};
use crate::pagination::PageQuery;
use crate::rate_limit;
use crate::state::AppState;

/// Backend-specific data returned alongside a generated key (e.g. which
/// Ed25519 public key signed it) is stored on the same `metadata` map as
/// user-supplied metadata, under this prefix, so it survives round-trips
/// through storage without a dedicated column.
const BACKEND_METADATA_PREFIX: &str = "__backend_";

fn merge_backend_metadata(
    user: HashMap<String, String>,
    backend: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = user;
    for (k, v) in backend {
        merged.insert(format!("{BACKEND_METADATA_PREFIX}{k}"), v);
    }
    merged
}

fn extract_backend_metadata(all: &HashMap<String, String>) -> HashMap<String, String> {
    all.iter()
        .filter_map(|(k, v)| {
            k.strip_prefix(BACKEND_METADATA_PREFIX)
                .map(|k| (k.to_string(), v.clone()))
        })
        .collect()
}

pub async fn generate(
    State(state): State<AppState>,
    LicenseManagerAuth(actor): LicenseManagerAuth,
    Json(req): Json<GenerateLicenseRequest>,
) -> ApiResult<Json<LicenseKey>> {
    let org_id = actor.org_id();
    let product = state.storage.get_product(req.product_id).await?;
    if product.org_id != org_id {
        return Err(ThaumielError::NotFound(format!("product '{}'", req.product_id)).into());
    }

    let backend_id = req
        .backend_id
        .clone()
        .unwrap_or_else(|| product.default_keygen_backend.clone());
    let backend = state.keygen.get(&backend_id)?;

    let gen_req = GenerateRequest {
        org_id,
        product_id: product.id,
        seats: req.seats,
        expires_at: req.expires_at,
        metadata: req.metadata.clone(),
    };
    let generated = backend.generate(&gen_req).await?;

    let license = LicenseKey {
        id: LicenseId::new(),
        org_id,
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
    audit::record(
        &state,
        org_id,
        actor.audit_label(),
        "license.generate",
        format!("license:{}", license.id),
    )
    .await;
    Ok(Json(license))
}

pub async fn list(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<Vec<LicenseKey>>> {
    let licenses = state
        .storage
        .list_licenses(identity.org_id, page.into())
        .await?;
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
    LicenseManagerAuth(actor): LicenseManagerAuth,
    Path(id): Path<LicenseId>,
) -> ApiResult<Json<LicenseKey>> {
    let org_id = actor.org_id();
    let existing = state.storage.get_license(id).await?;
    if existing.org_id != org_id {
        return Err(ThaumielError::NotFound(format!("license '{id}'")).into());
    }
    let license = state
        .storage
        .set_license_status(id, LicenseStatus::Revoked)
        .await?;
    audit::record(
        &state,
        org_id,
        actor.audit_label(),
        "license.revoke",
        format!("license:{id}"),
    )
    .await;
    Ok(Json(license))
}

/// Every machine that has activated a seat against this license. See issue #7.
pub async fn activations(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path(id): Path<LicenseId>,
) -> ApiResult<Json<Vec<Activation>>> {
    let license = state.storage.get_license(id).await?;
    if license.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("license '{id}'")).into());
    }
    Ok(Json(state.storage.list_activations(id).await?))
}

/// Frees one seat without revoking the whole license.
pub async fn revoke_activation(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path((id, activation_id)): Path<(LicenseId, ActivationId)>,
) -> ApiResult<axum::http::StatusCode> {
    let license = state.storage.get_license(id).await?;
    if license.org_id != identity.org_id {
        return Err(ThaumielError::NotFound(format!("license '{id}'")).into());
    }
    state.storage.delete_activation(id, activation_id).await?;
    audit::record(
        &state,
        identity.org_id,
        format!("user:{}", identity.user_id),
        "license.activation.revoke",
        format!("license:{id} activation:{activation_id}"),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn invalid(reason: impl Into<String>) -> ValidateLicenseResponse {
    ValidateLicenseResponse {
        valid: false,
        reason: Some(reason.into()),
        seats_total: None,
        seats_used: None,
    }
}

/// The one route meant to be called from a shipped application (via
/// [`ApiKeyAuth`], not an admin session) -- so it's rate-limited per calling
/// API key on top of normal auth.
pub async fn validate(
    State(state): State<AppState>,
    ApiKeyAuth(api_key): ApiKeyAuth,
    Json(req): Json<ValidateLicenseRequest>,
) -> ApiResult<Json<ValidateLicenseResponse>> {
    rate_limit::check(
        state.cache.as_ref(),
        &format!("validate:{}", api_key.key_prefix),
        120,
        Duration::from_secs(60),
    )
    .await?;
    crate::usage::record_validation(state.cache.as_ref(), api_key.org_id).await;

    let Ok(license) = state.storage.get_license_by_key(&req.key).await else {
        return Ok(Json(invalid("license not found")));
    };
    if license.org_id != api_key.org_id || license.product_id != req.product_id {
        return Ok(Json(invalid("license does not match this product")));
    }
    if !license.is_usable(Utc::now()) {
        return Ok(Json(invalid(format!(
            "license is not active (status: {:?})",
            license.status
        ))));
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
        let already_activated = state
            .storage
            .list_activations(license.id)
            .await?
            .iter()
            .any(|a| a.machine_fingerprint == fingerprint);
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

    Ok(Json(ValidateLicenseResponse {
        valid: true,
        reason: None,
        seats_total: Some(license.seats),
        seats_used: Some(seats_used),
    }))
}
