use axum::extract::{Query, State};
use axum::Json;

use thaumiel_core::models::AuditLogEntry;

use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::pagination::PageQuery;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<Vec<AuditLogEntry>>> {
    let entries = state
        .storage
        .list_audit_log(identity.org_id, page.into())
        .await?;
    Ok(Json(entries))
}
