//! Snapshot creation logic.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ChunkInfo;
use crate::config::BackupConfig;
use crate::error::Result;
use crate::storage::BackupStorage;
use crate::SnapshotInfo;
use ormdb_core::replication::ChangeLog;
use ormdb_core::StorageEngine;

/// Tree names for direct sled access.
const DATA_TREE: &str = "data";
const META_TREE: &str = "meta";
const TYPE_INDEX_TREE: &str = "index:entity_type";

/// Snapshot manifest containing metadata about a full backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    /// Manifest format version.
    pub version: u32,
    /// Timestamp when snapshot was created.
    pub created_at: chrono::DateTime<Utc>,
    /// LSN at snapshot time.
    pub lsn: u64,
    /// Overall checksum of all chunks.
    pub checksum: String,
    /// Individual chunk information.
    pub chunks: Vec<ChunkInfo>,
    /// Total size in bytes (uncompressed).
    pub total_bytes_uncompressed: u64,
    /// Total size in bytes (compressed).
    pub total_bytes_compressed: u64,
    /// Entity counts by type name.
    pub entity_counts: HashMap<String, u64>,
    /// Compression algorithm used.
    pub compression: String,
}

/// Creates full database snapshots.
pub struct SnapshotCreator {
    storage_engine: Arc<StorageEngine>,
    changelog: Arc<ChangeLog>,
    config: BackupConfig,
}

impl SnapshotCreator {
    /// Create a new snapshot creator.
    pub fn new(
        storage_engine: Arc<StorageEngine>,
        changelog: Arc<ChangeLog>,
        config: BackupConfig,
    ) -> Self {
        Self {
            storage_engine,
            changelog,
            config,
        }
    }

    /// Create a full snapshot and upload to storage.
    pub async fn create_snapshot(&self, storage: &dyn BackupStorage) -> Result<SnapshotManifest> {
        let snapshot_lsn = self.changelog.current_lsn();
        let timestamp = Utc::now();
        let snapshot_path = format!(
            "snapshots/{}/",
            timestamp.format("%Y-%m-%dT%H-%M-%SZ")
        );

        let mut chunks = Vec::new();
        let mut total_uncompressed = 0u64;
        let mut total_compressed = 0u64;
        let mut overall_hasher = Sha256::new();
        let entity_counts = HashMap::new();

        // Serialize the data tree
        let data = self.serialize_tree(DATA_TREE)?;
        total_uncompressed += data.len() as u64;

        // Compress data
        let compressed = self.compress_data(&data)?;
        total_compressed += compressed.len() as u64;

        // Calculate chunk checksum
        let chunk_checksum = format!("{:x}", Sha256::digest(&compressed));
        overall_hasher.update(&compressed);

        // Upload chunk
        let chunk_path = format!("{}data.000.ormdb.gz", snapshot_path);
        storage.put(&chunk_path, Bytes::from(compressed.clone())).await?;

        chunks.push(ChunkInfo {
            index: 0,
            path: chunk_path,
            size_bytes: compressed.len() as u64,
            checksum: chunk_checksum,
        });

        // Serialize meta tree
        let meta_data = self.serialize_tree(META_TREE)?;
        total_uncompressed += meta_data.len() as u64;

        let meta_compressed = self.compress_data(&meta_data)?;
        total_compressed += meta_compressed.len() as u64;

        let meta_checksum = format!("{:x}", Sha256::digest(&meta_compressed));
        overall_hasher.update(&meta_compressed);

        let meta_path = format!("{}meta.000.ormdb.gz", snapshot_path);
        storage.put(&meta_path, Bytes::from(meta_compressed.clone())).await?;

        chunks.push(ChunkInfo {
            index: 1,
            path: meta_path,
            size_bytes: meta_compressed.len() as u64,
            checksum: meta_checksum,
        });

        // Serialize type index tree
        let type_data = self.serialize_tree(TYPE_INDEX_TREE)?;
        total_uncompressed += type_data.len() as u64;

        let type_compressed = self.compress_data(&type_data)?;
        total_compressed += type_compressed.len() as u64;

        let type_checksum = format!("{:x}", Sha256::digest(&type_compressed));
        overall_hasher.update(&type_compressed);

        let type_path = format!("{}type_index.ormdb.gz", snapshot_path);
        storage.put(&type_path, Bytes::from(type_compressed.clone())).await?;

        chunks.push(ChunkInfo {
            index: 2,
            path: type_path,
            size_bytes: type_compressed.len() as u64,
            checksum: type_checksum,
        });

        let overall_checksum = format!("{:x}", overall_hasher.finalize());

        let manifest = SnapshotManifest {
            version: 1,
            created_at: timestamp,
            lsn: snapshot_lsn,
            checksum: overall_checksum,
            chunks,
            total_bytes_uncompressed: total_uncompressed,
            total_bytes_compressed: total_compressed,
            entity_counts,
            compression: "gzip".to_string(),
        };

        // Upload manifest
        let manifest_json = serde_json::to_vec_pretty(&manifest)?;
        let manifest_path = format!("{}manifest.json", snapshot_path);
        storage.put(&manifest_path, Bytes::from(manifest_json)).await?;

        // Record snapshot in storage state
        storage
            .record_snapshot(SnapshotInfo {
                timestamp,
                lsn: snapshot_lsn,
                path: snapshot_path,
                total_bytes: total_compressed,
            })
            .await?;

        Ok(manifest)
    }

    /// Serialize a sled tree to bytes.
    fn serialize_tree(&self, tree_name: &str) -> Result<Vec<u8>> {
        let db = self.storage_engine.db();
        let tree = db.open_tree(tree_name)?;

        let mut data = Vec::new();

        for result in tree.iter() {
            let (key, value) = result?;
            // Write key length + key + value length + value
            data.extend_from_slice(&(key.len() as u32).to_le_bytes());
            data.extend_from_slice(&key);
            data.extend_from_slice(&(value.len() as u32).to_le_bytes());
            data.extend_from_slice(&value);
        }

        Ok(data)
    }

    /// Compress data using gzip.
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let level = match self.config.compression_level {
            1..=3 => Compression::fast(),
            4..=6 => Compression::default(),
            _ => Compression::best(),
        };
        let mut encoder = GzEncoder::new(Vec::new(), level);
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }
}
