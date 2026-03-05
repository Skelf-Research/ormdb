//! Storage layer for ORMDB.
//!
//! This module provides a sled-based storage engine with MVCC versioning support.

mod btree_index;
mod changelog;
mod columnar;
mod compaction;
mod config;
mod engine;
mod fulltext_index;
mod geo_index;
mod hash_index;
mod index_worker;
mod record;
mod stemmer;
mod transaction;
mod vector_index;

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
pub use vector_index::{DistanceMetric, HnswConfig, VectorIndex};
pub use geo_index::{GeoIndex, GeoPoint, MBR, RTreeConfig, haversine_distance, point_in_polygon};
pub use stemmer::{PorterStemmer, tokenize, tokenize_and_stem, analyze, is_stop_word, STOP_WORDS};
pub use fulltext_index::{FullTextIndex, FullTextConfig, SearchResult};
