use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::Utc;

use thaumiel_auth::api_key::generate_api_key;
use thaumiel_core::ids::ApiKeyId;
use thaumiel_core::models::ApiKey;
use thaumiel_core::traits::Pagination;
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::{CreateApiKeyRequest, CreateApiKeyResponse};
use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::pagination::PageQuery;
use crate::state::AppState;

pub async fn create(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Json(req): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<CreateApiKeyResponse>> {
    let generated = generate_api_key(&req.env_tag);
    let record = ApiKey {
        id: ApiKeyId::new(),
        org_id: identity.org_id,
        name: req.name,
        key_hash: generated.hash,
        key_prefix: generated.prefix,
        scope: req.scope,
        created_at: Utc::now(),
        last_used_at: None,
        revoked_at: None,
    };
    let record = state.storage.create_api_key(record).await?;
    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "api_key.create", format!("api_key:{}", record.id)).await;
    Ok(Json(CreateApiKeyResponse { plaintext: generated.plaintext, record }))
}

pub async fn list(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<Vec<ApiKey>>> {
    let keys = state.storage.list_api_keys(identity.org_id, page.into()).await?;
    Ok(Json(keys))
}

pub async fn revoke(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Path(id): Path<ApiKeyId>,
) -> ApiResult<Json<ApiKey>> {
    let keys = state.storage.list_api_keys(identity.org_id, Pagination { limit: 1000, offset: 0 }).await?;
    if !keys.iter().any(|k| k.id == id) {
        return Err(ThaumielError::NotFound(format!("api_key '{id}'")).into());
    }
    let key = state.storage.revoke_api_key(id).await?;
    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "api_key.revoke", format!("api_key:{id}")).await;
    Ok(Json(key))
}
