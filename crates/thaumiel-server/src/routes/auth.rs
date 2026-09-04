use axum::extract::State;
use axum::Json;
use chrono::Utc;

use thaumiel_auth::{jwt, password};
use thaumiel_core::ids::{OrganizationId, UserId};
use thaumiel_core::models::{Organization, Role, User};
use thaumiel_core::traits::{Credentials, Identity};
use thaumiel_core::ThaumielError;

use crate::audit;
use crate::dto::{LoginRequest, RegisterRequest, SessionResponse};
use crate::error::ApiResult;
use crate::state::AppState;

/// Creates a brand-new organization plus its first user (role `owner`) in one
/// step, and logs them in. There is no separate "create organization" admin
/// route -- see `docs/ARCHITECTURE.md` for why (no superadmin/multi-org
/// concept in this build).
pub async fn register(State(state): State<AppState>, Json(req): Json<RegisterRequest>) -> ApiResult<Json<SessionResponse>> {
    if req.password.len() < 8 {
        return Err(ThaumielError::InvalidInput("password must be at least 8 characters".into()).into());
    }

    let org = Organization { id: OrganizationId::new(), name: req.org_name, created_at: Utc::now() };
    let org = state.storage.create_organization(org).await?;

    let user = User {
        id: UserId::new(),
        org_id: org.id,
        email: req.email.clone(),
        password_hash: Some(password::hash_password(&req.password)?),
        role: Role::Owner,
        created_at: Utc::now(),
    };
    let user = state.storage.create_user(user).await?;

    audit::record(&state, org.id, format!("user:{}", user.id), "organization.register", format!("organization:{}", org.id)).await;

    let identity = Identity { user_id: user.id, org_id: org.id, email: user.email, role: user.role };
    let token = jwt::issue_token(&identity, &state.config.auth.jwt_secret, state.config.auth.jwt_ttl_secs)?;
    Ok(Json(SessionResponse { token, identity }))
}

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> ApiResult<Json<SessionResponse>> {
    let provider = state.auth_providers.get(&state.config.auth.provider)?;
    let identity = provider
        .authenticate(Credentials::Password { org_id: req.org_id, email: req.email, password: req.password })
        .await?;

    audit::record(&state, identity.org_id, format!("user:{}", identity.user_id), "auth.login", format!("user:{}", identity.user_id)).await;

    let token = jwt::issue_token(&identity, &state.config.auth.jwt_secret, state.config.auth.jwt_ttl_secs)?;
    Ok(Json(SessionResponse { token, identity }))
}
