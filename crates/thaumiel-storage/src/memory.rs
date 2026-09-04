//! Process-local, non-persistent [`Storage`] implementation. Backs unit tests
//! and `cargo test --workspace` (no external database needed) and doubles as
//! a `backend = "memory"` option for quick demos.

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

use thaumiel_core::ids::{ApiKeyId, LicenseId, OrganizationId, ProductId, UserId};
use thaumiel_core::models::{Activation, ApiKey, AuditLogEntry, LicenseKey, LicenseStatus, Organization, Product, User};
use thaumiel_core::traits::{Pagination, Storage};
use thaumiel_core::{Result, ThaumielError};

#[derive(Default)]
struct Tables {
    organizations: HashMap<OrganizationId, Organization>,
    products: HashMap<ProductId, Product>,
    licenses: HashMap<LicenseId, LicenseKey>,
    activations: HashMap<thaumiel_core::ids::ActivationId, Activation>,
    api_keys: HashMap<ApiKeyId, ApiKey>,
    users: HashMap<UserId, User>,
    audit_log: HashMap<thaumiel_core::ids::AuditLogId, AuditLogEntry>,
}

pub struct InMemoryStorage {
    tables: RwLock<Tables>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self { tables: RwLock::new(Tables::default()) }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

fn paginate<T: Clone>(mut items: Vec<T>, page: Pagination, sort_key: impl Fn(&T) -> std::cmp::Reverse<chrono::DateTime<chrono::Utc>>) -> Vec<T> {
    items.sort_by_key(sort_key);
    items.into_iter().skip(page.offset as usize).take(page.limit as usize).collect()
}

#[async_trait]
impl Storage for InMemoryStorage {
    fn id(&self) -> &'static str {
        "memory"
    }

    async fn migrate(&self) -> Result<()> {
        Ok(())
    }

    async fn create_organization(&self, org: Organization) -> Result<Organization> {
        let mut t = self.tables.write().await;
        t.organizations.insert(org.id, org.clone());
        Ok(org)
    }

    async fn get_organization(&self, id: OrganizationId) -> Result<Organization> {
        let t = self.tables.read().await;
        t.organizations.get(&id).cloned().ok_or_else(|| ThaumielError::NotFound(format!("organization '{id}'")))
    }

    async fn list_organizations(&self, page: Pagination) -> Result<Vec<Organization>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.organizations.values().cloned().collect();
        Ok(paginate(items, page, |o| std::cmp::Reverse(o.created_at)))
    }

    async fn create_product(&self, product: Product) -> Result<Product> {
        let mut t = self.tables.write().await;
        t.products.insert(product.id, product.clone());
        Ok(product)
    }

    async fn get_product(&self, id: ProductId) -> Result<Product> {
        let t = self.tables.read().await;
        t.products.get(&id).cloned().ok_or_else(|| ThaumielError::NotFound(format!("product '{id}'")))
    }

    async fn list_products(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<Product>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.products.values().filter(|p| p.org_id == org_id).cloned().collect();
        Ok(paginate(items, page, |p| std::cmp::Reverse(p.created_at)))
    }

    async fn create_license(&self, license: LicenseKey) -> Result<LicenseKey> {
        let mut t = self.tables.write().await;
        t.licenses.insert(license.id, license.clone());
        Ok(license)
    }

    async fn get_license(&self, id: LicenseId) -> Result<LicenseKey> {
        let t = self.tables.read().await;
        t.licenses.get(&id).cloned().ok_or_else(|| ThaumielError::NotFound(format!("license '{id}'")))
    }

    async fn get_license_by_key(&self, key: &str) -> Result<LicenseKey> {
        let t = self.tables.read().await;
        t.licenses
            .values()
            .find(|l| l.key == key)
            .cloned()
            .ok_or_else(|| ThaumielError::NotFound(format!("license '{key}'")))
    }

    async fn list_licenses(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<LicenseKey>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.licenses.values().filter(|l| l.org_id == org_id).cloned().collect();
        Ok(paginate(items, page, |l| std::cmp::Reverse(l.created_at)))
    }

    async fn set_license_status(&self, id: LicenseId, status: LicenseStatus) -> Result<LicenseKey> {
        let mut t = self.tables.write().await;
        let license = t
            .licenses
            .get_mut(&id)
            .ok_or_else(|| ThaumielError::NotFound(format!("license '{id}'")))?;
        license.status = status;
        if status == LicenseStatus::Revoked {
            license.revoked_at = Some(chrono::Utc::now());
        }
        Ok(license.clone())
    }

    async fn create_activation(&self, activation: Activation) -> Result<Activation> {
        let mut t = self.tables.write().await;
        t.activations.insert(activation.id, activation.clone());
        Ok(activation)
    }

    async fn count_activations(&self, license_id: LicenseId) -> Result<u32> {
        let t = self.tables.read().await;
        Ok(t.activations.values().filter(|a| a.license_id == license_id).count() as u32)
    }

    async fn list_activations(&self, license_id: LicenseId) -> Result<Vec<Activation>> {
        let t = self.tables.read().await;
        Ok(t.activations.values().filter(|a| a.license_id == license_id).cloned().collect())
    }

    async fn delete_activation(&self, license_id: LicenseId, activation_id: thaumiel_core::ids::ActivationId) -> Result<()> {
        let mut t = self.tables.write().await;
        if matches!(t.activations.get(&activation_id), Some(a) if a.license_id == license_id) {
            t.activations.remove(&activation_id);
        }
        Ok(())
    }

    async fn create_api_key(&self, key: ApiKey) -> Result<ApiKey> {
        let mut t = self.tables.write().await;
        t.api_keys.insert(key.id, key.clone());
        Ok(key)
    }

    async fn get_api_key_by_prefix(&self, prefix: &str) -> Result<ApiKey> {
        let t = self.tables.read().await;
        t.api_keys
            .values()
            .find(|k| k.key_prefix == prefix)
            .cloned()
            .ok_or_else(|| ThaumielError::NotFound(format!("api_key '{prefix}'")))
    }

    async fn list_api_keys(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<ApiKey>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.api_keys.values().filter(|k| k.org_id == org_id).cloned().collect();
        Ok(paginate(items, page, |k| std::cmp::Reverse(k.created_at)))
    }

    async fn revoke_api_key(&self, id: ApiKeyId) -> Result<ApiKey> {
        let mut t = self.tables.write().await;
        let key = t.api_keys.get_mut(&id).ok_or_else(|| ThaumielError::NotFound(format!("api_key '{id}'")))?;
        key.revoked_at = Some(chrono::Utc::now());
        Ok(key.clone())
    }

    async fn touch_api_key_last_used(&self, id: ApiKeyId) -> Result<()> {
        let mut t = self.tables.write().await;
        if let Some(key) = t.api_keys.get_mut(&id) {
            key.last_used_at = Some(chrono::Utc::now());
        }
        Ok(())
    }

    async fn create_user(&self, user: User) -> Result<User> {
        let mut t = self.tables.write().await;
        if t.users.values().any(|u| u.org_id == user.org_id && u.email == user.email) {
            return Err(ThaumielError::Conflict(format!("user '{}' already exists", user.email)));
        }
        t.users.insert(user.id, user.clone());
        Ok(user)
    }

    async fn get_user_by_email(&self, org_id: OrganizationId, email: &str) -> Result<User> {
        let t = self.tables.read().await;
        t.users
            .values()
            .find(|u| u.org_id == org_id && u.email == email)
            .cloned()
            .ok_or_else(|| ThaumielError::NotFound(format!("user '{email}'")))
    }

    async fn get_user(&self, id: UserId) -> Result<User> {
        let t = self.tables.read().await;
        t.users.get(&id).cloned().ok_or_else(|| ThaumielError::NotFound(format!("user '{id}'")))
    }

    async fn list_users(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<User>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.users.values().filter(|u| u.org_id == org_id).cloned().collect();
        Ok(paginate(items, page, |u| std::cmp::Reverse(u.created_at)))
    }

    async fn append_audit_log(&self, entry: AuditLogEntry) -> Result<AuditLogEntry> {
        let mut t = self.tables.write().await;
        t.audit_log.insert(entry.id, entry.clone());
        Ok(entry)
    }

    async fn list_audit_log(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<AuditLogEntry>> {
        let t = self.tables.read().await;
        let items: Vec<_> = t.audit_log.values().filter(|e| e.org_id == org_id).cloned().collect();
        Ok(paginate(items, page, |e| std::cmp::Reverse(e.created_at)))
    }
}
