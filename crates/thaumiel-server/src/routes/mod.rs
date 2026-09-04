pub mod api_keys;
pub mod audit_log;
pub mod auth;
pub mod health;
pub mod keygen_backends;
pub mod licenses;
pub mod organizations;
pub mod products;
pub mod usage;
pub mod users;

use axum::http::HeaderValue;
use axum::routing::{delete, get, post};
use axum::Router;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::metrics_mw;
use crate::state::AppState;

/// Permissive (every origin, the zero-config default) unless
/// `server.cors_allowed_origins` names an explicit allow-list -- see that
/// field's doc comment in `thaumiel-config` and `docs/CONFIGURATION.md`.
/// Origin strings that don't parse as a valid header value are dropped with
/// a startup warning rather than panicking the server over a typo'd config.
fn cors_layer(allowed_origins: &[String]) -> CorsLayer {
    if allowed_origins.is_empty() {
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| {
            HeaderValue::from_str(o)
                .inspect_err(|e| tracing::warn!(origin = %o, error = %e, "invalid entry in server.cors_allowed_origins, ignoring"))
                .ok()
        })
        .collect();

    CorsLayer::new().allow_origin(AllowOrigin::list(origins)).allow_methods(tower_http::cors::Any).allow_headers(tower_http::cors::Any)
}

pub fn build_router(state: AppState) -> Router {
    let cors = cors_layer(&state.config.server.cors_allowed_origins);

    let v1 = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/login/oidc", post(auth::login_oidc));

    #[cfg(feature = "saml")]
    let v1 = v1
        .route("/auth/login/saml/metadata", get(auth::saml_metadata))
        .route("/auth/login/saml/start", get(auth::saml_start))
        .route("/auth/login/saml/acs", post(auth::saml_acs));

    let v1 = v1
        .route("/organizations/me", get(organizations::me))
        .route("/users", post(users::create).get(users::list))
        .route("/products", post(products::create).get(products::list))
        .route("/products/:id", get(products::get))
        .route("/licenses/generate", post(licenses::generate))
        .route("/licenses/validate", post(licenses::validate))
        .route("/licenses", get(licenses::list))
        .route("/licenses/:id", get(licenses::get))
        .route("/licenses/:id/revoke", post(licenses::revoke))
        .route("/licenses/:id/activations", get(licenses::activations))
        .route("/licenses/:id/activations/:activation_id", delete(licenses::revoke_activation))
        .route("/api-keys", post(api_keys::create).get(api_keys::list))
        .route("/api-keys/:id/revoke", post(api_keys::revoke))
        .route("/audit-log", get(audit_log::list))
        .route("/usage", get(usage::summary))
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
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http())
}
