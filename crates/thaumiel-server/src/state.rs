use std::sync::Arc;

use thaumiel_config::AppConfig;
use thaumiel_core::registry::{AuthProviderRegistry, KeygenRegistry};
use thaumiel_core::traits::{Cache, Storage};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage: Arc<dyn Storage>,
    pub cache: Arc<dyn Cache>,
    pub keygen: Arc<KeygenRegistry>,
    pub auth_providers: Arc<AuthProviderRegistry>,
}
