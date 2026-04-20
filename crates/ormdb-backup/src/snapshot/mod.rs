//! Full snapshot creation and management.

mod creator;

pub use creator::{SnapshotCreator, SnapshotManifest};

use serde::{Deserialize, Serialize};

/// Information about a snapshot chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    /// Chunk index.
    pub index: usize,
    /// Path in storage.
    pub path: String,
    /// Size in bytes (compressed).
    pub size_bytes: u64,
    /// SHA-256 checksum.
    pub checksum: String,
}
