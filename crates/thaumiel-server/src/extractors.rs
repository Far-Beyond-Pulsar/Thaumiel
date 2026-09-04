//! Authentication extractors, matching `docs/ARCHITECTURE.md`'s split:
//!
//! - [`AdminAuth`]: `Authorization: Bearer <jwt>` session token, required by
//!   every dashboard-only route (organizations, products, api-keys,
//!   audit-log).
//! - [`ApiKeyAuth`]: `Authorization: Bearer <api key>` (or `X-Api-Key`), any
//!   active scope -- required by `/v1/licenses/validate`, the one route
//!   meant to be called from a shipped application rather than an admin
//!   dashboard/CLI.
//! - [`LicenseManagerAuth`]: an admin JWT *or* an API key scoped `admin` or
//!   `license_manager` -- for license-management routes (generate/revoke)
//!   that should also be reachable from automation without a human admin
//!   session, but not from a `validate_only` key. See issue #9.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::http::header;
use async_trait::async_trait;

use thaumiel_auth::{api_key, jwt};
use thaumiel_core::ids::OrganizationId;
use thaumiel_core::models::{ApiKey, ApiKeyScope};
use thaumiel_core::traits::Identity;
use thaumiel_core::ThaumielError;

use crate::error::ApiError;
use crate::state::AppState;

fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

fn presented_api_key(parts: &Parts) -> Option<&str> {
    bearer_token(parts).or_else(|| parts.headers.get("X-Api-Key").and_then(|v| v.to_str().ok()))
}

/// Shared by [`ApiKeyAuth`] and [`LicenseManagerAuth`]: look up, verify, and
/// touch `last_used_at` on a presented API key secret. Does not check scope
/// -- callers decide what scopes they accept.
async fn authenticate_api_key(state: &AppState, presented: &str) -> Result<ApiKey, ThaumielError> {
    let prefix =
        api_key::prefix_of(presented).ok_or_else(|| ThaumielError::Unauthenticated("malformed API key".into()))?;
    let record = state
        .storage
        .get_api_key_by_prefix(prefix)
        .await
        .map_err(|_| ThaumielError::Unauthenticated("invalid API key".into()))?;

    if !record.is_active() {
        return Err(ThaumielError::Unauthenticated("API key revoked".into()));
    }
    if !api_key::verify_api_key(presented, &record.key_hash) {
        return Err(ThaumielError::Unauthenticated("invalid API key".into()));
    }

    // Best-effort; a failure here shouldn't block the caller's request.
    if let Err(e) = state.storage.touch_api_key_last_used(record.id).await {
        tracing::warn!(error = %e, "failed to update api key last_used_at");
    }

    Ok(record)
}

pub struct AdminAuth(pub Identity);

#[async_trait]
impl<S> FromRequestParts<S> for AdminAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let token = bearer_token(parts)
            .ok_or_else(|| ThaumielError::Unauthenticated("missing bearer session token".into()))?;
        let identity = jwt::verify_token(token, &state.config.auth.jwt_secret)?;
        Ok(AdminAuth(identity))
    }
}

pub struct ApiKeyAuth(pub ApiKey);

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let presented =
            presented_api_key(parts).ok_or_else(|| ThaumielError::Unauthenticated("missing API key".into()))?;
        Ok(ApiKeyAuth(authenticate_api_key(&state, presented).await?))
    }
}

/// Whichever of an admin session or an API key authenticated the request --
/// see [`LicenseManagerAuth`] and [`crate::audit`], which accepts this for
/// its `actor` label.
pub enum Actor {
    User(Identity),
    ApiKey(ApiKey),
}

impl Actor {
    pub fn org_id(&self) -> OrganizationId {
        match self {
            Actor::User(identity) => identity.org_id,
            Actor::ApiKey(key) => key.org_id,
        }
    }

    /// `"user:<id>"` or `"api_key:<id>"`, for `AuditLogEntry::actor`.
    pub fn audit_label(&self) -> String {
        match self {
            Actor::User(identity) => format!("user:{}", identity.user_id),
            Actor::ApiKey(key) => format!("api_key:{}", key.id),
        }
    }
}

/// Accepts an admin session JWT unconditionally, or an API key scoped
/// `admin`/`license_manager` (not `validate_only`) -- for license-management
/// routes that should be usable from automation (a CI pipeline minting keys
/// on release, an internal ops tool) without requiring a human to log in.
pub struct LicenseManagerAuth(pub Actor);

#[async_trait]
impl<S> FromRequestParts<S> for LicenseManagerAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let presented =
            presented_api_key(parts).ok_or_else(|| ThaumielError::Unauthenticated("missing credentials".into()))?;

        // API keys and JWTs never collide in shape: every API key this
        // server issues starts with "thm_" (see thaumiel_auth::api_key),
        // which is not a valid JWT header segment.
        if presented.starts_with("thm_") {
            let record = authenticate_api_key(&state, presented).await?;
            if !matches!(record.scope, ApiKeyScope::Admin | ApiKeyScope::LicenseManager) {
                return Err(ThaumielError::Forbidden(
                    "this API key's scope cannot manage licenses (needs 'admin' or 'license_manager')".into(),
                )
                .into());
            }
            Ok(LicenseManagerAuth(Actor::ApiKey(record)))
        } else {
            let identity = jwt::verify_token(presented, &state.config.auth.jwt_secret)?;
            Ok(LicenseManagerAuth(Actor::User(identity)))
        }
    }
}
