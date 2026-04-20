//! Incremental backup using changelog/CDC.

mod backup;

pub use backup::{IncrementalBackup, IncrementalManifest};
