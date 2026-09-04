//! Internal authentication for Thaumiel.
//!
//! - [`password`]: Argon2id password hashing for [`thaumiel_core::models::User`] accounts.
//! - [`jwt`]: HS256 session tokens issued after a successful login.
//! - [`api_key`]: generation/verification for machine-to-machine
//!   [`thaumiel_core::models::ApiKey`]s (a separate mechanism from `AuthProvider`,
//!   since API keys authenticate individual requests rather than a login flow).
//! - [`internal`]: [`internal::InternalAuthProvider`], the built-in
//!   `AuthProvider` implementation, self-registered under id `"internal"`.
//!
//! Other providers (OIDC, SAML, LDAP) would live in sibling crates
//! implementing [`thaumiel_core::traits::AuthProvider`] the same way -- see
//! `docs/ARCHITECTURE.md`.

pub mod api_key;
pub mod internal;
pub mod jwt;
pub mod password;

pub use internal::InternalAuthProvider;
