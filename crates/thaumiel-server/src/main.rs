use std::sync::Arc;

use thaumiel_config::{AppConfig, CacheBackend, DatabaseBackend};
use thaumiel_core::registry::{AuthProviderRegistry, KeygenRegistry, PluginContext};
use thaumiel_core::traits::{Cache, Storage};

use thaumiel_server::state::AppState;
use thaumiel_server::{routes, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load("config")?;
    telemetry::init(&config.telemetry);

    tracing::info!(
        database_backend = ?config.database.backend,
        cache_backend = ?config.cache.backend,
        "starting thaumiel-server"
    );

    let storage: Arc<dyn Storage> = build_storage(&config).await?;
    storage.migrate().await?;
    tracing::info!(backend = storage.id(), "storage migrations complete");

    let cache: Arc<dyn Cache> = build_cache(&config).await?;

    // See plugins.rs: forces the linker to actually include the built-in
    // plugin crates' registration code. Adding a third-party plugin crate
    // means adding it as a dependency and to that function; see docs/PLUGINS.md.
    thaumiel_server::plugins::ensure_builtin_plugins_linked();
    let ctx = PluginContext { storage: storage.clone(), cache: cache.clone() };
    let keygen = Arc::new(KeygenRegistry::from_inventory(&ctx));
    let auth_providers = Arc::new(AuthProviderRegistry::from_inventory(&ctx));
    // Not in the inventory registry -- see AppState::saml's doc comment.
    #[cfg(feature = "saml")]
    let saml = Arc::new(thaumiel_auth::SamlAuthProvider::new(&ctx));

    tracing::info!(backends = ?keygen.ids(), "keygen backends registered");
    tracing::info!(providers = ?auth_providers.ids(), "auth providers registered");
    #[cfg(not(feature = "saml"))]
    tracing::info!("built without the 'saml' feature -- /v1/auth/login/saml/* routes are not mounted");

    let metrics_handle = if config.telemetry.metrics_enabled {
        Some(metrics_exporter_prometheus::PrometheusBuilder::new().install_recorder()?)
    } else {
        None
    };

    let state = AppState {
        config: Arc::new(config.clone()),
        storage,
        cache,
        keygen,
        auth_providers,
        #[cfg(feature = "saml")]
        saml,
    };
    let mut app = routes::build_router(state);
    if let Some(handle) = metrics_handle {
        app = app.route("/metrics", axum::routing::get(move || async move { handle.render() }));
    }

    let addr = format!("{}:{}", config.server.bind, config.server.port);
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn build_storage(config: &AppConfig) -> anyhow::Result<Arc<dyn Storage>> {
    let db = &config.database;
    Ok(match db.backend {
        DatabaseBackend::Postgres => Arc::new(thaumiel_storage::PostgresStorage::connect(&db.url, db.max_connections).await?),
        DatabaseBackend::Mysql => Arc::new(thaumiel_storage::MySqlStorage::connect(&db.url, db.max_connections).await?),
        DatabaseBackend::Mssql => Arc::new(thaumiel_storage::MssqlStorage::connect(&db.url, db.max_connections).await?),
        DatabaseBackend::Sqlite => Arc::new(thaumiel_storage::SqliteStorage::connect(&db.url, db.max_connections).await?),
        DatabaseBackend::Memory => Arc::new(thaumiel_storage::InMemoryStorage::new()),
    })
}

async fn build_cache(config: &AppConfig) -> anyhow::Result<Arc<dyn Cache>> {
    Ok(match config.cache.backend {
        CacheBackend::Redis => Arc::new(thaumiel_cache::RedisCache::connect(&config.cache.redis_url).await?),
        CacheBackend::Memory => Arc::new(thaumiel_cache::InMemoryCache::new()),
    })
}
