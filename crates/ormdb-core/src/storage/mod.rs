//! Storage layer for ORMDB.
//!
//! This module provides a sled-based storage engine with MVCC versioning support.

mod btree_index;
mod changelog;
mod columnar;
mod compaction;
mod config;
mod engine;
mod hash_index;
mod index_worker;
mod record;
mod transaction;

pub mod key;

pub use btree_index::BTreeIndex;
pub use changelog::{Changelog, ChangelogEntry, MutationType};
pub use columnar::{ColumnarProjection, ColumnarStore, StringDictionary};
pub use compaction::{CompactionEngine, CompactionResult};
pub use config::{RetentionPolicy, StorageConfig};
pub use engine::StorageEngine;
pub use hash_index::HashIndex;
pub use index_worker::{IndexWorker, IndexWorkerConfig};
pub use key::VersionedKey;
pub use record::Record;
pub use transaction::Transaction;
