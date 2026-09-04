//! Usage metering (the tractable half of issue #6 -- see that issue and
//! `docs/ARCHITECTURE.md` for why actual billing/payment integration is
//! explicitly *not* part of this: no pricing model, no payment processor
//! account, and no plan-limit product decisions exist to build one against.
//! What's here is the data those decisions would need: how much an
//! organization is actually using this server.
//!
//! Backed by `Cache` (the same trait rate limiting already uses), keyed per
//! org per UTC day, rather than a new `Storage`-level table -- keeps this
//! feature schema-free across all five storage backends, at the cost of
//! being best-effort (a cache flush loses history) and not indefinitely
//! retained (counters carry a TTL). That trade is the right one for a
//! rolling usage dashboard; it would not be the right one for anything an
//! invoice gets generated from, which is exactly the line where "metering"
//! stops and "billing" -- deliberately not built -- would begin.

use std::time::Duration;

use chrono::Utc;
use serde::Serialize;

use thaumiel_core::ids::OrganizationId;
use thaumiel_core::traits::Cache;

const COUNTER_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 40); // ~40 days
const HISTORY_DAYS: i64 = 14;

fn day_key(org_id: OrganizationId, date: &str) -> String {
    format!("usage:validate:{org_id}:{date}")
}

/// Called once per `/v1/licenses/validate` request that gets far enough to
/// be rate-limit-checked -- i.e. it counts *attempts*, valid or not, the
/// same way an API metering system would count requests rather than
/// successes. Best-effort: a cache error here is logged and swallowed,
/// same policy as `crate::audit::record`, since a metering hiccup shouldn't
/// fail the caller's actual request.
pub async fn record_validation(cache: &dyn Cache, org_id: OrganizationId) {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    if let Err(e) = cache
        .incr(&day_key(org_id, &today), Some(COUNTER_TTL))
        .await
    {
        tracing::warn!(error = %e, %org_id, "failed to record validate-call usage counter");
    }
}

#[derive(Debug, Serialize)]
pub struct UsageDayCount {
    pub date: String,
    pub count: i64,
}

/// Last 14 days of validate-call volume, oldest first, zero-filled for days
/// with no calls (rather than omitted, so a client can render a fixed-width
/// chart without special-casing gaps).
pub async fn validate_history(cache: &dyn Cache, org_id: OrganizationId) -> Vec<UsageDayCount> {
    let mut days = Vec::with_capacity(HISTORY_DAYS as usize);
    for offset in (0..HISTORY_DAYS).rev() {
        let date = (Utc::now() - chrono::Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string();
        let count = cache
            .get(&day_key(org_id, &date))
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        days.push(UsageDayCount { date, count });
    }
    days
}
