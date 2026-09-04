//! `thaumiel-auth` and `thaumiel-keygen` self-register their plugins purely
//! by being linked in (via `inventory::submit!` in `thaumiel_core::registry`)
//! -- no call into either crate should be *required*. In practice, though,
//! some linkers (notably MSVC, used on Windows) only pull an rlib's compiled
//! code into the final binary when something in it is directly referenced;
//! an `inventory::submit!` ctor static on its own doesn't count as such a
//! reference from the *dependent* crate's point of view; and until such a
//! reference exists, the ctor simply never runs and the registry comes back
//! empty (see `Cargo.toml`'s `[profile.dev.package.*]` comment for the other
//! half of this fix -- pinning those crates to one codegen unit so that any
//! one reference pulls in all of their registrations, not just the one
//! symbol touched here).
//!
//! Call this once, before constructing a [`thaumiel_core::registry::KeygenRegistry`]
//! or [`thaumiel_core::registry::AuthProviderRegistry`]. `main.rs` and the
//! integration tests both do.
pub fn ensure_builtin_plugins_linked() {
    let _ = thaumiel_keygen::Ed25519SignedKeygen::new
        as fn(&thaumiel_core::registry::PluginContext) -> thaumiel_keygen::Ed25519SignedKeygen;
    let _ = thaumiel_keygen::HmacFormattedKeygen::new
        as fn(&thaumiel_core::registry::PluginContext) -> thaumiel_keygen::HmacFormattedKeygen;
    let _ = thaumiel_keygen::OpaqueTokenKeygen::new
        as fn(&thaumiel_core::registry::PluginContext) -> thaumiel_keygen::OpaqueTokenKeygen;
    let _ = thaumiel_auth::InternalAuthProvider::new
        as fn(&thaumiel_core::registry::PluginContext) -> thaumiel_auth::InternalAuthProvider;
}
