//! Layered configuration: `config/default.toml` → optional `config/<env>.toml`
//! (selected via `THAUMIEL_ENV`, default `development`) → `THAUMIEL_*`
//! environment variables (highest precedence, nested via double underscore,
//! e.g. `THAUMIEL_DATABASE__URL`).

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use thaumiel_core::{Result, ThaumielError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".into(),
            port: 8080,
        }
    }
}

/// Which linked-in [`thaumiel_core::traits::Storage`] backend to instantiate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseBackend {
    Postgres,
    Mysql,
    Sqlite,
    /// Non-persistent, process-local storage. Only sensible for tests/demos.
    Memory,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub backend: DatabaseBackend,
    /// Connection string. Ignored for `backend = "memory"`.
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub max_connections: u32,
}

fn default_pool_size() -> u32 {
    10
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: DatabaseBackend::Sqlite,
            url: "sqlite://thaumiel.db?mode=rwc".into(),
            max_connections: default_pool_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheBackend {
    Redis,
    Memory,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    pub backend: CacheBackend,
    #[serde(default = "default_redis_url")]
    pub redis_url: String,
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".into()
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            redis_url: default_redis_url(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// HMAC signing secret for admin session JWTs. Must be overridden via
    /// `THAUMIEL_AUTH__JWT_SECRET` in any non-development environment.
    pub jwt_secret: String,
    #[serde(default = "default_jwt_ttl")]
    pub jwt_ttl_secs: u64,
    /// Which linked-in `AuthProvider::id()` handles `/v1/auth/login`.
    #[serde(default = "default_auth_provider")]
    pub provider: String,
}

fn default_jwt_ttl() -> u64 {
    3600 * 12
}

fn default_auth_provider() -> String {
    "internal".into()
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "development-only-insecure-secret-change-me".into(),
            jwt_ttl_secs: default_jwt_ttl(),
            provider: default_auth_provider(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeygenConfig {
    /// Which linked-in `KeygenBackend::id()` is used when a product does not
    /// specify its own `default_keygen_backend`.
    #[serde(default = "default_keygen_backend")]
    pub default_backend: String,
}

fn default_keygen_backend() -> String {
    "opaque".into()
}

impl Default for KeygenConfig {
    fn default() -> Self {
        Self {
            default_backend: default_keygen_backend(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub json: bool,
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
}

fn default_log_level() -> String {
    "info".into()
}

fn default_true() -> bool {
    true
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            json: false,
            metrics_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub keygen: KeygenConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

impl AppConfig {
    /// Load `config/default.toml`, layer `config/<THAUMIEL_ENV>.toml` on top if
    /// present, then apply `THAUMIEL_*` env var overrides. `config_dir` is
    /// typically `"config"` relative to the process's working directory.
    pub fn load(config_dir: impl AsRef<std::path::Path>) -> Result<Self> {
        let config_dir = config_dir.as_ref();
        let env = std::env::var("THAUMIEL_ENV").unwrap_or_else(|_| "development".into());

        let mut figment = Figment::new();
        let default_path = config_dir.join("default.toml");
        if default_path.exists() {
            figment = figment.merge(Toml::file(default_path));
        }
        let env_path = config_dir.join(format!("{env}.toml"));
        if env_path.exists() {
            figment = figment.merge(Toml::file(env_path));
        }
        figment = figment.merge(Env::prefixed("THAUMIEL_").split("__"));

        figment
            .extract()
            .map_err(|e| ThaumielError::Config(e.to_string()))
    }
}
