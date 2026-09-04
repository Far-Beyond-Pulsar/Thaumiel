//! Redis-backed [`Cache`] implementation, using a
//! [`redis::aio::ConnectionManager`] (auto-reconnecting, cheaply `Clone`) so a
//! single [`RedisCache`] can be shared behind an `Arc` across the whole
//! server without a connection pool of its own.

use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;

use thaumiel_core::traits::Cache;
use thaumiel_core::{Result, ThaumielError};

pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).map_err(|e| ThaumielError::Cache(e.to_string()))?;
        let conn = client
            .get_connection_manager()
            .await
            .map_err(|e| ThaumielError::Cache(e.to_string()))?;
        Ok(Self { conn })
    }
}

#[async_trait]
impl Cache for RedisCache {
    fn id(&self) -> &'static str {
        "redis"
    }

    async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.conn.clone();
        conn.get(key)
            .await
            .map_err(|e| ThaumielError::Cache(e.to_string()))
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.conn.clone();
        match ttl {
            Some(ttl) => conn
                .set_ex::<_, _, ()>(key, value, ttl.as_secs().max(1))
                .await
                .map_err(|e| ThaumielError::Cache(e.to_string())),
            None => conn
                .set::<_, _, ()>(key, value)
                .await
                .map_err(|e| ThaumielError::Cache(e.to_string())),
        }
    }

    async fn del(&self, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        conn.del::<_, ()>(key)
            .await
            .map_err(|e| ThaumielError::Cache(e.to_string()))
    }

    async fn incr(&self, key: &str, ttl: Option<Duration>) -> Result<i64> {
        let mut conn = self.conn.clone();

        // Execute INCR and EXPIRE atomically using a Lua script to prevent race conditions.
        // If we did INCR then EXPIRE in two roundtrips, a crash in between would leave the key without a TTL.
        if let Some(ttl) = ttl {
            let script = redis::Script::new(
                r#"
                local current = redis.call('INCR', KEYS[1])
                if current == 1 then
                    redis.call('EXPIRE', KEYS[1], ARGV[1])
                end
                return current
            "#,
            );

            let count: i64 = script
                .key(key)
                .arg(ttl.as_secs().max(1) as i64)
                .invoke_async(&mut conn)
                .await
                .map_err(|e| ThaumielError::Cache(e.to_string()))?;

            Ok(count)
        } else {
            let count: i64 = conn
                .incr(key, 1)
                .await
                .map_err(|e| ThaumielError::Cache(e.to_string()))?;
            Ok(count)
        }
    }
}
