-- Thaumiel core schema (MySQL/MariaDB dialect).
-- Id/foreign-key/short-unique columns use VARCHAR(N) (MySQL cannot key a bare
-- TEXT column); everything else mirrors migrations/postgres/0001_init.sql --
-- see thaumiel-storage/src/*/mod.rs for the shared (de)serialization code.

CREATE TABLE IF NOT EXISTS organizations (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    created_at VARCHAR(40) NOT NULL
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS products (
    id VARCHAR(36) PRIMARY KEY,
    org_id VARCHAR(36) NOT NULL,
    name TEXT NOT NULL,
    default_keygen_backend VARCHAR(128) NOT NULL,
    created_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (org_id) REFERENCES organizations(id),
    INDEX idx_products_org (org_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS license_keys (
    id VARCHAR(36) PRIMARY KEY,
    org_id VARCHAR(36) NOT NULL,
    product_id VARCHAR(36) NOT NULL,
    backend_id VARCHAR(128) NOT NULL,
    key_value VARCHAR(512) NOT NULL UNIQUE,
    status VARCHAR(32) NOT NULL,
    seats BIGINT NOT NULL,
    expires_at VARCHAR(40),
    metadata TEXT NOT NULL,
    created_at VARCHAR(40) NOT NULL,
    revoked_at VARCHAR(40),
    FOREIGN KEY (org_id) REFERENCES organizations(id),
    FOREIGN KEY (product_id) REFERENCES products(id),
    INDEX idx_licenses_org (org_id),
    INDEX idx_licenses_product (product_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS activations (
    id VARCHAR(36) PRIMARY KEY,
    license_id VARCHAR(36) NOT NULL,
    machine_fingerprint VARCHAR(512) NOT NULL,
    activated_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (license_id) REFERENCES license_keys(id),
    INDEX idx_activations_license (license_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS api_keys (
    id VARCHAR(36) PRIMARY KEY,
    org_id VARCHAR(36) NOT NULL,
    name TEXT NOT NULL,
    key_hash VARCHAR(128) NOT NULL,
    key_prefix VARCHAR(64) NOT NULL UNIQUE,
    scope VARCHAR(32) NOT NULL,
    created_at VARCHAR(40) NOT NULL,
    last_used_at VARCHAR(40),
    revoked_at VARCHAR(40),
    FOREIGN KEY (org_id) REFERENCES organizations(id),
    INDEX idx_api_keys_org (org_id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY,
    org_id VARCHAR(36) NOT NULL,
    email VARCHAR(320) NOT NULL,
    password_hash TEXT,
    role VARCHAR(32) NOT NULL,
    created_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (org_id) REFERENCES organizations(id),
    UNIQUE KEY uq_users_org_email (org_id, email)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS audit_log (
    id VARCHAR(36) PRIMARY KEY,
    org_id VARCHAR(36) NOT NULL,
    actor VARCHAR(256) NOT NULL,
    action VARCHAR(256) NOT NULL,
    target VARCHAR(256) NOT NULL,
    metadata TEXT NOT NULL,
    created_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (org_id) REFERENCES organizations(id),
    INDEX idx_audit_log_org (org_id)
) ENGINE=InnoDB;
