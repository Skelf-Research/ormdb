//! Incremental backup creation from changelog.

use std::io::Write;
use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{BackupError, Result};
use crate::storage::BackupStorage;
use crate::IncrementalInfo;
use ormdb_core::replication::ChangeLog;

/// Manifest for an incremental backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalManifest {
    /// Manifest format version.
    pub version: u32,
    /// Timestamp when backup was created.
    pub created_at: chrono::DateTime<Utc>,
    /// Start LSN (exclusive - changes after this LSN).
    pub lsn_start: u64,
    /// End LSN (inclusive - changes up to and including this LSN).
    pub lsn_end: u64,
    /// Number of changelog entries.
    pub entry_count: usize,
    /// Path to backup file in storage.
    pub path: String,
    /// Size in bytes (compressed).
    pub size_bytes: u64,
    /// SHA-256 checksum.
    pub checksum: String,
    /// Compression algorithm used.
    pub compression: String,
}

/// Creates incremental backups from changelog entries.
pub struct IncrementalBackup {
    changelog: Arc<ChangeLog>,
}

impl IncrementalBackup {
    /// Create a new incremental backup creator.
    pub fn new(changelog: Arc<ChangeLog>) -> Self {
        Self { changelog }
    }

    /// Create an incremental backup from the given LSN.
    ///
    /// This backs up all changelog entries from `from_lsn + 1` to the current LSN.
    pub async fn create_backup(
        &self,
        storage: &dyn BackupStorage,
        from_lsn: u64,
    ) -> Result<IncrementalManifest> {
        let current_lsn = self.changelog.current_lsn();

        if current_lsn <= from_lsn {
            return Err(BackupError::InvalidConfig(format!(
                "no new changes to backup: current LSN {} <= from LSN {}",
                current_lsn, from_lsn
            )));
        }

        let timestamp = Utc::now();

        // Collect changelog entries
        let entries: Vec<_> = self
            .changelog
            .scan_from(from_lsn + 1)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if entries.is_empty() {
            return Err(BackupError::InvalidConfig(
                "no changelog entries found".to_string(),
            ));
        }

        let entry_count = entries.len();

        // Serialize entries using rkyv
        let serialized = rkyv::to_bytes::<rkyv::rancor::Error>(&entries)
            .map_err(|e| BackupError::Serialization(e.to_string()))?;

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&serialized)?;
        let compressed = encoder.finish()?;

        // Calculate checksum
        let checksum = format!("{:x}", Sha256::digest(&compressed));

        // Generate path
        let path = format!(
            "incremental/lsn-{:012}-{:012}.ormdb.gz",
            from_lsn + 1,
            current_lsn
        );

        // Upload to storage
        storage.put(&path, Bytes::from(compressed.clone())).await?;

        let manifest = IncrementalManifest {
            version: 1,
            created_at: timestamp,
            lsn_start: from_lsn + 1,
            lsn_end: current_lsn,
            entry_count,
            path: path.clone(),
            size_bytes: compressed.len() as u64,
            checksum,
            compression: "gzip".to_string(),
        };

        // Upload manifest
        let manifest_path = format!(
            "incremental/lsn-{:012}-{:012}.manifest.json",
            from_lsn + 1,
            current_lsn
        );
        let manifest_json = serde_json::to_vec_pretty(&manifest)?;
        storage.put(&manifest_path, Bytes::from(manifest_json)).await?;

        // Record incremental in storage state
        storage
            .record_incremental(IncrementalInfo {
                timestamp,
                lsn_start: from_lsn + 1,
                lsn_end: current_lsn,
                path,
            })
            .await?;

        Ok(manifest)
    }
}
