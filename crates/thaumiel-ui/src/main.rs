mod assets;
mod config;
mod serve;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use config::UiConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Arc::new(UiConfig::load("config")?);
    tracing::info!(api_base_url = %config.api.base_url, "starting thaumiel-ui");

    let app = Router::new()
        .route("/thaumiel-ui-config.json", get(runtime_config))
        .fallback(serve::static_handler)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(config.clone());

    let addr = format!("{}:{}", config.server.bind, config.server.port);
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// What the dashboard's JS fetches on boot to learn which Thaumiel API to
/// talk to -- see crates/thaumiel-ui/web/src/lib/runtime-config.ts. Not
/// embedded/cached: this is the one thing about a "static" export that's
/// actually decided at server-start time, from this binary's own config.
async fn runtime_config(State(config): State<Arc<UiConfig>>) -> Json<serde_json::Value> {
    Json(json!({ "apiBaseUrl": config.api.base_url }))
}
