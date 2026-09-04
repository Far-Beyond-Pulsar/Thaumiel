//! Maps [`ThaumielError`] to an HTTP response. Lives here (not in
//! `thaumiel-core`) so `thaumiel-core` never needs an `axum` dependency --
//! see that crate's `error.rs` doc comment.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use thaumiel_core::ThaumielError;

pub struct ApiError(pub ThaumielError);

impl From<ThaumielError> for ApiError {
    fn from(e: ThaumielError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, retry_after) = match &self.0 {
            ThaumielError::NotFound(_) => (StatusCode::NOT_FOUND, None),
            ThaumielError::Conflict(_) => (StatusCode::CONFLICT, None),
            ThaumielError::InvalidInput(_) => (StatusCode::BAD_REQUEST, None),
            ThaumielError::Unauthenticated(_) => (StatusCode::UNAUTHORIZED, None),
            ThaumielError::Forbidden(_) => (StatusCode::FORBIDDEN, None),
            ThaumielError::RateLimited { retry_after_secs } => {
                (StatusCode::TOO_MANY_REQUESTS, Some(*retry_after_secs))
            }
            ThaumielError::UnknownPlugin { .. } => (StatusCode::BAD_REQUEST, None),
            ThaumielError::Storage(_)
            | ThaumielError::Cache(_)
            | ThaumielError::Crypto(_)
            | ThaumielError::Config(_)
            | ThaumielError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, None),
        };

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self.0, category = self.0.category(), "internal error");
        }

        let body = Json(json!({
            "error": {
                "category": self.0.category(),
                "message": self.0.to_string(),
            }
        }));

        let mut response = (status, body).into_response();
        if let Some(secs) = retry_after {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                secs.to_string().parse().unwrap(),
            );
        }
        response
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
