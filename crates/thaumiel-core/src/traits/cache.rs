use async_trait::async_trait;
use std::time::Duration;

use crate::error::Result;

/// Cache abstraction implemented by `thaumiel-cache`'s `redis` and `memory`
/// modules. Used for rate limiting, hot license lookups, and JWT denylists.
///
/// Kept deliberately small (string keys/values) rather than wrapping every
/// possible Redis command — plugins that need more can depend on `redis`
/// directly, this trait only covers what the core server needs.
#[async_trait]
pub trait Cache: Send + Sync {
    fn id(&self) -> &'static str;

    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()>;
    async fn del(&self, key: &str) -> Result<()>;

    /// Atomically increment `key` by 1 (starting from 0), returning the new value.
    /// Used for fixed-window rate limiting. Implementations set `ttl` only the
    /// first time the key is created within a window.
    async fn incr(&self, key: &str, ttl: Option<Duration>) -> Result<i64>;
}
