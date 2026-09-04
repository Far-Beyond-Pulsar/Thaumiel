# Architecture

This document explains how Thaumiel is put together and, more importantly, why. The "what" is mostly readable from the code; the "why" is not, and that's the part worth writing down before it's forgotten.

## The core idea

A license server touches four things that enterprises tend to have strong, differing opinions about: where data lives, how admins authenticate, how machine clients authenticate, and what a license key actually looks like. Bake any of those in and you've written a tool for one company's stack, not a tool. So the design starts from a boundary: `thaumiel-core` defines what a storage backend, a cache, an auth provider, and a keygen backend *are* — as traits — and nothing else in the workspace is allowed to assume a specific one exists. Everything downstream is an implementation of one of those traits, chosen at startup by configuration, not compiled-in as the only option.

That boundary is also why `thaumiel-core` has no `axum` dependency and no `sqlx` dependency. It can't; it's imported by the storage crate and the server crate alike, and a storage backend shouldn't need to pull in an HTTP framework to exist. Errors cross that boundary through `ThaumielError`, a plain `thiserror` enum with no transport opinions of its own — `thaumiel-server` maps it to HTTP status codes at the edge, in `crates/thaumiel-server/src/error.rs`, precisely so that mapping doesn't have to live anywhere central.

## The crate graph

```
thaumiel-core     <- depended on by everything; depends on nothing in-repo
thaumiel-config   <- depends on core
thaumiel-storage  <- depends on core
thaumiel-cache    <- depends on core
thaumiel-auth     <- depends on core
thaumiel-keygen   <- depends on core
thaumiel-server   <- depends on all of the above; the only crate that knows HTTP exists
thaumiel-ui       <- standalone; depends on none of the above
```

`thaumiel-ui` (the admin dashboard) is deliberately off to the side rather than a dependent of `thaumiel-server`. It's a separate binary, with its own config, meant to be deployable on its own — potentially against a Thaumiel API it doesn't share a network, filesystem, or even a version with. It talks to that API exclusively over HTTP, the same way any other client would; nothing in `thaumiel-core`'s traits or `thaumiel-server`'s route handlers has any awareness that a UI exists. See [`crates/thaumiel-ui/README.md`](../crates/thaumiel-ui/README.md) for how a Next.js static export ends up embedded in and served by that binary.

No crate below `thaumiel-server` depends on any of its siblings. `thaumiel-storage` doesn't know `thaumiel-cache` exists, and neither knows `thaumiel-auth` exists. This is deliberate and it's checked by the compiler, not by convention: if a keygen backend ever needed direct database access, that would have to be modeled as a capability core hands it, not a new inter-crate dependency. It hasn't needed to be so far — see [Why keygen backends don't touch storage](#why-keygen-backends-dont-touch-storage) below.

## The plugin registry

Four traits live in `thaumiel_core::traits`: `Storage`, `Cache`, `AuthProvider`, `KeygenBackend`. Each is `async_trait`, object-safe, and used everywhere as `Arc<dyn Trait>` rather than a generic parameter — the alternative (making `AppState` generic over four trait parameters) would work, but it turns every function signature in the server into a wall of type parameters for no real benefit, since the backend is chosen once, at startup, from configuration.

Getting an implementation *into* that `Arc<dyn Trait>` without a central switch statement is what `thaumiel_core::registry` is for. A plugin crate writes:

```rust
thaumiel_core::register_keygen_backend!(MyBackend::new);
```

which expands to an `inventory::submit!` call, registering a constructor function into a process-wide collection keyed by type. At startup, `KeygenRegistry::from_inventory(&ctx)` walks every submission that got linked into the binary and constructs one instance of each, keyed by `KeygenBackend::id()`. The result: adding a backend means adding a Cargo dependency and one macro call, not editing a match statement in `thaumiel-server` that enumerates every backend that has ever existed. `docs/PLUGINS.md` walks through writing one end to end, including a linker subtlety on Windows that's worth reading before you hit it blind.

Constructors take a `&PluginContext` — currently just `Arc<dyn Storage>` and `Arc<dyn Cache>` — rather than being zero-argument. That's what lets a *stateful* plugin register exactly the same way a stateless one does. `thaumiel-auth`'s `InternalAuthProvider` needs to look users up in storage; it registers through `register_auth_provider!` exactly like a keygen backend that needs nothing at all. There's no separate "special" path for plugins that happen to need a database handle.

### Why keygen backends don't touch storage

`KeygenBackend::validate` takes a `ValidateContext` (org id, product id, and whatever metadata was recorded at generation time) but not a `Storage` handle. That's not an oversight — it's what keeps the trait honest about what it's actually responsible for. A keygen backend answers one question: *is this key's own content and signature valid?* Whether the underlying `LicenseKey` row is revoked, expired, or belongs to a different product is a storage-layer fact, and the route handler (`routes/licenses.rs::validate`) checks it before and independently of calling into the backend. Collapsing those two concerns into one trait method would mean every keygen backend author has to reimplement status/expiry/revocation logic, and get it right, every time.

## Domain model

Seven entities, defined once in `thaumiel_core::models` and shared by every storage backend and every route handler:

- **Organization** — the tenant boundary. Everything else hangs off an `org_id`.
- **Product** — belongs to an org, carries a `default_keygen_backend` id so most license generation calls don't need to specify one.
- **LicenseKey** — the thing being managed. Status (`active` / `suspended` / `revoked` / `expired`), seat count, optional expiry, and a `metadata` map that also carries backend-specific data (see below).
- **Activation** — one row per machine fingerprint that has checked in against a license, used purely for seat counting.
- **ApiKey** — machine credentials. Only a SHA-256 hash and a short lookup prefix are ever stored; the plaintext is shown once, at creation, and never again.
- **User** — an admin/dashboard account, Argon2id password hash included.
- **AuditLogEntry** — an append-only record of who did what, written by route handlers after every mutation.

IDs are UUIDv7 wrapped in per-entity newtypes (`OrganizationId`, `ProductId`, and so on) rather than a bare `Uuid` everywhere. The point isn't cosmetic: passing a `ProductId` where a `LicenseId` is expected is a compile error instead of a runtime bug that only shows up when someone's license lookup returns someone else's product.

One detail worth calling out: a generated key's `backend_metadata` (for instance, which Ed25519 public key signed it) is stored inside `LicenseKey::metadata` under keys prefixed `__backend_`, rather than in a separate column. There's no schema reason a dedicated column couldn't exist; it just isn't needed yet, and a prefix convention costs nothing to add a fifth keygen backend against later without another migration.

## Request lifecycle: generating and validating a key

Generation (`POST /v1/licenses/generate`, admin JWT required) looks up the product, resolves a keygen backend id (explicit in the request, or the product's default, or the server's configured default — in that order), calls `KeygenBackend::generate`, and persists the result as a new `LicenseKey` row with status `active`. Nothing about this path is backend-specific in the route handler; it's the same four lines regardless of whether `ed25519`, `hmac`, or a third-party backend is doing the actual key construction.

Validation (`POST /v1/licenses/validate`, API key required — not an admin session, see below) is the more interesting path, and it's worth walking in full since it's where most of the actual security logic lives:

1. Rate-limit check against the calling API key's prefix (`Cache::incr`, fixed window).
2. Look the license up by its key string. Not found → `valid: false`, not an error — a bad key from a client is an expected outcome, not a server fault.
3. Confirm the license's org and product match the calling API key's org and the request's product id. A key that's real but doesn't belong here fails closed.
4. Confirm the license is usable right now: `status == active` and, if set, `expires_at` hasn't passed.
5. Call the keygen backend's own `validate` — this is where signature/checksum verification happens for `ed25519` and `hmac`; `opaque` just checks the string shape, since its real check was step 2.
6. If a machine fingerprint was supplied and hasn't been seen before, check it against the seat count and record a new activation if there's room.

Every one of those steps can independently fail the request, and every failure returns `200 OK` with `{"valid": false, "reason": "..."}` rather than a 4xx — a license failing validation is the normal, expected shape of a "no" answer from this endpoint, not an error condition for the *caller* (the calling application) to handle as an exception.

## Two kinds of authentication, on purpose

Admin routes (organizations, products, license management, API key management, the audit log) require a JWT session, obtained from `/v1/auth/login` or `/v1/auth/register`, and verified per-request in `extractors::AdminAuth`. License validation requires an API key instead, verified in `extractors::ApiKeyAuth`, and does not accept an admin session token at all.

This isn't an oversight, it's a deliberate split of blast radius. An admin JWT can create products, mint other API keys, and revoke licenses; it has no business being embedded in a shipped desktop application just so that application can check its own license. An API key scoped `validate_only` can do exactly one thing — call the validate endpoint — and that's what should end up compiled into a client binary where anyone with a disassembler can eventually find it. If it leaks, the damage is "someone can burn your rate limit," not "someone can revoke every license you've ever issued."

`InternalAuthProvider` and `LdapAuthProvider` both implement `AuthProvider` (registered as `"internal"`/`"ldap"`, one selected via `auth.provider` in config, since both consume the same `Credentials::Password` shape). `OidcAuthProvider` also implements the trait (id `"oidc"`) but is reached via its own dedicated `POST /v1/auth/login/oidc` route rather than `auth.provider` dispatch, since a deployment might reasonably want password/LDAP login *and* OIDC side by side rather than a single either-or choice — see `thaumiel_auth::oidc`'s module doc comment.

`SamlAuthProvider` breaks the pattern entirely: it doesn't implement `AuthProvider` at all. SAML's browser-redirect flow (a metadata endpoint, a redirect to the IdP, a form POST back) has no way to fit through one `authenticate(Credentials) -> Identity` call the way a bearer token or a password does, so `thaumiel-server` holds it as a directly-typed `Arc<SamlAuthProvider>` on `AppState` and calls its own inherent methods (`metadata_xml`, `login_redirect_url`, `handle_acs`) from three dedicated routes instead of going through the generic registry at all — see `thaumiel_auth::saml`'s module doc comment for the full reasoning, including a known simplification (no `InResponseTo` replay tracking for SP-initiated logins yet).

SAML is also the one plugin in this entire workspace gated behind an opt-in, off-by-default Cargo feature (`saml`, on `thaumiel-auth` and forwarded through an identically-named feature on `thaumiel-server`): the only Rust SAML crate, `samael`, verifies XML signatures via `xmlsec1`, a C library with no pure-Rust equivalent, so building with `saml` enabled needs `libxml2-dev`/`libxmlsec1-dev`/`libxslt1-dev`/`libclang-dev`/`pkg-config` installed as system packages first -- not a `cargo build`-and-done story like every other plugin here. `docker/Dockerfile` installs them and enables the feature by default, since Debian is exactly the environment where that's a one-line `apt install`; a bare `cargo build` (no `--features saml`) needs none of it and is completely unaffected either way. Full detail in docs/CONFIGURATION.md.

## Storage: one trait, five backends, one row-mapping layer (mostly)

`Storage` is implemented five times: `PostgresStorage`, `MySqlStorage`, `SqliteStorage` (all in `thaumiel-storage`, each behind its own Cargo feature, all on by default) share one `sqlx`-based approach; `MssqlStorage` (also on by default) is the odd one out, described below; and `InMemoryStorage` backs the test suite and needs no external service at all.

Every `sqlx`-backed implementation uses `sqlx::query`/`query_as` with runtime-bound parameters, not the compile-time-checked `query!` macro. That's a conscious trade: `query!` needs a live database connection (or a maintained offline cache) *at compile time*, and maintaining that for three dialects simultaneously is friction that buys little here, since none of these queries are complex enough for compile-time column-type checking to be pulling much weight. What it does buy is that a schema/mapping bug only needs fixing once, in `thaumiel-storage/src/mapping.rs`, whose row-to-domain-type functions are generic over `R: sqlx::Row` and work unmodified against `PgRow`, `MySqlRow`, and `SqliteRow` alike.

`MssqlStorage` can't join that sharing: `sqlx` has no SQL Server driver at all, so SQL Server goes through `tiberius` (with `bb8`/`bb8-tiberius` supplying the connection pool `sqlx` gives the other three for free) -- a completely separate crate with its own `Row` type, unrelated to `sqlx::Row`. `thaumiel-storage/src/mssql.rs` keeps its own copy of the same row-mapping logic rather than fighting that boundary. It follows the same TEXT/NVARCHAR-everywhere schema convention as the other three (see `migrations/mssql/0001_init.sql`) and the same idempotent-DDL migration approach, just run as one `tiberius` batch instead of through `sqlx::migrate!` (which, like the rest of `sqlx`, doesn't know SQL Server exists).

That sharing is only possible because every column in every backend's schema is `TEXT`/`VARCHAR` — timestamps stored as RFC 3339 strings, UUIDs as their string form, enums as their lowercase name, `seats`/`COUNT(*)` as the one exception (wide integers, needed for correctness rather than style). Postgres's native `TIMESTAMPTZ` and MySQL's `DATETIME` decode to different Rust types through `sqlx`, and reconciling that per-backend would have undone exactly the sharing this design is trying to achieve. Storing everything as text costs a small amount of parse overhead and a small amount of column-level type safety at the database layer; it buys one mapping function per entity instead of three, and it's a trade this project makes gladly for a schema of this size.

Schema files live under `migrations/<backend>/`, one directory per dialect, embedded into the binary at compile time via `sqlx::migrate!` (which only reads files off disk during the build — no live database needed to compile, unlike `query!`). `Storage::migrate()` runs whichever set matches the configured backend, once, at server startup.

## Cache

`Cache` is smaller and correspondingly simpler: `get`/`set`/`del`/`incr`, all with an optional TTL. Two implementations — `RedisCache`, over a `redis::aio::ConnectionManager` (auto-reconnecting, cheap to clone, shared behind one `Arc` for the whole process rather than pooled), and `InMemoryCache`, a `Mutex<HashMap>` with lazy per-key expiry checked on access. The in-memory one exists so `cache.backend = "memory"` is a real, complete option for local development and single-instance deployments, not a stub — it's used for rate limiting and `/ready` checks the exact same way Redis is, just without cross-instance coordination.

## Configuration

Three layers, lowest to highest precedence: `config/default.toml`, then `config/<THAUMIEL_ENV>.toml` (`THAUMIEL_ENV` defaults to `development`), then `THAUMIEL_*` environment variables with double-underscore nesting (`THAUMIEL_DATABASE__URL`). Every `AppConfig` field carries a sane default, so the server starts with zero configuration files present at all — that's what makes `cargo run -p thaumiel-server` work with no setup. Full field-by-field reference: [`docs/CONFIGURATION.md`](CONFIGURATION.md).

Two secrets deliberately live outside this system entirely: `THAUMIEL_KEYGEN_HMAC_SECRET` and `THAUMIEL_KEYGEN_ED25519_SECRET`, read directly from the process environment by their respective keygen backends rather than through `AppConfig`. Key material and configuration have different operational lifecycles — you rotate one far more carefully than the other — and mixing them into one TOML file made that distinction easy to lose.

## Observability

`tracing` throughout, human-readable by default and JSON via `telemetry.json = true` for log aggregators. Every HTTP request is measured by a small `route_layer` middleware (`metrics_mw::track`) recording a request counter and a latency histogram, labeled by method, route *pattern* (not the literal path, so `/v1/licenses/:id` stays one label regardless of how many licenses exist), and status — exported in Prometheus text format at `/metrics`. `/health` reports process liveness; `/ready` actually round-trips storage and cache, so a load balancer can tell "the process started" apart from "the process can serve traffic."

## Roadmap

Written down on purpose rather than left implicit, since "is X supported" is a fair question to be able to answer precisely:

- **Multi-tenant billing/payment integration.** *Usage metering* -- `GET /v1/usage`, per-org resource counts and a 14-day validate-call history -- **is built** (see `docs/API.md`). Actual billing (pricing plans, payment processing, invoicing) is not, and wasn't attempted: it needs real business decisions (pricing, a payment processor account) this project has no basis for making, and building toward one speculatively would mean shipping code with no real integration to verify it against. The usage-metering endpoint exists specifically so a real billing system, whenever one gets built, has real data to build on rather than needing to invent its own metering from scratch.
- **Dynamic (cdylib) or WASM plugin loading.** Considered and deliberately not chosen, revisited and reaffirmed after this project added four more plugins (LDAP, OIDC, MSSQL, and a license-manager API-key auth path) the ordinary way with no friction. Compile-time registration means a plugin author gets full Rust, no ABI stability contract to maintain across compiler versions, and a build failure instead of a runtime crash when something's wrong — at the cost of needing a rebuild to add a plugin, which for the "enterprise extends this in-house" use case this is built for is a reasonable trade.

Three items that used to be on this list no longer are: an admin web UI (`thaumiel-ui` is built; see its [README](../crates/thaumiel-ui/README.md)), a SQL Server storage backend (`MssqlStorage`; see the storage section above -- note its "not exercised against a real SQL Server instance" caveat, though, in that section and in its own doc comment), and a SAML auth provider (`SamlAuthProvider`, real XML-DSig verification, feature-flagged off by default -- see the auth section above and docs/CONFIGURATION.md).

Each item above was tracked as an issue on the repo, not just a line in this file -- see the repo's issue tracker (closed issues included) for the full history of what was decided and why.
