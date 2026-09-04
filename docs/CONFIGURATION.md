# Configuration reference

Three layers, applied in this order, each overriding the last: `config/default.toml`, then `config/<THAUMIEL_ENV>.toml` (`THAUMIEL_ENV` defaults to `development` — set it to `docker` for the compose stack, `production` for a real deployment with your own file in place), then `THAUMIEL_*` environment variables. Env var names follow the TOML structure with a double underscore joining nested keys: `[database] url = "..."` becomes `THAUMIEL_DATABASE__URL`. Everything has a default; a server with no config files and no environment variables at all still starts, against SQLite and an in-memory cache.

## `[server]`

| Key | Env var | Default | |
|---|---|---|---|
| `bind` | `THAUMIEL_SERVER__BIND` | `0.0.0.0` | Interface to listen on. |
| `port` | `THAUMIEL_SERVER__PORT` | `8080` | |

## `[database]`

| Key | Env var | Default | |
|---|---|---|---|
| `backend` | `THAUMIEL_DATABASE__BACKEND` | `sqlite` | One of `postgres`, `mysql`, `sqlite`, `memory`. |
| `url` | `THAUMIEL_DATABASE__URL` | `sqlite://thaumiel.db?mode=rwc` | Connection string. Ignored when `backend = "memory"`. |
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
| `provider` | `THAUMIEL_AUTH__PROVIDER` | `internal` | Which registered `AuthProvider::id()` handles `/v1/auth/login`. Only `internal` ships today — see `docs/ARCHITECTURE.md`'s roadmap section. |

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
