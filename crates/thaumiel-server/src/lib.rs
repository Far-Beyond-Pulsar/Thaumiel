//! The Thaumiel HTTP API: wires a [`thaumiel_config::AppConfig`] into
//! concrete storage/cache/auth-provider/keygen-backend implementations (via
//! [`thaumiel_core::registry`]) and exposes the result as an [`axum::Router`].
//!
//! Split into a library so integration tests can build a full [`state::AppState`]
//! against [`thaumiel_storage::InMemoryStorage`] without going through `main`.

pub mod audit;
pub mod dto;
pub mod error;
pub mod extractors;
pub mod metrics_mw;
pub mod pagination;
pub mod plugins;
pub mod rate_limit;
pub mod routes;
pub mod state;
pub mod telemetry;
pub mod usage;

pub use state::AppState;
