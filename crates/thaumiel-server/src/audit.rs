use thaumiel_core::ids::{AuditLogId, OrganizationId};
use thaumiel_core::models::AuditLogEntry;

use crate::state::AppState;

/// Best-effort audit trail write: logged and swallowed on failure so a
/// storage hiccup on the audit log never fails the request that triggered it.
pub async fn record(
    state: &AppState,
    org_id: OrganizationId,
    actor: impl Into<String>,
    action: &str,
    target: impl Into<String>,
) {
    let entry = AuditLogEntry {
        id: AuditLogId::new(),
        org_id,
        actor: actor.into(),
        action: action.to_string(),
        target: target.into(),
        metadata: Default::default(),
        created_at: chrono::Utc::now(),
    };
    if let Err(e) = state.storage.append_audit_log(entry).await {
        tracing::warn!(error = %e, %action, "failed to append audit log entry");
    }
}
