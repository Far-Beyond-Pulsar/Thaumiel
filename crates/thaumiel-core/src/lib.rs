//! Domain types, plugin traits, and the compile-time plugin registry shared by
//! every Thaumiel crate.
//!
//! `thaumiel-core` has no knowledge of HTTP, SQL, or any specific database or
//! cache — it only defines *shapes* ([`models`]), *contracts* ([`traits`]),
//! *errors* ([`error`]), and the *plugin registry* ([`registry`]) that lets
//! independent crates (`thaumiel-storage`, `thaumiel-cache`, `thaumiel-auth`,
//! `thaumiel-keygen`, and any third-party plugin crate) self-register
//! implementations without a central switch statement anywhere.
//!
//! See `docs/ARCHITECTURE.md` for the big picture and `docs/PLUGINS.md` for a
//! guide to writing a new plugin.

pub mod error;
pub mod ids;
pub mod models;
pub mod registry;
pub mod traits;

pub use error::{Result, ThaumielError};

/// Re-exported so [`register_keygen_backend!`] / [`register_auth_provider!`]
/// can expand to `$crate::inventory::submit!` from any downstream crate
/// without those crates needing their own direct dependency on `inventory`.
#[doc(hidden)]
pub use inventory;
