use thiserror::Error;

/// The single error type shared by every Thaumiel crate.
///
/// Kept transport-agnostic on purpose (no axum dependency here) so `thaumiel-core`
/// stays usable from plugin crates, CLIs, or tests without dragging in the HTTP
/// stack. `thaumiel-server` maps each variant to an HTTP status + JSON body.
#[derive(Debug, Error)]
pub enum ThaumielError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("already exists: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unauthenticated: {0}")]
    Unauthenticated(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("unknown plugin '{kind}' backend: '{id}'")]
    UnknownPlugin { kind: &'static str, id: String },

    #[error("storage error: {0}")]
    Storage(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T, E = ThaumielError> = std::result::Result<T, E>;

impl ThaumielError {
    /// Coarse category, useful for metrics labels and logging without leaking
    /// error message text (which may contain user input) into low-cardinality
    /// dimensions.
    pub fn category(&self) -> &'static str {
        match self {
            ThaumielError::NotFound(_) => "not_found",
            ThaumielError::Conflict(_) => "conflict",
            ThaumielError::InvalidInput(_) => "invalid_input",
            ThaumielError::Unauthenticated(_) => "unauthenticated",
            ThaumielError::Forbidden(_) => "forbidden",
            ThaumielError::RateLimited { .. } => "rate_limited",
            ThaumielError::UnknownPlugin { .. } => "unknown_plugin",
            ThaumielError::Storage(_) => "storage",
            ThaumielError::Cache(_) => "cache",
            ThaumielError::Crypto(_) => "crypto",
            ThaumielError::Config(_) => "config",
            ThaumielError::Internal(_) => "internal",
        }
    }
}
