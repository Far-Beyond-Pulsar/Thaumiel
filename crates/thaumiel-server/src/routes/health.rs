use axum::extract::State;
use axum::response::IntoResponse;
use axum::{http::StatusCode, Json};
use serde_json::json;

use crate::state::AppState;

pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// Unlike `/health` (process is up), `/ready` actually round-trips storage
/// and cache so a load balancer can detect "up but can't serve traffic yet".
pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let storage_ok = state.storage.list_organizations(thaumiel_core::traits::Pagination { limit: 1, offset: 0 }).await.is_ok();
    let cache_ok = state.cache.get("__readiness_probe__").await.is_ok();

    let ok = storage_ok && cache_ok;
    let status = if ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(json!({ "storage": storage_ok, "cache": cache_ok })))
}
