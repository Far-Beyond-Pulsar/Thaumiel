use axum::extract::State;
use axum::Json;

use thaumiel_core::models::LicenseStatus;
use thaumiel_core::traits::Pagination;

use crate::dto::UsageSummary;
use crate::error::ApiResult;
use crate::extractors::AdminAuth;
use crate::state::AppState;
use crate::usage;

const COUNT_CAP: u32 = 200;

pub async fn summary(
    State(state): State<AppState>,
    AdminAuth(identity): AdminAuth,
) -> ApiResult<Json<UsageSummary>> {
    let page = Pagination {
        limit: COUNT_CAP,
        offset: 0,
    };
    let (products, licenses, api_keys) = tokio::try_join!(
        state.storage.list_products(identity.org_id, page),
        state.storage.list_licenses(identity.org_id, page),
        state.storage.list_api_keys(identity.org_id, page),
    )?;

    let validate_calls_last_14_days =
        usage::validate_history(state.cache.as_ref(), identity.org_id).await;

    Ok(Json(UsageSummary {
        products: products.len() as u32,
        licenses_total: licenses.len() as u32,
        licenses_active: licenses
            .iter()
            .filter(|l| l.status == LicenseStatus::Active)
            .count() as u32,
        api_keys_active: api_keys.iter().filter(|k| k.revoked_at.is_none()).count() as u32,
        counts_capped_at: COUNT_CAP,
        validate_calls_last_14_days,
    }))
}
