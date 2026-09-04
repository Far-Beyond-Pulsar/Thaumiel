//! Built-in [`thaumiel_core::traits::KeygenBackend`] implementations, each
//! self-registered via [`thaumiel_core::register_keygen_backend!`]:
//!
//! - [`ed25519_signed::Ed25519SignedKeygen`] (id `"ed25519"`) -- offline-verifiable, signed.
//! - [`hmac_formatted::HmacFormattedKeygen`] (id `"hmac"`) -- human-typable `HM-XXXX-...`.
//! - [`opaque::OpaqueTokenKeygen`] (id `"opaque"`) -- opaque random token, DB-lookup only.
//!
//! Importing this crate anywhere in the final binary (even just for its side
//! effects) is enough for all three to show up in
//! `thaumiel_core::registry::KeygenRegistry::from_inventory`; see
//! `docs/PLUGINS.md` for how to add a fourth without touching this crate.

pub mod ed25519_signed;
pub mod hmac_formatted;
pub mod opaque;

pub use ed25519_signed::Ed25519SignedKeygen;
pub use hmac_formatted::HmacFormattedKeygen;
pub use opaque::OpaqueTokenKeygen;
