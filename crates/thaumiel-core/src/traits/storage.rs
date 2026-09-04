use async_trait::async_trait;

use crate::error::Result;
use crate::ids::{ActivationId, ApiKeyId, LicenseId, OrganizationId, ProductId, UserId};
use crate::models::{
    Activation, ApiKey, AuditLogEntry, LicenseKey, LicenseStatus, Organization, Product, User,
};

#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub limit: u32,
    pub offset: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { limit: 50, offset: 0 }
    }
}

/// Persistence abstraction implemented once per database backend
/// (`thaumiel-storage`'s `postgres`, `mysql`, `sqlite`, and `memory` modules).
///
/// Every method is intentionally storage-agnostic: no SQL, no backend-specific
/// types leak through this boundary, so `thaumiel-server` and every plugin can
/// depend on `Arc<dyn Storage>` without caring which database is behind it.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Stable identifier for this backend, e.g. `"postgres"`, `"mysql"`, `"sqlite"`.
    fn id(&self) -> &'static str;

    /// Run pending schema migrations. Called once at startup.
    async fn migrate(&self) -> Result<()>;

    // -- organizations ---------------------------------------------------
    async fn create_organization(&self, org: Organization) -> Result<Organization>;
    async fn get_organization(&self, id: OrganizationId) -> Result<Organization>;
    async fn list_organizations(&self, page: Pagination) -> Result<Vec<Organization>>;

    // -- products ----------------------------------------------------------
    async fn create_product(&self, product: Product) -> Result<Product>;
    async fn get_product(&self, id: ProductId) -> Result<Product>;
    async fn list_products(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<Product>>;

    // -- licenses ------------------------------------------------------------
    async fn create_license(&self, license: LicenseKey) -> Result<LicenseKey>;
    async fn get_license(&self, id: LicenseId) -> Result<LicenseKey>;
    /// Look up a license by its exact key string. Required by DB-lookup keygen
    /// backends (e.g. opaque tokens); offline-verifiable backends may still use
    /// this for revocation checks.
    async fn get_license_by_key(&self, key: &str) -> Result<LicenseKey>;
    async fn list_licenses(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<LicenseKey>>;
    async fn set_license_status(&self, id: LicenseId, status: LicenseStatus) -> Result<LicenseKey>;

    // -- activations (seat tracking) -----------------------------------------
    async fn create_activation(&self, activation: Activation) -> Result<Activation>;
    async fn count_activations(&self, license_id: LicenseId) -> Result<u32>;
    async fn list_activations(&self, license_id: LicenseId) -> Result<Vec<Activation>>;
    /// Frees a seat without revoking the whole license. `license_id` is
    /// required (not just `activation_id`) so a caller can't free a seat on
    /// a license outside whatever org-ownership check they've already done.
    async fn delete_activation(&self, license_id: LicenseId, activation_id: ActivationId) -> Result<()>;

    // -- api keys --------------------------------------------------------------
    async fn create_api_key(&self, key: ApiKey) -> Result<ApiKey>;
    /// Look up by the short, non-secret prefix; the caller still verifies the
    /// full secret against `key_hash` before trusting the result.
    async fn get_api_key_by_prefix(&self, prefix: &str) -> Result<ApiKey>;
    async fn list_api_keys(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<ApiKey>>;
    async fn revoke_api_key(&self, id: ApiKeyId) -> Result<ApiKey>;
    async fn touch_api_key_last_used(&self, id: ApiKeyId) -> Result<()>;

    // -- users -------------------------------------------------------------------
    async fn create_user(&self, user: User) -> Result<User>;
    async fn get_user_by_email(&self, org_id: OrganizationId, email: &str) -> Result<User>;
    async fn get_user(&self, id: UserId) -> Result<User>;
    async fn list_users(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<User>>;

    // -- audit log -----------------------------------------------------------------
    async fn append_audit_log(&self, entry: AuditLogEntry) -> Result<AuditLogEntry>;
    async fn list_audit_log(&self, org_id: OrganizationId, page: Pagination) -> Result<Vec<AuditLogEntry>>;
}
