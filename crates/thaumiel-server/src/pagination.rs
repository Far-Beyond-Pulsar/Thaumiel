//! `?limit=&offset=` for list routes, converting into
//! [`thaumiel_core::traits::Pagination`]. See issue #10: every `Storage`
//! list method already took a `Pagination`, but route handlers hardcoded
//! `Pagination::default()` -- this is what lets a caller actually page past
//! the first N results.

use serde::Deserialize;
use thaumiel_core::traits::Pagination;

/// Anything above this is clamped down rather than rejected outright, so a
/// caller asking for too much gets a bounded response instead of an error
/// (or an accidental unbounded query against storage).
const MAX_LIMIT: u32 = 200;
const DEFAULT_LIMIT: u32 = 50;

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

impl From<PageQuery> for Pagination {
    fn from(q: PageQuery) -> Self {
        let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        Pagination { limit, offset: q.offset.unwrap_or(0) }
    }
}
