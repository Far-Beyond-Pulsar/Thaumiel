use std::time::Duration;

use thaumiel_core::traits::Cache;
use thaumiel_core::{Result, ThaumielError};

/// Fixed-window rate limit backed by `Cache::incr`. `key` should already be
/// scoped to whatever identity/IP is being limited (e.g. `"validate:{prefix}"`).
pub async fn check(cache: &dyn Cache, key: &str, limit: i64, window: Duration) -> Result<()> {
    let count = cache.incr(key, Some(window)).await?;
    if count > limit {
        return Err(ThaumielError::RateLimited { retry_after_secs: window.as_secs() });
    }
    Ok(())
}
