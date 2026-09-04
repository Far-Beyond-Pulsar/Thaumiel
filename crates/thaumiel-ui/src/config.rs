//! Config for the UI binary itself -- deliberately separate from
//! `thaumiel-config`'s `AppConfig`. `thaumiel-ui` is meant to be deployable
//! on its own, possibly pointed at a Thaumiel server it doesn't share a
//! filesystem or process with, so it shouldn't need to link against the
//! server's config types (or agree on their shape) to exist.
//!
//! Same layering convention as the main server for consistency: a baseline,
//! then `config/<THAUMIEL_UI_ENV>.toml` (env defaults to `development`), then
//! `THAUMIEL_UI_*` environment variables, double-underscore-nested. The
//! baseline is *embedded at compile time* (`include_str!`), not read from
//! `config/default.toml` on disk at startup like thaumiel-server's is --
//! `thaumiel-ui` is meant to run as one self-contained binary with zero
//! required files next to it, and a relative `config/default.toml` path is
//! also how thaumiel-server finds *its* config; run both from the same
//! working directory (an easy thing to do by accident) and a disk-relative
//! lookup would silently load the wrong file, since the two schemas overlap
//! enough that figment wouldn't even notice.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

const EMBEDDED_DEFAULTS: &str = include_str!("../config/default.toml");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".into(),
            port: 4200,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    /// Handed to the browser at runtime via `GET /thaumiel-ui-config.json` --
    /// see `docs/CONFIGURATION.md` in this crate. Defaults to a known-good
    /// value (a thaumiel-server on localhost) so the UI is usable out of the
    /// box; override for anything beyond local development.
    pub base_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".into(),
        }
    }
}

impl UiConfig {
    /// `config_dir`, if it exists on disk relative to the current working
    /// directory, may contain a `<THAUMIEL_UI_ENV>.toml` to layer on top of
    /// the embedded defaults -- e.g. `config/production.toml` with just
    /// `[api] base_url = "..."` in it. Entirely optional; the binary runs
    /// with zero files present.
    pub fn load(config_dir: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let config_dir = config_dir.as_ref();
        let env = std::env::var("THAUMIEL_UI_ENV").unwrap_or_else(|_| "development".into());

        let mut figment = Figment::new().merge(Toml::string(EMBEDDED_DEFAULTS));
        let env_path = config_dir.join(format!("{env}.toml"));
        if env_path.exists() {
            figment = figment.merge(Toml::file(env_path));
        }
        figment = figment.merge(Env::prefixed("THAUMIEL_UI_").split("__"));

        Ok(figment.extract()?)
    }
}
