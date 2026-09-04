//! Authentication for Thaumiel.
//!
//! - [`password`]: Argon2id password hashing for [`thaumiel_core::models::User`] accounts.
//! - [`jwt`]: HS256 session tokens issued after a successful login.
//! - [`api_key`]: generation/verification for machine-to-machine
//!   [`thaumiel_core::models::ApiKey`]s (a separate mechanism from `AuthProvider`,
//!   since API keys authenticate individual requests rather than a login flow).
//! - [`internal`]: [`internal::InternalAuthProvider`], the built-in
//!   `AuthProvider` implementation, self-registered under id `"internal"`.
//! - [`ldap`]: [`ldap::LdapAuthProvider`], id `"ldap"`. Set `auth.provider =
//!   "ldap"` in config to make it the one `/v1/auth/login` routes to; see
//!   `docs/CONFIGURATION.md` for its `THAUMIEL_LDAP_*` env vars.
//! - [`oidc`]: [`oidc::OidcAuthProvider`], id `"oidc"`, reachable via the
//!   separate `POST /v1/auth/login/oidc` route (not gated by `auth.provider`
//!   -- see that module's doc comment for why). `THAUMIEL_OIDC_*` env vars in
//!   `docs/CONFIGURATION.md`.
//! - [`saml`] (cargo feature `saml`, **off by default**): [`saml::SamlAuthProvider`].
//!   Not an `AuthProvider` at all -- see its module doc comment for why --
//!   reachable via three dedicated `/v1/auth/login/saml/*` routes when built
//!   with this feature. Off by default because it needs `libxml2`/`xmlsec1`
//!   as system libraries (via `samael`'s `xmlsec` feature) to build at all,
//!   unlike every other plugin in this workspace; see
//!   `docs/CONFIGURATION.md` and this crate's README.

pub mod api_key;
pub mod internal;
pub mod jwt;
pub mod ldap;
pub mod oidc;
pub mod password;
#[cfg(feature = "saml")]
pub mod saml;

pub use internal::InternalAuthProvider;
pub use ldap::LdapAuthProvider;
pub use oidc::OidcAuthProvider;
#[cfg(feature = "saml")]
pub use saml::SamlAuthProvider;
