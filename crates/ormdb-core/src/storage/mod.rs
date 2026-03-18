//! Storage layer for ORMDB.
//!
//! This module provides a sled-based storage engine with MVCC versioning support.

mod columnar;
mod compaction;
mod config;
mod engine;
mod record;
mod transaction;

pub mod key;

pub use columnar::{ColumnarProjection, ColumnarStore, StringDictionary};
pub use compaction::{CompactionEngine, CompactionResult};
pub use config::{RetentionPolicy, StorageConfig};
pub use engine::StorageEngine;
pub use key::VersionedKey;
pub use record::Record;
pub use transaction::Transaction;
