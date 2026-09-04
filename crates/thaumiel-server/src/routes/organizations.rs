use axum::extract::State;
use axum::Json;

use thaumiel_core::models::Organization;

use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::state::AppState;

/// There is no general "list/get any organization" route: a caller only ever
/// sees their own, scoped by their JWT's `org_id`. See `docs/ARCHITECTURE.md`.
pub async fn me(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
) -> ApiResult<Json<Organization>> {
    let org = state.storage.get_organization(identity.org_id).await?;
    Ok(Json(org))
}
