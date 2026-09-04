-- Thaumiel core schema (SQL Server / T-SQL dialect). Mirrors
-- migrations/postgres/0001_init.sql; NVARCHAR everywhere id/foreign-key/text
-- columns exist, same reasoning as the MySQL migration (SQL Server also
-- needs an explicit length to index a string column) -- see
-- thaumiel-storage/src/mssql.rs, which (unlike postgres/mysql/sqlite) does
-- NOT share thaumiel-storage/src/mapping.rs's row-mapping code, since this
-- backend goes through `tiberius`, not `sqlx`.
--
-- Run as a single batch per statement by thaumiel-storage/src/mssql.rs's
-- `migrate()` (there is no sqlx::migrate! for MSSQL to lean on), so every
-- statement here is individually idempotent via an OBJECT_ID/sys.indexes
-- guard rather than relying on multi-statement batching.

IF OBJECT_ID(N'dbo.organizations', N'U') IS NULL
BEGIN
    CREATE TABLE organizations (
        id NVARCHAR(36) PRIMARY KEY,
        name NVARCHAR(MAX) NOT NULL,
        created_at NVARCHAR(40) NOT NULL
    )
END

IF OBJECT_ID(N'dbo.products', N'U') IS NULL
BEGIN
    CREATE TABLE products (
        id NVARCHAR(36) PRIMARY KEY,
        org_id NVARCHAR(36) NOT NULL REFERENCES organizations(id),
        name NVARCHAR(MAX) NOT NULL,
        default_keygen_backend NVARCHAR(128) NOT NULL,
        created_at NVARCHAR(40) NOT NULL
    )
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_products_org')
BEGIN
    CREATE INDEX idx_products_org ON products(org_id)
END

IF OBJECT_ID(N'dbo.license_keys', N'U') IS NULL
BEGIN
    CREATE TABLE license_keys (
        id NVARCHAR(36) PRIMARY KEY,
        org_id NVARCHAR(36) NOT NULL REFERENCES organizations(id),
        product_id NVARCHAR(36) NOT NULL REFERENCES products(id),
        backend_id NVARCHAR(128) NOT NULL,
        key_value NVARCHAR(900) NOT NULL UNIQUE,
        status NVARCHAR(32) NOT NULL,
        seats BIGINT NOT NULL,
        expires_at NVARCHAR(40) NULL,
        metadata NVARCHAR(MAX) NOT NULL,
        created_at NVARCHAR(40) NOT NULL,
        revoked_at NVARCHAR(40) NULL
    )
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_licenses_org')
BEGIN
    CREATE INDEX idx_licenses_org ON license_keys(org_id)
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_licenses_product')
BEGIN
    CREATE INDEX idx_licenses_product ON license_keys(product_id)
END

IF OBJECT_ID(N'dbo.activations', N'U') IS NULL
BEGIN
    CREATE TABLE activations (
        id NVARCHAR(36) PRIMARY KEY,
        license_id NVARCHAR(36) NOT NULL REFERENCES license_keys(id),
        machine_fingerprint NVARCHAR(900) NOT NULL,
        activated_at NVARCHAR(40) NOT NULL
    )
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_activations_license')
BEGIN
    CREATE INDEX idx_activations_license ON activations(license_id)
END

IF OBJECT_ID(N'dbo.api_keys', N'U') IS NULL
BEGIN
    CREATE TABLE api_keys (
        id NVARCHAR(36) PRIMARY KEY,
        org_id NVARCHAR(36) NOT NULL REFERENCES organizations(id),
        name NVARCHAR(MAX) NOT NULL,
        key_hash NVARCHAR(128) NOT NULL,
        key_prefix NVARCHAR(64) NOT NULL UNIQUE,
        scope NVARCHAR(32) NOT NULL,
        created_at NVARCHAR(40) NOT NULL,
        last_used_at NVARCHAR(40) NULL,
        revoked_at NVARCHAR(40) NULL
    )
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_api_keys_org')
BEGIN
    CREATE INDEX idx_api_keys_org ON api_keys(org_id)
END

IF OBJECT_ID(N'dbo.users', N'U') IS NULL
BEGIN
    CREATE TABLE users (
        id NVARCHAR(36) PRIMARY KEY,
        org_id NVARCHAR(36) NOT NULL REFERENCES organizations(id),
        email NVARCHAR(320) NOT NULL,
        password_hash NVARCHAR(MAX) NULL,
        role NVARCHAR(32) NOT NULL,
        created_at NVARCHAR(40) NOT NULL,
        CONSTRAINT uq_users_org_email UNIQUE (org_id, email)
    )
END

IF OBJECT_ID(N'dbo.audit_log', N'U') IS NULL
BEGIN
    CREATE TABLE audit_log (
        id NVARCHAR(36) PRIMARY KEY,
        org_id NVARCHAR(36) NOT NULL REFERENCES organizations(id),
        actor NVARCHAR(256) NOT NULL,
        action NVARCHAR(256) NOT NULL,
        target NVARCHAR(256) NOT NULL,
        metadata NVARCHAR(MAX) NOT NULL,
        created_at NVARCHAR(40) NOT NULL
    )
END

IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE name = 'idx_audit_log_org')
BEGIN
    CREATE INDEX idx_audit_log_org ON audit_log(org_id)
END
