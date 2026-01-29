//! ORMDB Core - Storage engine, catalog, and query execution.
//!
//! This crate provides the core database functionality for ORMDB.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod catalog;
pub mod error;
pub mod query;
pub mod storage;

pub use catalog::{
    Cardinality, Catalog, ConstraintDef, DeleteBehavior, EntityDef, FieldDef, FieldType,
    LifecycleRules, OrderBy, OrderDirection, RelationDef, ScalarType, SchemaBundle,
};
pub use error::Error;
pub use storage::{Record, StorageConfig, StorageEngine, Transaction, VersionedKey};

/// Re-export protocol types.
pub use ormdb_proto as proto;
