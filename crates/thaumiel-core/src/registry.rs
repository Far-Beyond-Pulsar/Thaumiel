//! The compile-time plugin registry.
//!
//! Plugins never appear in a central list. Instead, a plugin crate calls
//! [`register_keygen_backend!`] or [`register_auth_provider!`] once (typically
//! in its `lib.rs`), which uses [`inventory`] to submit a constructor function
//! into a static, link-time-populated collection. `thaumiel-server` links every
//! plugin crate it wants available as an ordinary Cargo dependency (gated
//! behind a feature flag, by convention) and then calls
//! [`KeygenRegistry::from_inventory`] / [`AuthProviderRegistry::from_inventory`]
//! once at startup, passing a [`PluginContext`], to discover and construct
//! everything that got linked in — no server-side code change needed to add a
//! backend, only a `Cargo.toml` dependency + feature.
//!
//! Constructors receive a [`PluginContext`] (shared `Storage`/`Cache` handles)
//! rather than being zero-argument, so a plugin that needs persistence (like
//! `thaumiel-auth`'s internal password/JWT provider, which looks users up in
//! `Storage`) registers exactly the same way a stateless one (like any
//! `thaumiel-keygen` backend) does.
//!
//! See `docs/PLUGINS.md` for a walkthrough of writing a new plugin crate.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Result, ThaumielError};
use crate::traits::{AuthProvider, Cache, KeygenBackend, Storage};

/// Shared handles every plugin constructor gets, so plugins needing
/// persistence or caching don't need their own bespoke wiring.
#[derive(Clone)]
pub struct PluginContext {
    pub storage: Arc<dyn Storage>,
    pub cache: Arc<dyn Cache>,
}

/// A submitted keygen-backend constructor. Don't construct this directly; use
/// [`register_keygen_backend!`].
pub struct KeygenPlugin(pub fn(&PluginContext) -> Box<dyn KeygenBackend>);
inventory::collect!(KeygenPlugin);

/// A submitted auth-provider constructor. Don't construct this directly; use
/// [`register_auth_provider!`].
pub struct AuthProviderPlugin(pub fn(&PluginContext) -> Box<dyn AuthProvider>);
inventory::collect!(AuthProviderPlugin);

/// Registers a [`KeygenBackend`] implementation so it is discovered by
/// [`KeygenRegistry::from_inventory`]. `$ctor` must be a capture-free
/// expression producing a `fn(&PluginContext) -> T` (e.g. a plain function
/// path); for backends that ignore the context, `|_ctx| MyBackend::new()`
/// works too since closures without captures coerce to `fn` pointers.
///
/// ```ignore
/// thaumiel_core::register_keygen_backend!(|_ctx| MyBackend::new());
/// ```
#[macro_export]
macro_rules! register_keygen_backend {
    ($ctor:expr) => {
        $crate::inventory::submit! {
            $crate::registry::KeygenPlugin(|ctx: &$crate::registry::PluginContext| {
                // Explicit annotation forces the `Box<Concrete>` ->
                // `Box<dyn KeygenBackend>` unsized coercion here, so the
                // closure's inferred return type matches the plain `fn`
                // pointer signature `KeygenPlugin` requires exactly.
                let backend: ::std::boxed::Box<dyn $crate::traits::KeygenBackend> =
                    ::std::boxed::Box::new(($ctor)(ctx));
                backend
            })
        }
    };
}

/// Registers an [`AuthProvider`] implementation so it is discovered by
/// [`AuthProviderRegistry::from_inventory`]. Same shape as
/// [`register_keygen_backend!`].
#[macro_export]
macro_rules! register_auth_provider {
    ($ctor:expr) => {
        $crate::inventory::submit! {
            $crate::registry::AuthProviderPlugin(|ctx: &$crate::registry::PluginContext| {
                let provider: ::std::boxed::Box<dyn $crate::traits::AuthProvider> =
                    ::std::boxed::Box::new(($ctor)(ctx));
                provider
            })
        }
    };
}

/// Every [`KeygenBackend`] linked into the current binary, keyed by
/// [`KeygenBackend::id`].
pub struct KeygenRegistry {
    backends: HashMap<&'static str, Arc<dyn KeygenBackend>>,
}

impl KeygenRegistry {
    /// Instantiate one copy of every backend submitted via
    /// [`register_keygen_backend!`] in any linked crate.
    pub fn from_inventory(ctx: &PluginContext) -> Self {
        let mut backends = HashMap::new();
        for plugin in inventory::iter::<KeygenPlugin> {
            let backend: Arc<dyn KeygenBackend> = Arc::from((plugin.0)(ctx));
            backends.insert(backend.id(), backend);
        }
        Self { backends }
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn KeygenBackend>> {
        self.backends
            .get(id)
            .cloned()
            .ok_or_else(|| ThaumielError::UnknownPlugin {
                kind: "keygen_backend",
                id: id.to_string(),
            })
    }

    pub fn ids(&self) -> Vec<&'static str> {
        let mut ids: Vec<_> = self.backends.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn KeygenBackend>> {
        self.backends.values()
    }
}

/// Every [`AuthProvider`] linked into the current binary, keyed by
/// [`AuthProvider::id`].
pub struct AuthProviderRegistry {
    providers: HashMap<&'static str, Arc<dyn AuthProvider>>,
}

impl AuthProviderRegistry {
    pub fn from_inventory(ctx: &PluginContext) -> Self {
        let mut providers = HashMap::new();
        for plugin in inventory::iter::<AuthProviderPlugin> {
            let provider: Arc<dyn AuthProvider> = Arc::from((plugin.0)(ctx));
            providers.insert(provider.id(), provider);
        }
        Self { providers }
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn AuthProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| ThaumielError::UnknownPlugin {
                kind: "auth_provider",
                id: id.to_string(),
            })
    }

    pub fn ids(&self) -> Vec<&'static str> {
        let mut ids: Vec<_> = self.providers.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}
