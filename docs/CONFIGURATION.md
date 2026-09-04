# Configuration reference

Three layers, applied in this order, each overriding the last: `config/default.toml`, then `config/<THAUMIEL_ENV>.toml` (`THAUMIEL_ENV` defaults to `development` — set it to `docker` for the compose stack, `production` for a real deployment with your own file in place), then `THAUMIEL_*` environment variables. Env var names follow the TOML structure with a double underscore joining nested keys: `[database] url = "..."` becomes `THAUMIEL_DATABASE__URL`. Everything has a default; a server with no config files and no environment variables at all still starts, against SQLite and an in-memory cache.

## `[server]`

| Key | Env var | Default | |
|---|---|---|---|
| `bind` | `THAUMIEL_SERVER__BIND` | `0.0.0.0` | Interface to listen on. |
| `port` | `THAUMIEL_SERVER__PORT` | `8080` | |
| `cors_allowed_origins` | `THAUMIEL_SERVER__CORS_ALLOWED_ORIGINS` | `[]` | Empty means permissive (every origin, no config needed). A non-empty list restricts CORS to exactly those origins. As an env var, a comma-separated list (figment's array parsing). |

## `[database]`

| Key | Env var | Default | |
|---|---|---|---|
| `backend` | `THAUMIEL_DATABASE__BACKEND` | `sqlite` | One of `postgres`, `mysql`, `mssql`, `sqlite`, `memory`. |
| `url` | `THAUMIEL_DATABASE__URL` | `sqlite://thaumiel.db?mode=rwc` | Connection string. Ignored when `backend = "memory"`. For `mssql`, this is an ADO-style string (`Server=tcp:host,1433;Database=thaumiel;User Id=sa;Password=...;TrustServerCertificate=true;`), not a URL -- see `thaumiel_storage::MssqlStorage`. |
| `max_connections` | `THAUMIEL_DATABASE__MAX_CONNECTIONS` | `10` | Pool size. |

`backend = "memory"` is real, not a stub — `InMemoryStorage` implements the full `Storage` trait — but it's process-local and non-persistent, so it's a fit for tests and quick demos, not anything you'd restart expecting data to survive.

## `[cache]`

| Key | Env var | Default | |
|---|---|---|---|
| `backend` | `THAUMIEL_CACHE__BACKEND` | `memory` | `redis` or `memory`. |
| `redis_url` | `THAUMIEL_CACHE__REDIS_URL` | `redis://127.0.0.1:6379` | Ignored when `backend = "memory"`. |

## `[auth]`

| Key | Env var | Default | |
|---|---|---|---|
| `jwt_secret` | `THAUMIEL_AUTH__JWT_SECRET` | an insecure placeholder | HMAC signing key for session JWTs. **Override this in anything that isn't local development** — the default is deliberately obvious and checked into the repo so it's impossible to mistake for a real secret. |
| `jwt_ttl_secs` | `THAUMIEL_AUTH__JWT_TTL_SECS` | `43200` (12h) | Session lifetime. |
| `provider` | `THAUMIEL_AUTH__PROVIDER` | `internal` | Which registered `AuthProvider::id()` handles `/v1/auth/login`. `internal` (Argon2id) or `ldap` ship today — see `docs/ARCHITECTURE.md`'s roadmap section for OIDC/SAML. |

## `[keygen]`

| Key | Env var | Default | |
|---|---|---|---|
| `default_backend` | `THAUMIEL_KEYGEN__DEFAULT_BACKEND` | `opaque` | Used when a product doesn't set its own `default_keygen_backend`. Must match a registered `KeygenBackend::id()` -- checked at product-creation time, not deferred to first use. |

Two more environment variables control keygen backends directly and are **not** part of `AppConfig` — read straight from the process environment by the backends themselves, deliberately kept out of the layered TOML system since key material has a different rotation lifecycle than ordinary configuration (see `docs/ARCHITECTURE.md`):

| Env var | Used by | |
|---|---|---|
| `THAUMIEL_KEYGEN_HMAC_SECRET` | the `hmac` backend | Hex-encoded HMAC key. Unset → an ephemeral random secret is generated at startup and logged as a warning; every key minted that run stops validating after a restart. |
| `THAUMIEL_KEYGEN_ED25519_SECRET` | the `ed25519` backend | 32 bytes, hex-encoded, used as the Ed25519 signing seed. Same ephemeral-fallback behavior, plus the public key (and therefore what offline clients need to verify against) changes on every restart until this is set. |

Generate either with `openssl rand -hex 32`.

## LDAP (only read when `auth.provider = "ldap"`)

Also read directly from the process environment, not `AppConfig`, for the same reason as the keygen secrets above -- this is connection/credential material for an external directory, not ordinary server configuration.

| Env var | Default | |
|---|---|---|
| `THAUMIEL_LDAP_URL` | *(empty)* | e.g. `ldap://dc.example.com:389` or `ldaps://dc.example.com:636`. Unset -> every LDAP login fails, logged as a warning at startup. |
| `THAUMIEL_LDAP_BIND_DN` | *(empty)* | Service account DN used for the search phase, e.g. `cn=readonly,dc=example,dc=com`. |
| `THAUMIEL_LDAP_BIND_PASSWORD` | *(empty)* | Service account password. |
| `THAUMIEL_LDAP_BASE_DN` | *(empty)* | Where to search for user entries, e.g. `ou=people,dc=example,dc=com`. |
| `THAUMIEL_LDAP_USER_FILTER` | `(mail={email})` | `{email}` is replaced with the (escaped) presented email. Active Directory deployments typically want `(userPrincipalName={email})`. |

On first successful login, a matching `User` row is created automatically (role `member`) if one doesn't already exist for that email within the org -- there's no separate LDAP-specific provisioning step to run first.

## OIDC (reachable via `POST /v1/auth/login/oidc`, independent of `auth.provider`)

Also read directly from the process environment.

| Env var | Default | |
|---|---|---|
| `THAUMIEL_OIDC_ISSUER_URL` | *(empty)* | e.g. `https://accounts.example.com`. Discovery (`{issuer}/.well-known/openid-configuration` + JWKS) happens lazily on first login attempt, not at startup, and is cached after that. Unset -> every OIDC login fails, logged as a warning at startup. |
| `THAUMIEL_OIDC_CLIENT_ID` | *(empty)* | Used as the expected audience when verifying a token's `aud` claim. |

Same JIT-provisioning behavior as LDAP: a matching `User` row (role `member`) is created automatically on first successful verification if one doesn't already exist for the token's email within the org given in the request. There's no client secret, no authorization-code exchange, and no redirect handling here -- the caller (a browser doing its own OIDC flow, a CLI that already has a token) hands Thaumiel an `id_token` it already obtained, and this only verifies it.

## SAML (cargo feature `saml`, **off by default** -- see below)

Reachable via three dedicated routes: `GET /v1/auth/login/saml/metadata`, `GET /v1/auth/login/saml/start?org_id=`, `POST /v1/auth/login/saml/acs` -- not gated by `auth.provider`, same reasoning as OIDC. See docs/API.md.

| Env var | Default | |
|---|---|---|
| `THAUMIEL_SAML_SP_ENTITY_ID` | *(empty)* | This server's SAML entity id -- typically its own metadata URL. |
| `THAUMIEL_SAML_ACS_URL` | *(empty)* | This server's public `.../v1/auth/login/saml/acs` URL -- what the IdP is told to POST responses back to. Must be reachable from wherever the IdP redirects the browser, so usually your real public hostname, not `localhost`. |
| `THAUMIEL_SAML_IDP_METADATA_URL` | *(empty)* | Fetched once, cached thereafter. Set this *or* `_PATH`, not both. |
| `THAUMIEL_SAML_IDP_METADATA_PATH` | *(empty)* | A local file path to a static IdP metadata XML document, for IdPs that hand you a file rather than exposing a live metadata endpoint. |

Any of the required three unset -> every SAML route fails with a clear `config` error, logged as a warning at startup, same pattern as LDAP/OIDC.

### Why this one is a feature flag, and off by default

Every other backend in this workspace builds with nothing but `cargo build`. SAML doesn't fit that: the only Rust SAML crate (`samael`) verifies XML signatures via `xmlsec1`, a C library, so building with `saml` enabled needs `libxml2-dev`, `libxmlsec1-dev`, `libxslt1-dev`, `libclang-dev`, and `pkg-config` installed as system packages first:

```bash
# Debian/Ubuntu (and this is exactly what docker/Dockerfile installs)
sudo apt install libxml2-dev libxmlsec1-dev libxslt1-dev libclang-dev pkg-config
cargo build -p thaumiel-server --features saml
```

On Windows, these aren't a one-line install -- if you're on Windows without WSL, build inside a Linux environment (WSL2, or the project's own `docker/Dockerfile`, which builds with `saml` enabled by default) rather than fighting native library installation directly. The default `cargo build` (no `--features saml`) needs none of this and is unaffected either way.

## `[telemetry]`

| Key | Env var | Default | |
|---|---|---|---|
| `log_level` | `THAUMIEL_TELEMETRY__LOG_LEVEL` | `info` | Anything `tracing_subscriber::EnvFilter` accepts, e.g. `thaumiel_server=debug,info`. `RUST_LOG`, if set, takes priority over this field entirely. |
| `json` | `THAUMIEL_TELEMETRY__JSON` | `false` | Structured JSON logs instead of human-readable text. |
| `metrics_enabled` | `THAUMIEL_TELEMETRY__METRICS_ENABLED` | `true` | Whether `/metrics` is mounted at all. |

## Example: pointing a local run at the Docker Compose databases

Everything below the `[server]` block, without touching any file:

```bash
THAUMIEL_DATABASE__BACKEND=postgres \
THAUMIEL_DATABASE__URL=postgres://thaumiel:thaumiel@localhost:5432/thaumiel \
THAUMIEL_CACHE__BACKEND=redis \
THAUMIEL_CACHE__REDIS_URL=redis://localhost:6379 \
cargo run -p thaumiel-server
```
