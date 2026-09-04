use axum::extract::{Query, State};
use axum::Json;
use chrono::Utc;

use thaumiel_auth::password::hash_password;
use thaumiel_core::ids::UserId;
use thaumiel_core::models::{Role, User};
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::CreateUserRequest;
use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::pagination::PageQuery;
use crate::state::AppState;

/// Adds a user to the caller's organization (issue #8). Restricted to
/// `owner`/`admin` callers -- a `member` shouldn't be able to grant
/// themselves or anyone else more access than they already have.
pub async fn create(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Json(req): Json<CreateUserRequest>,
) -> ApiResult<Json<User>> {
    if !matches!(identity.role, Role::Owner | Role::Admin) {
        return Err(ThaumielError::Forbidden("only an owner or admin can add users".into()).into());
    }
    if req.password.len() < 8 {
        return Err(
            ThaumielError::InvalidInput("password must be at least 8 characters".into()).into(),
        );
    }

    let user = User {
        id: UserId::new(),
        org_id: identity.org_id,
        email: req.email,
        password_hash: Some(hash_password(&req.password)?),
        role: req.role,
        created_at: Utc::now(),
    };
    let user = state.storage.create_user(user).await?;
    audit::record(
        &state,
        identity.org_id,
        format!("user:{}", identity.user_id),
        "user.create",
        format!("user:{}", user.id),
    )
    .await;
    Ok(Json(user))
}

pub async fn list(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<Vec<User>>> {
    let users = state
        .storage
        .list_users(identity.org_id, page.into())
        .await?;
    Ok(Json(users))
}
