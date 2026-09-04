pub mod api_keys;
pub mod audit_log;
pub mod auth;
pub mod health;
pub mod keygen_backends;
pub mod licenses;
pub mod organizations;
pub mod products;

use axum::routing::{get, post};
use axum::Router;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::metrics_mw;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/organizations/me", get(organizations::me))
        .route("/products", post(products::create).get(products::list))
        .route("/products/:id", get(products::get))
        .route("/licenses/generate", post(licenses::generate))
        .route("/licenses/validate", post(licenses::validate))
        .route("/licenses", get(licenses::list))
        .route("/licenses/:id", get(licenses::get))
        .route("/licenses/:id/revoke", post(licenses::revoke))
        .route("/api-keys", post(api_keys::create).get(api_keys::list))
        .route("/api-keys/:id/revoke", post(api_keys::revoke))
        .route("/audit-log", get(audit_log::list))
        .route("/keygen-backends", get(keygen_backends::list));

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .nest("/v1", v1)
        // `route_layer` (not `layer`): only runs for requests that matched a
        // route, and is what guarantees the `MatchedPath` extension
        // `metrics_mw::track` reads is actually populated.
        .route_layer(axum::middleware::from_fn(metrics_mw::track))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http())
}
