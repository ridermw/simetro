//! Wire schema versioning. Phase 1 ships at v1.
//!
//! Migration map is empty in P1. When a v2 lands, add a migrator
//! `(u32, u32) -> Result<Value, VersionError>` and route it from
//! the consumer side before deserializing the typed payload.

pub const SCHEMA_VERSION: u32 = 1;
