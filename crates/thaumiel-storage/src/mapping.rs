//! Row -> domain-type mapping shared by every `sqlx` backend.
//!
//! Every column in every backend's schema is TEXT/VARCHAR (see
//! `migrations/*/0001_init.sql`) except `seats`/`COUNT(*)`, which are wide
//! integers. That means the exact same generic decode logic works whether
//! `R` is a `PgRow`, `MySqlRow`, or `SqliteRow` -- only the SQL query text
//! (placeholder syntax) differs between `postgres.rs`, `mysql.rs`, and
//! `sqlite.rs`. Keeping the mapping here means a schema/field change only
//! needs to be taught to one function, not three.

use chrono::{DateTime, Utc};
use sqlx::{ColumnIndex, Decode, Row, Type};
use std::collections::HashMap;
use uuid::Uuid;

use thaumiel_core::ids::{
    ActivationId, ApiKeyId, AuditLogId, LicenseId, OrganizationId, ProductId, UserId,
};
use thaumiel_core::models::{
    Activation, ApiKey, ApiKeyScope, AuditLogEntry, LicenseKey, LicenseStatus, Organization,
    Product, Role, User,
};
use thaumiel_core::{Result, ThaumielError};

fn get_string<'r, R>(row: &'r R, col: &'static str) -> Result<String>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    row.try_get(col)
        .map_err(|e| ThaumielError::Storage(format!("column '{col}': {e}")))
}

fn get_opt_string<'r, R>(row: &'r R, col: &'static str) -> Result<Option<String>>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    row.try_get(col)
        .map_err(|e| ThaumielError::Storage(format!("column '{col}': {e}")))
}

fn get_i64<'r, R>(row: &'r R, col: &'static str) -> Result<i64>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    i64: Decode<'r, R::Database> + Type<R::Database>,
{
    row.try_get(col)
        .map_err(|e| ThaumielError::Storage(format!("column '{col}': {e}")))
}

fn parse_uuid(s: &str, field: &'static str) -> Result<Uuid> {
    Uuid::parse_str(s)
        .map_err(|e| ThaumielError::Storage(format!("invalid uuid in '{field}': {e}")))
}

fn parse_dt(s: &str, field: &'static str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ThaumielError::Storage(format!("invalid timestamp in '{field}': {e}")))
}

fn parse_opt_dt(s: Option<String>, field: &'static str) -> Result<Option<DateTime<Utc>>> {
    s.map(|s| parse_dt(&s, field)).transpose()
}

fn parse_metadata(s: &str) -> Result<HashMap<String, String>> {
    if s.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str(s)
        .map_err(|e| ThaumielError::Storage(format!("invalid metadata json: {e}")))
}

pub fn organization_from_row<'r, R>(row: &'r R) -> Result<Organization>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(Organization {
        id: OrganizationId(parse_uuid(&get_string(row, "id")?, "id")?),
        name: get_string(row, "name")?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
    })
}

pub fn product_from_row<'r, R>(row: &'r R) -> Result<Product>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(Product {
        id: ProductId(parse_uuid(&get_string(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&get_string(row, "org_id")?, "org_id")?),
        name: get_string(row, "name")?,
        default_keygen_backend: get_string(row, "default_keygen_backend")?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
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

pub fn license_status_str(status: LicenseStatus) -> &'static str {
    match status {
        LicenseStatus::Active => "active",
        LicenseStatus::Suspended => "suspended",
        LicenseStatus::Revoked => "revoked",
        LicenseStatus::Expired => "expired",
    }
}

pub fn license_from_row<'r, R>(row: &'r R) -> Result<LicenseKey>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
    i64: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(LicenseKey {
        id: LicenseId(parse_uuid(&get_string(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&get_string(row, "org_id")?, "org_id")?),
        product_id: ProductId(parse_uuid(&get_string(row, "product_id")?, "product_id")?),
        backend_id: get_string(row, "backend_id")?,
        key: get_string(row, "key_value")?,
        status: parse_status(&get_string(row, "status")?)?,
        seats: get_i64(row, "seats")? as u32,
        expires_at: parse_opt_dt(get_opt_string(row, "expires_at")?, "expires_at")?,
        metadata: parse_metadata(&get_string(row, "metadata")?)?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
        revoked_at: parse_opt_dt(get_opt_string(row, "revoked_at")?, "revoked_at")?,
    })
}

pub fn activation_from_row<'r, R>(row: &'r R) -> Result<Activation>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(Activation {
        id: ActivationId(parse_uuid(&get_string(row, "id")?, "id")?),
        license_id: LicenseId(parse_uuid(&get_string(row, "license_id")?, "license_id")?),
        machine_fingerprint: get_string(row, "machine_fingerprint")?,
        activated_at: parse_dt(&get_string(row, "activated_at")?, "activated_at")?,
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

pub fn api_key_scope_str(scope: ApiKeyScope) -> &'static str {
    match scope {
        ApiKeyScope::Admin => "admin",
        ApiKeyScope::LicenseManager => "license_manager",
        ApiKeyScope::ValidateOnly => "validate_only",
    }
}

pub fn api_key_from_row<'r, R>(row: &'r R) -> Result<ApiKey>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(ApiKey {
        id: ApiKeyId(parse_uuid(&get_string(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&get_string(row, "org_id")?, "org_id")?),
        name: get_string(row, "name")?,
        key_hash: get_string(row, "key_hash")?,
        key_prefix: get_string(row, "key_prefix")?,
        scope: parse_scope(&get_string(row, "scope")?)?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
        last_used_at: parse_opt_dt(get_opt_string(row, "last_used_at")?, "last_used_at")?,
        revoked_at: parse_opt_dt(get_opt_string(row, "revoked_at")?, "revoked_at")?,
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

pub fn role_str(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Member => "member",
    }
}

pub fn user_from_row<'r, R>(row: &'r R) -> Result<User>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(User {
        id: UserId(parse_uuid(&get_string(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&get_string(row, "org_id")?, "org_id")?),
        email: get_string(row, "email")?,
        password_hash: get_opt_string(row, "password_hash")?,
        role: parse_role(&get_string(row, "role")?)?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
    })
}

pub fn audit_log_from_row<'r, R>(row: &'r R) -> Result<AuditLogEntry>
where
    R: Row,
    &'static str: ColumnIndex<R>,
    String: Decode<'r, R::Database> + Type<R::Database>,
{
    Ok(AuditLogEntry {
        id: AuditLogId(parse_uuid(&get_string(row, "id")?, "id")?),
        org_id: OrganizationId(parse_uuid(&get_string(row, "org_id")?, "org_id")?),
        actor: get_string(row, "actor")?,
        action: get_string(row, "action")?,
        target: get_string(row, "target")?,
        metadata: parse_metadata(&get_string(row, "metadata")?)?,
        created_at: parse_dt(&get_string(row, "created_at")?, "created_at")?,
    })
}

pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}

pub fn dt_str(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

pub fn metadata_json(meta: &HashMap<String, String>) -> Result<String> {
    serde_json::to_string(meta)
        .map_err(|e| ThaumielError::Storage(format!("failed to serialize metadata: {e}")))
}

/// Small helper so backend modules can bubble up "row not found" consistently.
pub fn not_found(entity: &str, ident: impl std::fmt::Display) -> ThaumielError {
    ThaumielError::NotFound(format!("{entity} '{ident}' not found"))
}

/// Render an `Id` newtype (or any `Display`) for use in a `WHERE id = ?` bind
/// -- trivial, but centralizes the pattern so call sites read consistently.
pub fn uuid_str(s: impl std::fmt::Display) -> String {
    s.to_string()
}
