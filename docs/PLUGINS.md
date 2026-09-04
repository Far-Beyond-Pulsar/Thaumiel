# Writing a plugin

Two kinds of plugin exist today: a `KeygenBackend` (a license key format) and an `AuthProvider` (a login method). Both register the same way, against the same underlying mechanism, described in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md#the-plugin-registry). This document is the practical walkthrough; that one is the reasoning behind it.

## Writing a keygen backend

Three files' worth of example already exist in `crates/thaumiel-keygen/src/` — `opaque.rs` is the shortest and worth reading first. The shape is always the same.

**1. Implement the trait.**

```rust
use async_trait::async_trait;
use thaumiel_core::registry::PluginContext;
use thaumiel_core::traits::{GenerateRequest, GeneratedKey, KeygenBackend, ValidateContext, Validation};
use thaumiel_core::Result;

pub struct MyBackend;

impl MyBackend {
    // Constructors take `&PluginContext` even if unused, so stateless and
    // stateful plugins register identically -- see ARCHITECTURE.md.
    pub fn new(_ctx: &PluginContext) -> Self {
        Self
    }
}

#[async_trait]
impl KeygenBackend for MyBackend {
    fn id(&self) -> &'static str {
        "my-backend" // stable forever once anything in production uses it
    }

    fn description(&self) -> &'static str {
        "One sentence, shown by GET /v1/keygen-backends."
    }

    fn offline_verifiable(&self) -> bool {
        false // true only if `validate` needs no external state at all
    }

    async fn generate(&self, req: &GenerateRequest) -> Result<GeneratedKey> {
        // req.org_id, req.product_id, req.seats, req.expires_at, req.metadata
        todo!()
    }

    async fn validate(&self, key: &str, ctx: &ValidateContext) -> Result<Validation> {
        // The route handler has already checked status/expiry/revocation
        // against storage before calling this. You're only answering: is
        // this key's own content/signature legitimate? See ARCHITECTURE.md's
        // "Why keygen backends don't touch storage" for the reasoning.
        todo!()
    }
}
```

**2. Register it**, typically at the bottom of the same file:

```rust
thaumiel_core::register_keygen_backend!(MyBackend::new);
```

**3. Link it in.** Add the crate as an ordinary dependency of `thaumiel-server` (`crates/thaumiel-server/Cargo.toml`). That's the entire integration. `KeygenRegistry::from_inventory`, called once in `main.rs`, discovers it automatically — no route handler, no match statement, no central list to edit.

`id()` is the one thing to get right before shipping a key with it: it's stored on every `LicenseKey::backend_id` row that backend ever generates, and validation looks the backend up by that string later. Renaming it after real license keys exist means those keys can no longer be validated.

## Writing an auth provider

Identical shape, different trait:

```rust
#[async_trait]
impl AuthProvider for MyProvider {
    fn id(&self) -> &'static str { "my-provider" }

    async fn authenticate(&self, credentials: Credentials) -> Result<Identity> {
        todo!()
    }
}

thaumiel_core::register_auth_provider!(MyProvider::new);
```

`Credentials` is an enum (`Password { .. }` today; add a variant for anything token-shaped, e.g. an OIDC id token, without breaking `InternalAuthProvider`'s handling of `Password`). Set `auth.provider = "my-provider"` in config once it's registered, and `/v1/auth/login` routes to it — again, with no change to `routes/auth.rs` itself.

`InternalAuthProvider` (`crates/thaumiel-auth/src/internal.rs`) is a complete, non-trivial example: it holds an `Arc<dyn Storage>` from its `PluginContext`, looks a user up by org and email, and verifies their Argon2id hash. Worth reading if the provider being written needs to actually check something, rather than just wrapping a third-party token verifier.

## A linking gotcha worth knowing about upfront

`inventory::submit!` registers a static that runs before `main` — it doesn't require anyone to call anything for the registration to happen. That's true in principle everywhere `inventory` runs, and true in practice on Linux and macOS without any extra care. On Windows, it isn't quite, and this project ran into it directly while building the three built-in keygen backends.

The short version: MSVC's linker only pulls an object file out of a static library (an `.rlib`, in Rust's case) when something *else* already being linked has an unresolved reference into it. A registration-only file — nothing calls into `opaque.rs`, nothing takes its address, it only contains an `inventory::submit!` — gives the linker no reason to pull that object file in at all, on Windows specifically. The registration code is real, it compiles fine, and it simply never ends up in the final binary. The registry comes back empty and nothing tells you why.

Two things fix it together, and this repo does both:

1. **`crates/thaumiel-server/src/plugins.rs`** takes the address of each plugin's constructor function (`Ed25519SignedKeygen::new as fn(...) -> ...`, and so on) from somewhere that's unambiguously part of the final binary. Taking a function's address is a real reference the linker has to resolve, which forces that object file to be pulled in.
2. **The root `Cargo.toml`**'s `[profile.dev.package.thaumiel-keygen]` / `thaumiel-auth` entries pin those two crates to a single codegen unit in dev builds. Without that, a crate's code can be split across several object files, and a reference to `new` doesn't guarantee it lands in the *same* object file as the `inventory::submit!` a few lines below it — so the forced reference in step 1 has to force in the right object file, not just some object file from that crate.

Do both jobs together and the registration is guaranteed to be included; do only one and it works by luck on small crates (which is exactly how this bug hid for a while during development, before an integration test caught it with a very literal "unknown plugin 'opaque'" error).

**What this means for a new plugin crate:** if it's small and its own crate, add its constructor's address to `ensure_builtin_plugins_linked()` in `plugins.rs`, and add a `[profile.dev.package.<your-crate>] codegen-units = 1` entry next to the existing ones in the workspace `Cargo.toml`. Release builds already compile the whole workspace at `codegen-units = 1` (see `[profile.release]`), so this is a dev/test-build-only concern — but it's exactly the build a fresh `cargo test` runs, which is where you'd otherwise discover it the hard way.

## Testing a plugin

Unit-test `generate`/`validate` directly against the struct, no registry involved — every built-in backend does this (`crates/thaumiel-keygen/src/*.rs`, bottom of each file). For an end-to-end check that registration itself actually works, the integration test in `crates/thaumiel-server/tests/integration.rs` is the pattern to copy: build an `AppState` from `InMemoryStorage`, call `ensure_builtin_plugins_linked()` (or your own equivalent, if the plugin lives outside this workspace), construct the registries from a `PluginContext`, and drive the real HTTP router with `tower::ServiceExt::oneshot`.
