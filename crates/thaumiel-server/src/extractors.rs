//! Two authentication extractors, matching `docs/ARCHITECTURE.md`'s split:
//!
//! - [`AdminAuth`]: `Authorization: Bearer <jwt>` session token, required by
//!   every admin/management route (organizations, products, api-keys,
//!   audit-log, and license management).
//! - [`ApiKeyAuth`]: `Authorization: Bearer <api key>` (or `X-Api-Key`),
//!   required by `/v1/licenses/validate` -- the one route meant to be called
//!   from a shipped application rather than an admin dashboard/CLI.

use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::header;
use axum::http::request::Parts;

use thaumiel_auth::{api_key, jwt};
use thaumiel_core::models::ApiKey;
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
        let presented = bearer_token(parts)
            .or_else(|| parts.headers.get("X-Api-Key").and_then(|v| v.to_str().ok()))
            .ok_or_else(|| ThaumielError::Unauthenticated("missing API key".into()))?;

        let prefix = api_key::prefix_of(presented)
            .ok_or_else(|| ThaumielError::Unauthenticated("malformed API key".into()))?;
        let record = state
            .storage
            .get_api_key_by_prefix(prefix)
            .await
            .map_err(|_| ThaumielError::Unauthenticated("invalid API key".into()))?;

        if !record.is_active() {
            return Err(ThaumielError::Unauthenticated("API key revoked".into()).into());
        }
        if !api_key::verify_api_key(presented, &record.key_hash) {
            return Err(ThaumielError::Unauthenticated("invalid API key".into()).into());
        }

        // Best-effort; a failure here shouldn't block the caller's request.
        if let Err(e) = state.storage.touch_api_key_last_used(record.id).await {
            tracing::warn!(error = %e, "failed to update api key last_used_at");
        }

        Ok(ApiKeyAuth(record))
    }
}
