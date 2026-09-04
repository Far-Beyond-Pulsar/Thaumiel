//! Strongly typed, UUIDv7 (time-sortable) identifiers for every domain entity.
//!
//! A plain `Uuid` field lets you accidentally pass an `OrganizationId` where a
//! `ProductId` is expected; these newtypes make that a compile error while still
//! being `Copy`, `serde`-transparent, and free to construct.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a new, time-sortable (UUIDv7) id.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }
    };
}

typed_id!(OrganizationId);
typed_id!(ProductId);
typed_id!(LicenseId);
typed_id!(ActivationId);
typed_id!(ApiKeyId);
typed_id!(UserId);
typed_id!(AuditLogId);
