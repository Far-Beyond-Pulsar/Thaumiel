use axum::extract::State;
use axum::Json;

use crate::dto::KeygenBackendInfo;
use crate::state::AppState;

/// Public (no auth) so a product's client SDK can discover which backends
/// exist / which are offline-verifiable without needing credentials.
pub async fn list(State(state): State<AppState>) -> Json<Vec<KeygenBackendInfo>> {
    let mut backends: Vec<_> = state
        .keygen
        .iter()
        .map(|b| KeygenBackendInfo { id: b.id(), description: b.description(), offline_verifiable: b.offline_verifiable() })
        .collect();
    backends.sort_by_key(|b| b.id);
    Json(backends)
}
