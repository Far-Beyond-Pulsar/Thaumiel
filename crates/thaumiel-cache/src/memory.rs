//! Process-local in-memory [`Cache`] implementation. No external dependencies,
//! no persistence across restarts, and does not coordinate across multiple
//! server instances — intended for local development or single-instance
//! deployments where Redis isn't available.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use thaumiel_core::traits::Cache;
use thaumiel_core::Result;

struct Entry {
    value: String,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_expired(&self, now: Instant) -> bool {
        matches!(self.expires_at, Some(exp) if now >= exp)
    }
}

pub struct InMemoryCache {
    store: Mutex<HashMap<String, Entry>>,
}

impl InMemoryCache {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    fn id(&self) -> &'static str {
        "memory"
    }

    async fn get(&self, key: &str) -> Result<Option<String>> {
        let now = Instant::now();
        let mut store = self.store.lock().expect("cache mutex poisoned");
        match store.get(key) {
            Some(entry) if entry.is_expired(now) => {
                store.remove(key);
                Ok(None)
            }
            Some(entry) => Ok(Some(entry.value.clone())),
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let mut store = self.store.lock().expect("cache mutex poisoned");
        store.insert(
            key.to_string(),
            Entry {
                value: value.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<()> {
        let mut store = self.store.lock().expect("cache mutex poisoned");
        store.remove(key);
        Ok(())
    }

    async fn incr(&self, key: &str, ttl: Option<Duration>) -> Result<i64> {
        let now = Instant::now();
        let mut store = self.store.lock().expect("cache mutex poisoned");
        let expired = matches!(store.get(key), Some(e) if e.is_expired(now));
        if expired {
            store.remove(key);
        }
        match store.get_mut(key) {
            Some(entry) => {
                let next: i64 = entry.value.parse().unwrap_or(0) + 1;
                entry.value = next.to_string();
                Ok(next)
            }
            None => {
                let expires_at = ttl.map(|d| now + d);
                store.insert(
                    key.to_string(),
                    Entry {
                        value: "1".to_string(),
                        expires_at,
                    },
                );
                Ok(1)
            }
        }
    }
}
