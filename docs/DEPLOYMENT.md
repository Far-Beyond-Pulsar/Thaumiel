# Deployment

Notes for running Thaumiel somewhere real, past `cargo run` on a laptop.

## Docker

`docker/Dockerfile` is a two-stage build — `rust:1.79-bookworm` compiles a release binary, `debian:bookworm-slim` runs it as a non-root `thaumiel` user. Build context is the repo root:

```bash
docker build -f docker/Dockerfile -t thaumiel-server .
```

`docker/docker-compose.yml` wraps that with Postgres, MySQL, and Redis, all three reachable on their normal ports so you can point tooling at any of them directly, plus a `server` service running against Postgres and Redis by default:

```bash
docker compose -f docker/docker-compose.yml up --build
```

The compose file sets `THAUMIEL_ENV=docker`, which layers `config/docker.toml` (checked into the repo, non-secret — it only changes hostnames from `localhost` to the compose service names) over `config/default.toml`. It also sets a real `THAUMIEL_AUTH__JWT_SECRET` and stable `THAUMIEL_KEYGEN_*_SECRET` values directly as environment variables, both placeholders — replace them before this stack is anything other than a local sandbox.

## Before this is a production deployment

A few things the codebase deliberately leaves to the deployer rather than assuming for you:

- **`THAUMIEL_AUTH__JWT_SECRET`, `THAUMIEL_KEYGEN_HMAC_SECRET`, `THAUMIEL_KEYGEN_ED25519_SECRET`.** All three ship with obviously-fake defaults specifically so nobody mistakes them for real ones. Generate real values (`openssl rand -hex 32`) and inject them as environment variables from whatever secret store your platform uses — not from a committed file. See `docs/CONFIGURATION.md`.
- **TLS.** Thaumiel speaks plain HTTP; it assumes a reverse proxy or load balancer in front of it terminates TLS. Nothing here handles certificates.
- **Database backups.** Whichever of Postgres/MySQL/SQL Server you run in production, back it up the way you'd back up any other production database for that engine — Thaumiel doesn't do anything special here, and running `backend = "sqlite"` in production at all is a choice to weigh carefully against your actual durability needs.
- **`cache.backend = "memory"` doesn't coordinate across instances.** Fine for a single-process deployment. Running more than one instance of `thaumiel-server` behind a load balancer needs `cache.backend = "redis"`, or rate limiting and readiness checks silently stop meaning anything across the fleet.
- **Migrations run automatically at startup** (`Storage::migrate()`, in `main.rs`, before the server starts accepting connections). Fine for a single instance; if you run several replicas that all start at once against the same database, make sure your deploy process doesn't race multiple migration runs against each other — the simplest fix is a single init/migration step ahead of the actual rollout, common in most container orchestrators.
- **CORS is wide open by default** (empty `server.cors_allowed_origins`, permissive). Fine for local development and for API-only backends sitting behind their own gateway; set `server.cors_allowed_origins` (`THAUMIEL_SERVER__CORS_ALLOWED_ORIGINS`) to an explicit list once a browser is calling this API directly from a specific origin -- see `docs/CONFIGURATION.md`.

## The dashboard

`thaumiel-ui` is a second, independent binary and deployment unit — build and run it separately from `thaumiel-server` (its own `docker build`, its own process, its own port, `:4200` by default). Point it at your API with `THAUMIEL_UI_API__BASE_URL` (see [`crates/thaumiel-ui/README.md`](../crates/thaumiel-ui/README.md)); nothing about deploying it requires the two to share a host. Same TLS caveat as the API: it speaks plain HTTP and expects a reverse proxy in front of it in anything but local development.

## Observability in production

Point a Prometheus scraper at `GET /metrics`; point your load balancer's readiness probe at `GET /ready`, not `/health` — the difference matters exactly at the moment a deploy rolls out and the process is up but its database connection isn't ready yet. Set `telemetry.json = true` for any log aggregator that expects structured lines rather than the human-readable default.

## Key rotation

Rotating `THAUMIEL_AUTH__JWT_SECRET` invalidates every outstanding session immediately — every admin has to log in again, nothing more.

The two keygen secrets differ from each other here in a way worth knowing before you rotate either. `ed25519` records the public key it used at generation time on the license row itself (`LicenseKey::metadata`, under a `__backend_` key), and validation prefers that recorded key over whatever the server's current signing key is — so rotating `THAUMIEL_KEYGEN_ED25519_SECRET` only changes what *new* keys get signed with, and every key issued before the rotation keeps validating exactly as before. `hmac` has no per-key equivalent; its checksum is always recomputed against the server's *current* `THAUMIEL_KEYGEN_HMAC_SECRET`, so rotating it invalidates every `hmac`-format key issued under the old one, all at once, with no grace period. Plan `hmac` rotations around your actual license lifecycle accordingly, or prefer `ed25519` for anything you expect to rotate.
