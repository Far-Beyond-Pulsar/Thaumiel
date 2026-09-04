use async_trait::async_trait;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySqlPool, Row};

use thaumiel_core::ids::{ApiKeyId, LicenseId, OrganizationId, ProductId, UserId};
use thaumiel_core::models::{
    Activation, ApiKey, AuditLogEntry, LicenseKey, LicenseStatus, Organization, Product, User,
};
use thaumiel_core::traits::{Pagination, Storage};
use thaumiel_core::{Result, ThaumielError};

use crate::mapping::*;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations/mysql");

pub struct MySqlStorage {
    pool: MySqlPool,
}

impl MySqlStorage {
    pub async fn connect(url: &str, max_connections: u32) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max_connections)
            .connect(url)
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))?;
        Ok(Self { pool })
    }
}

fn sqlx_err(e: sqlx::Error) -> ThaumielError {
    ThaumielError::Storage(e.to_string())
}

#[async_trait]
impl Storage for MySqlStorage {
    fn id(&self) -> &'static str {
        "mysql"
    }

    async fn migrate(&self) -> Result<()> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|e| ThaumielError::Storage(e.to_string()))
    }

    async fn create_organization(&self, org: Organization) -> Result<Organization> {
        sqlx::query("INSERT INTO organizations (id, name, created_at) VALUES (?, ?, ?)")
            .bind(uuid_str(org.id))
            .bind(&org.name)
            .bind(dt_str(org.created_at))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
        Ok(org)
    }

    async fn get_organization(&self, id: OrganizationId) -> Result<Organization> {
        let row = sqlx::query("SELECT * FROM organizations WHERE id = ?")
            .bind(uuid_str(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("organization", id))?;
        organization_from_row(&row)
    }

    async fn list_organizations(&self, page: Pagination) -> Result<Vec<Organization>> {
        let rows =
            sqlx::query("SELECT * FROM organizations ORDER BY created_at DESC LIMIT ? OFFSET ?")
                .bind(page.limit as i64)
                .bind(page.offset as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(sqlx_err)?;
        rows.iter().map(organization_from_row).collect()
    }

    async fn create_product(&self, product: Product) -> Result<Product> {
        sqlx::query(
            "INSERT INTO products (id, org_id, name, default_keygen_backend, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(product.id))
        .bind(uuid_str(product.org_id))
        .bind(&product.name)
        .bind(&product.default_keygen_backend)
        .bind(dt_str(product.created_at))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err)?;
        Ok(product)
    }

    async fn get_product(&self, id: ProductId) -> Result<Product> {
        let row = sqlx::query("SELECT * FROM products WHERE id = ?")
            .bind(uuid_str(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("product", id))?;
        product_from_row(&row)
    }

    async fn list_products(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            "SELECT * FROM products WHERE org_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(org_id))
        .bind(page.limit as i64)
        .bind(page.offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(product_from_row).collect()
    }

    async fn create_license(&self, license: LicenseKey) -> Result<LicenseKey> {
        sqlx::query(
            "INSERT INTO license_keys (id, org_id, product_id, backend_id, key_value, status, seats, expires_at, metadata, created_at, revoked_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(license.id))
        .bind(uuid_str(license.org_id))
        .bind(uuid_str(license.product_id))
        .bind(&license.backend_id)
        .bind(&license.key)
        .bind(license_status_str(license.status))
        .bind(license.seats as i64)
        .bind(license.expires_at.map(dt_str))
        .bind(metadata_json(&license.metadata)?)
        .bind(dt_str(license.created_at))
        .bind(license.revoked_at.map(dt_str))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err)?;
        Ok(license)
    }

    async fn get_license(&self, id: LicenseId) -> Result<LicenseKey> {
        let row = sqlx::query("SELECT * FROM license_keys WHERE id = ?")
            .bind(uuid_str(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("license", id))?;
        license_from_row(&row)
    }

    async fn get_license_by_key(&self, key: &str) -> Result<LicenseKey> {
        let row = sqlx::query("SELECT * FROM license_keys WHERE key_value = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("license", key))?;
        license_from_row(&row)
    }

    async fn list_licenses(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<LicenseKey>> {
        let rows = sqlx::query(
            "SELECT * FROM license_keys WHERE org_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(org_id))
        .bind(page.limit as i64)
        .bind(page.offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(license_from_row).collect()
    }

    async fn set_license_status(&self, id: LicenseId, status: LicenseStatus) -> Result<LicenseKey> {
        let revoked_at = matches!(status, LicenseStatus::Revoked).then(now_str);
        sqlx::query(
            "UPDATE license_keys SET status = ?, revoked_at = COALESCE(?, revoked_at) WHERE id = ?",
        )
        .bind(license_status_str(status))
        .bind(revoked_at)
        .bind(uuid_str(id))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err)?;
        self.get_license(id).await
    }

    async fn create_activation(&self, activation: Activation) -> Result<Activation> {
        sqlx::query("INSERT INTO activations (id, license_id, machine_fingerprint, activated_at) VALUES (?, ?, ?, ?)")
            .bind(uuid_str(activation.id))
            .bind(uuid_str(activation.license_id))
            .bind(&activation.machine_fingerprint)
            .bind(dt_str(activation.activated_at))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
        Ok(activation)
    }

    async fn count_activations(&self, license_id: LicenseId) -> Result<u32> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM activations WHERE license_id = ?")
            .bind(uuid_str(license_id))
            .fetch_one(&self.pool)
            .await
            .map_err(sqlx_err)?;
        let count: i64 = row.try_get("c").map_err(sqlx_err)?;
        Ok(count as u32)
    }

    async fn list_activations(&self, license_id: LicenseId) -> Result<Vec<Activation>> {
        let rows = sqlx::query(
            "SELECT * FROM activations WHERE license_id = ? ORDER BY activated_at DESC",
        )
        .bind(uuid_str(license_id))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(activation_from_row).collect()
    }

    async fn delete_activation(
        &self,
        license_id: LicenseId,
        activation_id: thaumiel_core::ids::ActivationId,
    ) -> Result<()> {
        sqlx::query("DELETE FROM activations WHERE id = ? AND license_id = ?")
            .bind(uuid_str(activation_id))
            .bind(uuid_str(license_id))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
        Ok(())
    }

    async fn create_api_key(&self, key: ApiKey) -> Result<ApiKey> {
        sqlx::query(
            "INSERT INTO api_keys (id, org_id, name, key_hash, key_prefix, scope, created_at, last_used_at, revoked_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(key.id))
        .bind(uuid_str(key.org_id))
        .bind(&key.name)
        .bind(&key.key_hash)
        .bind(&key.key_prefix)
        .bind(api_key_scope_str(key.scope))
        .bind(dt_str(key.created_at))
        .bind(key.last_used_at.map(dt_str))
        .bind(key.revoked_at.map(dt_str))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err)?;
        Ok(key)
    }

    async fn get_api_key_by_prefix(&self, prefix: &str) -> Result<ApiKey> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE key_prefix = ?")
            .bind(prefix)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("api_key", prefix))?;
        api_key_from_row(&row)
    }

    async fn list_api_keys(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<ApiKey>> {
        let rows = sqlx::query(
            "SELECT * FROM api_keys WHERE org_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(org_id))
        .bind(page.limit as i64)
        .bind(page.offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(api_key_from_row).collect()
    }

    async fn revoke_api_key(&self, id: ApiKeyId) -> Result<ApiKey> {
        sqlx::query("UPDATE api_keys SET revoked_at = ? WHERE id = ?")
            .bind(now_str())
            .bind(uuid_str(id))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
        let row = sqlx::query("SELECT * FROM api_keys WHERE id = ?")
            .bind(uuid_str(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("api_key", id))?;
        api_key_from_row(&row)
    }

    async fn touch_api_key_last_used(&self, id: ApiKeyId) -> Result<()> {
        sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
            .bind(now_str())
            .bind(uuid_str(id))
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
        Ok(())
    }

    async fn create_user(&self, user: User) -> Result<User> {
        sqlx::query(
            "INSERT INTO users (id, org_id, email, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(user.id))
        .bind(uuid_str(user.org_id))
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(role_str(user.role))
        .bind(dt_str(user.created_at))
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e {
                if db.is_unique_violation() {
                    return ThaumielError::Conflict(format!("user '{}' already exists", user.email));
                }
            }
            sqlx_err(e)
        })?;
        Ok(user)
    }

    async fn get_user_by_email(&self, org_id: OrganizationId, email: &str) -> Result<User> {
        let row = sqlx::query("SELECT * FROM users WHERE org_id = ? AND email = ?")
            .bind(uuid_str(org_id))
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("user", email))?;
        user_from_row(&row)
    }

    async fn get_user(&self, id: UserId) -> Result<User> {
        let row = sqlx::query("SELECT * FROM users WHERE id = ?")
            .bind(uuid_str(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_err)?
            .ok_or_else(|| not_found("user", id))?;
        user_from_row(&row)
    }

    async fn list_users(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT * FROM users WHERE org_id = ? ORDER BY created_at ASC LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(org_id))
        .bind(page.limit as i64)
        .bind(page.offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(user_from_row).collect()
    }

    async fn append_audit_log(&self, entry: AuditLogEntry) -> Result<AuditLogEntry> {
        sqlx::query(
            "INSERT INTO audit_log (id, org_id, actor, action, target, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(entry.id))
        .bind(uuid_str(entry.org_id))
        .bind(&entry.actor)
        .bind(&entry.action)
        .bind(&entry.target)
        .bind(metadata_json(&entry.metadata)?)
        .bind(dt_str(entry.created_at))
        .execute(&self.pool)
        .await
        .map_err(sqlx_err)?;
        Ok(entry)
    }

    async fn list_audit_log(
        &self,
        org_id: OrganizationId,
        page: Pagination,
    ) -> Result<Vec<AuditLogEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM audit_log WHERE org_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(org_id))
        .bind(page.limit as i64)
        .bind(page.offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_err)?;
        rows.iter().map(audit_log_from_row).collect()
    }
}
