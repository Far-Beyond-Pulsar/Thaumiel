-- Thaumiel core schema (PostgreSQL dialect).
-- All timestamps and ids are stored as TEXT (RFC3339 / UUID strings respectively)
-- deliberately, so the exact same application-level (de)serialization code works
-- unchanged across postgres/mysql/sqlite -- see thaumiel-storage/src/*/mod.rs.

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    default_keygen_backend TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_products_org ON products(org_id);

CREATE TABLE IF NOT EXISTS license_keys (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    product_id TEXT NOT NULL REFERENCES products(id),
    backend_id TEXT NOT NULL,
    key_value TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    seats BIGINT NOT NULL,
    expires_at TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_licenses_org ON license_keys(org_id);
CREATE INDEX IF NOT EXISTS idx_licenses_product ON license_keys(product_id);

CREATE TABLE IF NOT EXISTS activations (
    id TEXT PRIMARY KEY,
    license_id TEXT NOT NULL REFERENCES license_keys(id),
    machine_fingerprint TEXT NOT NULL,
    activated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_activations_license ON activations(license_id);

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    key_prefix TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_api_keys_org ON api_keys(org_id);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    email TEXT NOT NULL,
    password_hash TEXT,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(org_id, email)
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_log_org ON audit_log(org_id);
