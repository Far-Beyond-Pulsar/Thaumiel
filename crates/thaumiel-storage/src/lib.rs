//! [`thaumiel_core::traits::Storage`] implementations, one module per
//! database backend, each behind its own cargo feature (all on by default):
//! [`postgres::PostgresStorage`], [`mysql::MySqlStorage`],
//! [`sqlite::SqliteStorage`], and [`memory::InMemoryStorage`] (no external
//! database, used by tests and quick demos).
//!
//! Every `sqlx` backend shares its row -> domain-type mapping via
//! [`mapping`] -- only SQL placeholder syntax and pool setup differ between
//! them (see that module's doc comment for why this is safe).

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
pub mod mapping;

#[cfg(feature = "memory")]
pub mod memory;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "memory")]
pub use memory::InMemoryStorage;
#[cfg(feature = "mysql")]
pub use mysql::MySqlStorage;
#[cfg(feature = "postgres")]
pub use postgres::PostgresStorage;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStorage;
