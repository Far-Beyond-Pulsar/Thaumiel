//! [`thaumiel_core::traits::Cache`] implementations: [`memory::InMemoryCache`]
//! (default, no external service) and [`redis::RedisCache`] (feature
//! `redis-backend`, on by default).

#[cfg(feature = "memory-backend")]
pub mod memory;
#[cfg(feature = "redis-backend")]
pub mod redis;

#[cfg(feature = "redis-backend")]
pub use crate::redis::RedisCache;
#[cfg(feature = "memory-backend")]
pub use memory::InMemoryCache;
