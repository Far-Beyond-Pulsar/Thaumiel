use axum::extract::State;
use axum::Json;

use thaumiel_core::models::AuditLogEntry;
use thaumiel_core::traits::Pagination;

use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>, AdminAuth(identity): AdminAuth) -> ApiResult<Json<Vec<AuditLogEntry>>> {
    let entries = state.storage.list_audit_log(identity.org_id, Pagination { limit: 100, offset: 0 }).await?;
    Ok(Json(entries))
}
