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
    /// Not part of `auth_providers` -- SAML's redirect-based flow doesn't
    /// fit the generic `AuthProvider`/`Credentials` shape those go through.
    /// See `thaumiel_auth::saml`'s module doc comment. Only present when
    /// built with `--features saml` (off by default; needs system libraries
    /// not every build environment has -- see docs/CONFIGURATION.md).
    #[cfg(feature = "saml")]
    pub saml: Arc<thaumiel_auth::SamlAuthProvider>,
}
