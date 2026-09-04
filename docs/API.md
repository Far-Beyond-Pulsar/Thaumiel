# API reference

Base path for everything below except health/metrics is `/v1`. Every response body is JSON. Errors share one shape:

```json
{ "error": { "category": "not_found", "message": "license 'lic_...' not found" } }
```

`category` is a stable, low-cardinality string (`not_found`, `conflict`, `invalid_input`, `unauthenticated`, `forbidden`, `rate_limited`, `unknown_plugin`, or one of a few internal categories) — safe to branch on in client code. `message` is human-readable and not guaranteed stable between versions.

## Authentication

Two separate mechanisms, and a route accepts exactly one of them — see [`docs/ARCHITECTURE.md`](ARCHITECTURE.md#two-kinds-of-authentication-on-purpose) for why they're kept apart.

- **Admin session** — `Authorization: Bearer <jwt>`, obtained from `/v1/auth/register` or `/v1/auth/login`. Required by every organization/product/license-management/API-key/audit-log route.
- **API key** — `Authorization: Bearer <api key>` or `X-Api-Key: <api key>`, obtained from `POST /v1/api-keys`. Required by `POST /v1/licenses/validate` only — the one route meant to be called from a shipped application rather than a dashboard or admin script.

A request without the right kind of credential gets `401 Unauthenticated`, not a redirect or a partial response.

## Health, readiness, metrics

| Method | Path | Auth | Notes |
|---|---|---|---|
| `GET` | `/health` | none | Process is up. Always `{"status":"ok"}` if the server is answering at all. |
| `GET` | `/ready` | none | Round-trips storage and cache. `200` if both answer, `503` otherwise — the one to point a load balancer's readiness probe at. |
| `GET` | `/metrics` | none | Prometheus exposition format: `http_requests_total` and `http_request_duration_seconds`, labeled by method, route pattern, and status. |
| `GET` | `/v1/keygen-backends` | none | Every linked-in `KeygenBackend`: id, description, whether it's offline-verifiable. What a client SDK would query to know what key formats exist. |

## Auth

### `POST /v1/auth/register`

Creates a new organization and its first user (role `owner`) in one step, and logs them in. There's no separate "create an organization" admin route — see `docs/ARCHITECTURE.md` for why there's no multi-org/superadmin concept in this build.

```json
// request
{ "org_name": "Acme", "email": "owner@acme.test", "password": "at least 8 characters" }
// response (200)
{ "token": "<jwt>", "identity": { "user_id": "...", "org_id": "...", "email": "...", "role": "owner" } }
```

### `POST /v1/auth/login`

```json
// request -- org_id is required, since email uniqueness is scoped per organization
{ "org_id": "...", "email": "owner@acme.test", "password": "..." }
// response (200): same shape as /register
```

`401` on any wrong field, deliberately worded identically whether the email doesn't exist or the password is wrong — an internal-auth login endpoint that says which one it was is handing out a free list of valid email addresses to anyone willing to try one at a time.

## Organizations

### `GET /v1/organizations/me`

Returns the caller's own organization — resolved from the JWT, not a path parameter. There's no route to look up an arbitrary organization by id; a caller only ever sees their own.

## Products

### `POST /v1/products`

```json
// request
{ "name": "Widget Pro", "default_keygen_backend": "hmac" }
```

`default_keygen_backend` is optional; omitted, it falls back to the server's `keygen.default_backend` config value. Either way it's validated against the live registry at creation time (`400 unknown_plugin` immediately, rather than the error surfacing later on whoever first tries to generate a license).

### `GET /v1/products`

Lists products belonging to the caller's organization.

### `GET /v1/products/:id`

`404` if the product doesn't exist or belongs to a different organization — deliberately the same response for both, so this endpoint can't be used to enumerate other organizations' product ids by timing which error comes back.

## Licenses

### `POST /v1/licenses/generate`

```json
// request
{
  "product_id": "...",
  "seats": 3,                          // defaults to 1
  "expires_at": "2027-01-01T00:00:00Z", // optional, RFC 3339
  "backend_id": "ed25519",             // optional, overrides the product's default for this one key
  "metadata": { "customer": "acme" }   // optional, free-form
}
// response (200): the full LicenseKey row, including the plaintext key string
```

Unlike API keys, a license key's plaintext is stored and can be retrieved again later via `GET /v1/licenses/:id` — support staff looking a customer's key back up is a normal operation for a license server, unlike an API key's "show once" security posture.

### `GET /v1/licenses` / `GET /v1/licenses/:id`

List or fetch, scoped to the caller's organization the same way products are.

### `POST /v1/licenses/:id/revoke`

Sets status to `revoked` and stamps `revoked_at`. Idempotent in effect, though calling it twice writes two audit log entries.

### `POST /v1/licenses/validate`

The one API-key-authenticated route. See [`docs/ARCHITECTURE.md`](ARCHITECTURE.md#request-lifecycle-generating-and-validating-a-key) for the full six-step check this runs through.

```json
// request
{ "key": "HM-XXXX-...", "product_id": "...", "machine_fingerprint": "optional-device-id" }
// response (200) -- always 200; a "no" is a normal answer, not an error
{ "valid": true, "reason": null, "seats_total": 3, "seats_used": 1 }
```

`reason` is `null` on a valid result and a short human-readable string otherwise (`"license expired"`, `"seat limit reached"`, `"signature verification failed"`, and so on). `machine_fingerprint` is optional; omit it for a pure "is this key valid" check with no seat-consumption side effect. Supply it and it's new, and it counts against `seats` — supply one that's been seen before, and it's a no-op re-check, not a second activation.

Rate-limited per calling API key (120 requests/minute by default, fixed window, enforced via `Cache`); exceeding it returns `429` with a `Retry-After` header.

## API keys

### `POST /v1/api-keys`

```json
// request
{ "name": "desktop client", "scope": "validate_only", "env_tag": "live" }
// response (200) -- plaintext shown exactly once
{
  "plaintext": "thm_live_9f2a...",
  "id": "...", "org_id": "...", "name": "desktop client",
  "key_hash": "...", "key_prefix": "thm_live_9f2a",
  "scope": "validate_only", "created_at": "...", "last_used_at": null, "revoked_at": null
}
```

`scope` is one of `admin`, `license_manager`, `validate_only`; only `validate_only`'s behavior is actually enforced by a route today (`/v1/licenses/validate` accepts any active key regardless of scope, since it's currently the only API-key-gated route) — the field exists so scope-based restriction has somewhere to attach as more machine-facing routes are added, without a breaking schema change later.

### `GET /v1/api-keys`

Lists keys for the caller's organization. Never includes `key_hash` in a way that's useful for anything (it's a one-way hash, shown for transparency/debugging, not a secret leak) and never includes the plaintext, which isn't stored anywhere to include.

### `POST /v1/api-keys/:id/revoke`

Immediate. A revoked key fails `ApiKeyAuth` on its very next use with `401`, not a grace period.

## Audit log

### `GET /v1/audit-log`

Most recent 100 entries for the caller's organization, newest first. Every mutating admin action writes one entry — `organization.register`, `product.create`, `license.generate`, `license.revoke`, `api_key.create`, `api_key.revoke` — with the acting user, the action, and the target id. Writing to the audit log is best-effort: a failure there is logged and swallowed rather than failing the request that triggered it, since an audit trail hiccup shouldn't be the reason a legitimate admin action fails.
