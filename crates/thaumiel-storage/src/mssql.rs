//! SQL Server backend, via `tiberius` + `bb8-tiberius` rather than `sqlx`
//! (which has no MSSQL driver at all -- see this crate's `Cargo.toml`).
//! Because of that, this module keeps its own row-mapping functions instead
//! of sharing `mapping.rs` with the other three SQL backends: `tiberius::Row`
//! is an unrelated type to `sqlx::Row`, with its own `get`/`try_get` API.
//!
//! `url` (`DatabaseConfig::url`) is an ADO-style connection string, e.g.
//! `Server=tcp:localhost,1433;Database=thaumiel;User Id=sa;Password=...;TrustServerCertificate=true;`.
//!
//! Note: unlike the other three backends, this one has not been exercised
//! against a real SQL Server instance as part of building it (no Docker
//! available in the environment that wrote it) -- it's implemented to the
//! same pattern and compiles, but treat it as less battle-tested than
//! postgres/mysql/sqlite until someone runs it for real. Issue tracked on
//! the repo if you hit something.

use async_trait::async_trait;
use bb8::Pool;
use bb8_tiberius::ConnectionManager;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tiberius::Row;
use uuid::Uuid;

use thaumiel_core::ids::{
    ActivationId, ApiKeyId, AuditLogId, LicenseId, OrganizationId, ProductId, UserId,
};
use thaumiel_core::models::{
    Activation, ApiKey, ApiKeyScope, AuditLogEntry, LicenseKey, LicenseStatus, Organization,
    Product, Role, User,
};
use thaumiel_core::traits::{Pagination, Storage};
use thaumiel_core::{Result, ThaumielError};

const MIGRATION_SQL: &str = include_str!("../../../migrations/mssql/0001_init.sql");

pub struct MssqlStorage {
    pool: Pool<ConnectionManager>,
}

impl MssqlStorage {
    pub async fn connect(url: &str, max_connections: u32) -> Result<Self> {
        let manager =
            ConnectionManager::build(url).map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let pool = Pool::builder()
            .max_size(max_connections)
            .build(manager)
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        Ok(Self { pool })
    }
}

fn tiberius_err(e: tiberius::error::Error) -> ThaumielError {
    ThaumielError::Storage(e.to_string())
}

fn not_found(entity: &str, ident: impl std::fmt::Display) -> ThaumielError {
    ThaumielError::NotFound(format!("{entity} '{ident}' not found"))
}

// -- row -> domain mapping (see module doc for why this isn't shared with
// the sqlx backends' mapping.rs) ---------------------------------------------

fn col_str(row: &Row, name: &str) -> Result<String> {
    row.try_get::<&str, _>(name)
        .map_err(|e| ThaumielError::Storage(format!("column '{name}': {e}")))?
        .map(|s| s.to_string())
        .ok_or_else(|| ThaumielError::Storage(format!("column '{name}' was unexpectedly NULL")))
}

fn col_opt_str(row: &Row, name: &str) -> Result<Option<String>> {
    row.try_get::<&str, _>(name)
        .map_err(|e| ThaumielError::Storage(format!("column '{name}': {e}")))
        .map(|opt| opt.map(|s| s.to_string()))
}

fn col_i64(row: &Row, name: &str) -> Result<i64> {
    row.try_get::<i64, _>(name)
        .map_err(|e| ThaumielError::Storage(format!("column '{name}': {e}")))?
        .ok_or_else(|| ThaumielError::Storage(format!("column '{name}' was unexpectedly NULL")))
}

fn parse_uuid(s: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(s)
        .map_err(|e| ThaumielError::Storage(format!("invalid uuid in '{field}': {e}")))
}

fn parse_dt(s: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ThaumielError::Storage(format!("invalid timestamp in '{field}': {e}")))
}

fn parse_opt_dt(s: Option<String>, field: &str) -> Result<Option<DateTime<Utc>>> {
    s.map(|s| parse_dt(&s, field)).transpose()
}

fn parse_metadata(s: &str) -> Result<HashMap<String, String>> {
    if s.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str(s)
        .map_err(|e| ThaumielError::Storage(format!("invalid metadata json: {e}")))
}

fn now_str() -> String {
    Utc::now().to_rfc3339()
}
fn dt_str(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}
fn metadata_json(meta: &HashMap<String, String>) -> Result<String> {
    serde_json::to_string(meta)
        .map_err(|e| ThaumielError::Storage(format!("failed to serialize metadata: {e}")))
}

fn organization_from_row(row: &Row) -> Result<Organization> {
    Ok(Organization {
        id: OrganizationId(parse_uuid(&col_str(row, "id")?, "id")?),
        name: col_str(row, "name")?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
    })
}

fn product_from_row(row: &Row) -> Result<Product> {
    Ok(Product {
        id: ProductId(parse_uuid(&col_str(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&col_str(row, "org_id")?, "org_id")?),
        name: col_str(row, "name")?,
        default_keygen_backend: col_str(row, "default_keygen_backend")?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
    })
}

fn parse_status(s: &str) -> Result<LicenseStatus> {
    match s {
        "active" => Ok(LicenseStatus::Active),
        "suspended" => Ok(LicenseStatus::Suspended),
        "revoked" => Ok(LicenseStatus::Revoked),
        "expired" => Ok(LicenseStatus::Expired),
        other => Err(ThaumielError::Storage(format!(
            "invalid license status '{other}'"
        ))),
    }
}
fn license_status_str(status: LicenseStatus) -> &'static str {
    match status {
        LicenseStatus::Active => "active",
        LicenseStatus::Suspended => "suspended",
        LicenseStatus::Revoked => "revoked",
        LicenseStatus::Expired => "expired",
    }
}

fn license_from_row(row: &Row) -> Result<LicenseKey> {
    Ok(LicenseKey {
        id: LicenseId(parse_uuid(&col_str(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&col_str(row, "org_id")?, "org_id")?),
        product_id: ProductId(parse_uuid(&col_str(row, "product_id")?, "product_id")?),
        backend_id: col_str(row, "backend_id")?,
        key: col_str(row, "key_value")?,
        status: parse_status(&col_str(row, "status")?)?,
        seats: col_i64(row, "seats")? as u32,
        expires_at: parse_opt_dt(col_opt_str(row, "expires_at")?, "expires_at")?,
        metadata: parse_metadata(&col_str(row, "metadata")?)?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
        revoked_at: parse_opt_dt(col_opt_str(row, "revoked_at")?, "revoked_at")?,
    })
}

fn activation_from_row(row: &Row) -> Result<Activation> {
    Ok(Activation {
        id: ActivationId(parse_uuid(&col_str(row, "id")?, "id")?),
        license_id: LicenseId(parse_uuid(&col_str(row, "license_id")?, "license_id")?),
        machine_fingerprint: col_str(row, "machine_fingerprint")?,
        activated_at: parse_dt(&col_str(row, "activated_at")?, "activated_at")?,
    })
}

fn parse_scope(s: &str) -> Result<ApiKeyScope> {
    match s {
        "admin" => Ok(ApiKeyScope::Admin),
        "license_manager" => Ok(ApiKeyScope::LicenseManager),
        "validate_only" => Ok(ApiKeyScope::ValidateOnly),
        other => Err(ThaumielError::Storage(format!(
            "invalid api key scope '{other}'"
        ))),
    }
}
fn api_key_scope_str(scope: ApiKeyScope) -> &'static str {
    match scope {
        ApiKeyScope::Admin => "admin",
        ApiKeyScope::LicenseManager => "license_manager",
        ApiKeyScope::ValidateOnly => "validate_only",
    }
}

fn api_key_from_row(row: &Row) -> Result<ApiKey> {
    Ok(ApiKey {
        id: ApiKeyId(parse_uuid(&col_str(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&col_str(row, "org_id")?, "org_id")?),
        name: col_str(row, "name")?,
        key_hash: col_str(row, "key_hash")?,
        key_prefix: col_str(row, "key_prefix")?,
        scope: parse_scope(&col_str(row, "scope")?)?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
        last_used_at: parse_opt_dt(col_opt_str(row, "last_used_at")?, "last_used_at")?,
        revoked_at: parse_opt_dt(col_opt_str(row, "revoked_at")?, "revoked_at")?,
    })
}

fn parse_role(s: &str) -> Result<Role> {
    match s {
        "owner" => Ok(Role::Owner),
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        other => Err(ThaumielError::Storage(format!("invalid role '{other}'"))),
    }
}
fn role_str(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Member => "member",
    }
}

fn user_from_row(row: &Row) -> Result<User> {
    Ok(User {
        id: UserId(parse_uuid(&col_str(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&col_str(row, "org_id")?, "org_id")?),
        email: col_str(row, "email")?,
        password_hash: col_opt_str(row, "password_hash")?,
        role: parse_role(&col_str(row, "role")?)?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
    })
}

fn audit_log_from_row(row: &Row) -> Result<AuditLogEntry> {
    Ok(AuditLogEntry {
        id: AuditLogId(parse_uuid(&col_str(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&col_str(row, "org_id")?, "org_id")?),
        actor: col_str(row, "actor")?,
        action: col_str(row, "action")?,
        target: col_str(row, "target")?,
        metadata: parse_metadata(&col_str(row, "metadata")?)?,
        created_at: parse_dt(&col_str(row, "created_at")?, "created_at")?,
    })
}

#[async_trait]
impl Storage for MssqlStorage {
    fn id(&self) -> &'static str {
        "mssql"
    }

    async fn migrate(&self) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        // One batch: every statement in the file is individually idempotent
        // (OBJECT_ID / sys.indexes guarded), so there's no sqlx_migrate-style
        // tracking table needed for this single migration file.
        conn.simple_query(MIGRATION_SQL)
            .await
            .map_err(tiberius_err)?
            .into_results()
            .await
            .map_err(tiberius_err)?;
        Ok(())
    }

    async fn create_organization(&self, org: Organization) -> Result<Organization> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT INTO organizations (id, name, created_at) VALUES (@P1, @P2, @P3)",
            &[&org.id.to_string(), &org.name, &dt_str(org.created_at)],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(org)
    }

    async fn get_organization(&self, id: OrganizationId) -> Result<Organization> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query(
                "SELECT * FROM organizations WHERE id = @P1",
                &[&id.to_string()],
            )
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("organization", id))?;
        organization_from_row(&row)
    }

    async fn list_organizations(&self, page: Pagination) -> Result<Vec<Organization>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM organizations ORDER BY created_at DESC OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY",
                &[&(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(organization_from_row).collect()
    }

    async fn create_product(&self, product: Product) -> Result<Product> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT INTO products (id, org_id, name, default_keygen_backend, created_at) VALUES (@P1, @P2, @P3, @P4, @P5)",
            &[
                &product.id.to_string(),
                &product.org_id.to_string(),
                &product.name,
                &product.default_keygen_backend,
                &dt_str(product.created_at),
            ],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(product)
    }

    async fn get_product(&self, id: ProductId) -> Result<Product> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query("SELECT * FROM products WHERE id = @P1", &[&id.to_string()])
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("product", id))?;
        product_from_row(&row)
    }

    async fn list_products(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<Product>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM products WHERE org_id = @P1 ORDER BY created_at DESC OFFSET @P2 ROWS FETCH NEXT @P3 ROWS ONLY",
                &[&org_id.to_string(), &(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(product_from_row).collect()
    }

    async fn create_license(&self, license: LicenseKey) -> Result<LicenseKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let metadata = metadata_json(&license.metadata)?;
        conn.execute(
            "INSERT INTO license_keys (id, org_id, product_id, backend_id, key_value, status, seats, expires_at, metadata, created_at, revoked_at) \
             VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7, @P8, @P9, @P10, @P11)",
            &[
                &license.id.to_string(),
                &license.org_id.to_string(),
                &license.product_id.to_string(),
                &license.backend_id,
                &license.key,
                &license_status_str(license.status),
                &(license.seats as i64),
                &license.expires_at.map(dt_str),
                &metadata,
                &dt_str(license.created_at),
                &license.revoked_at.map(dt_str),
            ],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(license)
    }

    async fn get_license(&self, id: LicenseId) -> Result<LicenseKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query(
                "SELECT * FROM license_keys WHERE id = @P1",
                &[&id.to_string()],
            )
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("license", id))?;
        license_from_row(&row)
    }

    async fn get_license_by_key(&self, key: &str) -> Result<LicenseKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query("SELECT * FROM license_keys WHERE key_value = @P1", &[&key])
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("license", key))?;
        license_from_row(&row)
    }

    async fn list_licenses(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<LicenseKey>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM license_keys WHERE org_id = @P1 ORDER BY created_at DESC OFFSET @P2 ROWS FETCH NEXT @P3 ROWS ONLY",
                &[&org_id.to_string(), &(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(license_from_row).collect()
    }

    async fn set_license_status(&self, id: LicenseId, status: LicenseStatus) -> Result<LicenseKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let revoked_at = matches!(status, LicenseStatus::Revoked).then(now_str);
        conn.execute(
            "UPDATE license_keys SET status = @P1, revoked_at = COALESCE(@P2, revoked_at) WHERE id = @P3",
            &[&license_status_str(status), &revoked_at, &id.to_string()],
        )
        .await
        .map_err(tiberius_err)?;
        drop(conn);
        self.get_license(id).await
    }

    async fn create_activation(&self, activation: Activation) -> Result<Activation> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT INTO activations (id, license_id, machine_fingerprint, activated_at) VALUES (@P1, @P2, @P3, @P4)",
            &[&activation.id.to_string(), &activation.license_id.to_string(), &activation.machine_fingerprint, &dt_str(activation.activated_at)],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(activation)
    }

    async fn count_activations(&self, license_id: LicenseId) -> Result<u32> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query(
                "SELECT COUNT(*) AS c FROM activations WHERE license_id = @P1",
                &[&license_id.to_string()],
            )
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| ThaumielError::Storage("COUNT(*) returned no row".into()))?;
        let count: i32 = row.try_get("c").map_err(tiberius_err)?.unwrap_or(0);
        Ok(count as u32)
    }

    async fn list_activations(&self, license_id: LicenseId) -> Result<Vec<Activation>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM activations WHERE license_id = @P1 ORDER BY activated_at DESC",
                &[&license_id.to_string()],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(activation_from_row).collect()
    }

    async fn delete_activation(
        &self,
        license_id: LicenseId,
        activation_id: ActivationId,
    ) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM activations WHERE id = @P1 AND license_id = @P2",
            &[&activation_id.to_string(), &license_id.to_string()],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(())
    }

    async fn create_api_key(&self, key: ApiKey) -> Result<ApiKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT INTO api_keys (id, org_id, name, key_hash, key_prefix, scope, created_at, last_used_at, revoked_at) \
             VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7, @P8, @P9)",
            &[
                &key.id.to_string(),
                &key.org_id.to_string(),
                &key.name,
                &key.key_hash,
                &key.key_prefix,
                &api_key_scope_str(key.scope),
                &dt_str(key.created_at),
                &key.last_used_at.map(dt_str),
                &key.revoked_at.map(dt_str),
            ],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(key)
    }

    async fn get_api_key_by_prefix(&self, prefix: &str) -> Result<ApiKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query("SELECT * FROM api_keys WHERE key_prefix = @P1", &[&prefix])
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("api_key", prefix))?;
        api_key_from_row(&row)
    }

    async fn list_api_keys(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<ApiKey>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM api_keys WHERE org_id = @P1 ORDER BY created_at DESC OFFSET @P2 ROWS FETCH NEXT @P3 ROWS ONLY",
                &[&org_id.to_string(), &(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(api_key_from_row).collect()
    }

    async fn revoke_api_key(&self, id: ApiKeyId) -> Result<ApiKey> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "UPDATE api_keys SET revoked_at = @P1 WHERE id = @P2",
            &[&now_str(), &id.to_string()],
        )
        .await
        .map_err(tiberius_err)?;
        let row = conn
            .query("SELECT * FROM api_keys WHERE id = @P1", &[&id.to_string()])
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("api_key", id))?;
        api_key_from_row(&row)
    }

    async fn touch_api_key_last_used(&self, id: ApiKeyId) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "UPDATE api_keys SET last_used_at = @P1 WHERE id = @P2",
            &[&now_str(), &id.to_string()],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(())
    }

    async fn create_user(&self, user: User) -> Result<User> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (id, org_id, email, password_hash, role, created_at) VALUES (@P1, @P2, @P3, @P4, @P5, @P6)",
            &[
                &user.id.to_string(),
                &user.org_id.to_string(),
                &user.email,
                &user.password_hash,
                &role_str(user.role),
                &dt_str(user.created_at),
            ],
        )
        .await
        .map_err(|e| {
            // Tiberius surfaces a unique-constraint violation as a generic
            // server error; matching on the message is the only option
            // without a documented error-code accessor on this version.
            if e.to_string().contains("UNIQUE") || e.to_string().contains("duplicate") {
                ThaumielError::Conflict(format!("user '{}' already exists", user.email))
            } else {
                tiberius_err(e)
            }
        })?;
        Ok(user)
    }

    async fn get_user_by_email(&self, org_id: OrganizationId, email: &str) -> Result<User> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query(
                "SELECT * FROM users WHERE org_id = @P1 AND email = @P2",
                &[&org_id.to_string(), &email],
            )
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("user", email))?;
        user_from_row(&row)
    }

    async fn get_user(&self, id: UserId) -> Result<User> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let row = conn
            .query("SELECT * FROM users WHERE id = @P1", &[&id.to_string()])
            .await
            .map_err(tiberius_err)?
            .into_row()
            .await
            .map_err(tiberius_err)?
            .ok_or_else(|| not_found("user", id))?;
        user_from_row(&row)
    }

    async fn list_users(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<User>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM users WHERE org_id = @P1 ORDER BY created_at ASC OFFSET @P2 ROWS FETCH NEXT @P3 ROWS ONLY",
                &[&org_id.to_string(), &(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(user_from_row).collect()
    }

    async fn append_audit_log(&self, entry: AuditLogEntry) -> Result<AuditLogEntry> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let metadata = metadata_json(&entry.metadata)?;
        conn.execute(
            "INSERT INTO audit_log (id, org_id, actor, action, target, metadata, created_at) VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7)",
            &[&entry.id.to_string(), &entry.org_id.to_string(), &entry.actor, &entry.action, &entry.target, &metadata, &dt_str(entry.created_at)],
        )
        .await
        .map_err(tiberius_err)?;
        Ok(entry)
    }

    async fn list_audit_log(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<AuditLogEntry>> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        let rows = conn
            .query(
                "SELECT * FROM audit_log WHERE org_id = @P1 ORDER BY created_at DESC OFFSET @P2 ROWS FETCH NEXT @P3 ROWS ONLY",
                &[&org_id.to_string(), &(page.offset as i64), &(page.limit as i64)],
            )
            .await
            .map_err(tiberius_err)?
            .into_first_result()
            .await
            .map_err(tiberius_err)?;
        rows.iter().map(audit_log_from_row).collect()
    }
}
