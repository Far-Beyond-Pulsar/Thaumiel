# Thaumiel

<img width="250" height="250" alt="colorkit" src="https://github.com/user-attachments/assets/877182b7-98e1-4ef0-8ced-b8316e15c311" />

A license key server, written in Rust, built around one idea: almost nothing about how you issue and check licenses should be hardcoded. Which database you use, which key format your product ships with, how your admins log in — all of that is a plugin choice, not a fork.

Thaumiel exposes an HTTP API for managing organizations, products, license keys, and API keys, backed by whichever storage and cache you configure, authenticating admins through JWT sessions and machine clients through hashed API keys, and generating keys through one of several interchangeable backends. Three ship out of the box. Writing a fourth takes one file and a macro call, not a patch to core.

## What's actually in here

- **Storage**: PostgreSQL, MySQL/MariaDB, SQL Server, SQLite, and an in-memory backend for tests, all implementing the same `Storage` trait, all runnable side by side.
- **Cache**: Redis or a process-local in-memory cache. Used for rate limiting and readiness checks today; the trait is small enough to extend.
- **Auth**: Argon2id password hashing, JWT admin sessions, hashed API keys for machine callers, LDAP/Active Directory login, and OIDC token verification — all through one `AuthProvider` trait, so SAML can slot in later the same way without touching route handlers.
- **License key generation**: three built-in backends —
  - `ed25519` — signed, offline-verifiable keys. A licensed application can check one without calling home.
  - `hmac` — human-typable `HM-XXXX-XXXX-...` keys with an embedded checksum, the format most people picture when they hear "license key."
  - `opaque` — a plain random token, validated purely by database lookup. The simplest possible option, and the default.
- **Everything above is a compile-time plugin.** No dynamic loading, no unsafe ABI, no runtime crashes from a mismatched plugin build — just a Rust trait implementation that self-registers when its crate is linked in. See [`docs/PLUGINS.md`](docs/PLUGINS.md).
- Seat-limited activations (with a dedicated endpoint to free one without revoking the whole license), an audit log, per-org usage metering (`GET /v1/usage`), Prometheus metrics at `/metrics`, structured tracing, and a config system layered from file to environment variables.
- **A web dashboard** (`thaumiel-ui`) — a Next.js admin UI, statically exported and embedded into its own standalone Rust binary. One executable, no Node runtime required to run it, points at any Thaumiel API via config. See [`crates/thaumiel-ui/README.md`](crates/thaumiel-ui/README.md).

## Architecture

`thaumiel-core` defines the domain types and the traits everything else implements — it has no idea what a database or an HTTP request even is. `thaumiel-storage`, `thaumiel-cache`, `thaumiel-auth`, and `thaumiel-keygen` each provide concrete implementations of those traits, registering themselves via a small `inventory`-based macro so `thaumiel-server` can discover whatever got linked in at startup, without a central list anywhere. `thaumiel-server` is the thin layer that turns all of that into an axum HTTP API. Full breakdown, including why it's built this way, in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

```
crates/
  thaumiel-core/      domain types, error type, plugin traits, the registry itself
  thaumiel-config/     layered TOML + env configuration
  thaumiel-storage/    Storage impls: postgres, mysql, mssql, sqlite, memory
  thaumiel-cache/      Cache impls: redis, memory
  thaumiel-auth/       Argon2id passwords, JWT sessions, API keys, InternalAuthProvider
  thaumiel-keygen/     Ed25519, HMAC, and opaque keygen backends
  thaumiel-server/     axum app: routes, middleware, main.rs
  thaumiel-ui/         admin dashboard: Next.js static export embedded in its own Rust binary
migrations/            per-backend SQL, one folder per dialect
config/                default.toml, docker.toml, an example production file
docker/                Dockerfile + a compose stack with all three databases and Redis
docs/                  architecture, plugin guide, API reference, config reference, deployment
```

## Running it

Nothing to install for the default path. SQLite and an in-memory cache are the out-of-the-box configuration, so this is the whole setup:

```bash
cargo run -p thaumiel-server
```

The server listens on `:8080`, creates `thaumiel.db` next to wherever you ran it from, and logs which keygen backends and auth providers it found at startup. Try it:

```bash
curl http://localhost:8080/health
curl http://localhost:8080/v1/keygen-backends
```

### The full stack

To exercise Postgres, MySQL, SQL Server, and Redis instead of the defaults:

```bash
docker compose -f docker/docker-compose.yml up --build
```

That brings up all four databases plus Redis and a server pointed at Postgres. The other two stay reachable on their normal ports the whole time too, if you want to point a local run at either directly, e.g. `THAUMIEL_DATABASE__BACKEND=mysql THAUMIEL_DATABASE__URL=mysql://thaumiel:thaumiel@localhost:3306/thaumiel cargo run -p thaumiel-server`.

### The dashboard

```bash
cargo run -p thaumiel-ui
```

builds the Next.js frontend the first time (needs Node on the machine building it, not the one running the resulting binary — see [`crates/thaumiel-ui/README.md`](crates/thaumiel-ui/README.md)) and serves it on `:4200`, pointed at `http://localhost:8080` by default. Open it, register an organization, and the rest — products, licenses, API keys, the audit log — is click-through from there.

## A five-minute walkthrough

Register creates an organization and its first user in one step and hands back a session token:

```bash
curl -X POST localhost:8080/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"org_name":"Acme","email":"owner@acme.test","password":"hunter22222"}'
```

Grab the `token` from the response and use it as a bearer token for everything admin-facing. Create a product, then generate a license against it:

```bash
TOKEN=... # from above
PRODUCT_ID=$(curl -s -X POST localhost:8080/v1/products \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"name":"Widget Pro"}' | jq -r .id)

curl -X POST localhost:8080/v1/licenses/generate \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"product_id\":\"$PRODUCT_ID\",\"seats\":3}"
```

A generated license key is only useful to a shipped application if that application can check it, and license validation deliberately doesn't accept the admin session token — it wants an API key, the kind you'd embed in a client:

```bash
API_KEY=$(curl -s -X POST localhost:8080/v1/api-keys \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"name":"desktop client","scope":"validate_only"}' | jq -r .plaintext)

curl -X POST localhost:8080/v1/licenses/validate \
  -H "authorization: Bearer $API_KEY" -H 'content-type: application/json' \
  -d "{\"key\":\"<the key from above>\",\"product_id\":\"$PRODUCT_ID\",\"machine_fingerprint\":\"laptop-1\"}"
```

That last call returns `{"valid":true,"seats_total":3,"seats_used":1}` and, on a second call with a new fingerprint, tracks a second seat automatically. Full endpoint-by-endpoint reference lives in [`docs/API.md`](docs/API.md).

## Configuring it

Config loads from `config/default.toml`, then layers `config/<THAUMIEL_ENV>.toml` on top (default environment is `development`), then applies `THAUMIEL_*` environment variables last, nested keys joined with a double underscore — `THAUMIEL_DATABASE__URL`, `THAUMIEL_AUTH__JWT_SECRET`, and so on. `config/example.production.toml` is a starting point for a real deployment; copy it, don't commit the copy with real secrets in it. Every field, every default, and every environment variable name is documented in [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md).

## Writing a plugin

A new keygen backend is a struct, a trait implementation, and one macro call:

```rust
pub struct MyBackend;

#[async_trait::async_trait]
impl thaumiel_core::traits::KeygenBackend for MyBackend {
    fn id(&self) -> &'static str { "my-backend" }
    fn description(&self) -> &'static str { "..." }
    fn offline_verifiable(&self) -> bool { false }
    async fn generate(&self, req: &GenerateRequest) -> Result<GeneratedKey> { /* ... */ }
    async fn validate(&self, key: &str, ctx: &ValidateContext) -> Result<Validation> { /* ... */ }
}

thaumiel_core::register_keygen_backend!(|_ctx| MyBackend);
```

Add the crate as a dependency of `thaumiel-server`, and it shows up in `GET /v1/keygen-backends` and becomes selectable by id, with zero changes to any existing route or storage code. Auth providers follow the identical pattern against a different trait. The whole thing, including a Windows-specific linker gotcha worth knowing about before you hit it yourself, is written up in [`docs/PLUGINS.md`](docs/PLUGINS.md).

## Testing

```bash
cargo test --workspace
```

runs unit tests for password hashing, JWT round-trips, API key generation, and all three keygen backends, plus one end-to-end integration test that drives the full HTTP API — register, login, create a product, generate a license, mint an API key, validate — against `InMemoryStorage`, no external services required. `cargo clippy --workspace --all-targets` is clean.

## License

MIT. See [`LICENSE`](LICENSE).
