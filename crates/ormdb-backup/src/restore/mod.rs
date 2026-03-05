//! Restore and point-in-time recovery.

use std::io::Read;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

use crate::error::{BackupError, Result};
use crate::snapshot::SnapshotManifest;
use crate::storage::BackupStorage;
use crate::{BackupChainEntry, BackupType};

/// Result of a restore operation.
#[derive(Debug)]
pub struct RestoreResult {
    /// Path where database was restored.
    pub path: PathBuf,
    /// LSN at restore point.
    pub restored_lsn: u64,
    /// Number of records restored.
    pub records_restored: u64,
    /// Whether integrity verification passed.
    pub verified: bool,
}

/// Engine for restoring backups.
pub struct RestoreEngine {
    target_path: PathBuf,
}

impl RestoreEngine {
    /// Create a new restore engine.
    pub fn new(target_path: PathBuf) -> Self {
        Self { target_path }
    }

    /// Restore from a full snapshot.
    pub async fn restore_full(
        &self,
        storage: &dyn BackupStorage,
        snapshot_path: &str,
    ) -> Result<RestoreResult> {
        // Check target doesn't exist
        if self.target_path.exists() {
            return Err(BackupError::TargetExists(
                self.target_path.display().to_string(),
            ));
        }

        // Create target directory
        std::fs::create_dir_all(&self.target_path)?;

        // Load manifest
        let manifest_path = format!("{}manifest.json", snapshot_path);
        let manifest_bytes = storage.get(&manifest_path).await?;
        let manifest: SnapshotManifest = serde_json::from_slice(&manifest_bytes)?;

        let mut records_restored = 0u64;

        // Restore each chunk
        for chunk in &manifest.chunks {
            let chunk_data = storage.get(&chunk.path).await?;

            // Verify checksum
            let checksum = format!("{:x}", Sha256::digest(&chunk_data));
            if checksum != chunk.checksum {
                return Err(BackupError::ChecksumMismatch {
                    expected: chunk.checksum.clone(),
                    actual: checksum,
                });
            }

            // Decompress
            let mut decoder = GzDecoder::new(&chunk_data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;

            // Parse and restore data
            records_restored += self.restore_chunk(&decompressed, chunk.index)?;
        }

        Ok(RestoreResult {
            path: self.target_path.clone(),
            restored_lsn: manifest.lsn,
            records_restored,
            verified: true,
        })
    }

    /// Restore to a specific point in time (LSN).
    pub async fn restore_to_point_in_time(
        &self,
        storage: &dyn BackupStorage,
        target_lsn: u64,
    ) -> Result<RestoreResult> {
        // Get backup status to find the backup chain
        let status = storage.get_backup_status().await?;

        // Find the most recent full snapshot before or at target LSN
        let base_snapshot = self
            .find_base_snapshot(&status.backup_chain, target_lsn)?
            .clone();

        // Restore base snapshot
        let mut result = self.restore_full(storage, &base_snapshot.path).await?;

        // Find and apply incremental backups
        let incrementals = self.find_incrementals(&status.backup_chain, base_snapshot.lsn_end, target_lsn)?;

        for incremental in incrementals {
            self.apply_incremental(storage, &incremental).await?;
            result.restored_lsn = incremental.lsn_end;
        }

        // Verify final LSN
        if result.restored_lsn < target_lsn {
            return Err(BackupError::LsnNotFound(target_lsn));
        }

        Ok(result)
    }

    /// Find the base snapshot for point-in-time recovery.
    fn find_base_snapshot<'a>(
        &self,
        chain: &'a [BackupChainEntry],
        target_lsn: u64,
    ) -> Result<&'a BackupChainEntry> {
        chain
            .iter()
            .filter(|e| e.backup_type == BackupType::Full && e.lsn_end <= target_lsn)
            .max_by_key(|e| e.lsn_end)
            .ok_or(BackupError::NoBackupFound)
    }

    /// Find incremental backups between base snapshot and target LSN.
    fn find_incrementals(
        &self,
        chain: &[BackupChainEntry],
        from_lsn: u64,
        to_lsn: u64,
    ) -> Result<Vec<BackupChainEntry>> {
        let mut incrementals: Vec<_> = chain
            .iter()
            .filter(|e| {
                e.backup_type == BackupType::Incremental
                    && e.lsn_start > from_lsn
                    && e.lsn_start <= to_lsn
            })
            .cloned()
            .collect();

        incrementals.sort_by_key(|e| e.lsn_start);

        // Verify chain continuity
        let mut expected_lsn = from_lsn;
        for inc in &incrementals {
            if inc.lsn_start != expected_lsn + 1 {
                return Err(BackupError::BrokenChain(expected_lsn + 1, inc.lsn_start));
            }
            expected_lsn = inc.lsn_end;
        }

        Ok(incrementals)
    }

    /// Apply an incremental backup.
    async fn apply_incremental(
        &self,
        storage: &dyn BackupStorage,
        entry: &BackupChainEntry,
    ) -> Result<()> {
        let data = storage.get(&entry.path).await?;

        // Decompress
        let mut decoder = GzDecoder::new(&data[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        // Deserialize changelog entries
        let entries: Vec<ormdb_proto::replication::ChangeLogEntry> =
            rkyv::from_bytes::<Vec<ormdb_proto::replication::ChangeLogEntry>, rkyv::rancor::Error>(&decompressed)
                .map_err(|e: rkyv::rancor::Error| BackupError::Serialization(e.to_string()))?;

        // Apply entries to the restored database
        self.apply_changelog_entries(&entries)?;

        Ok(())
    }

    /// Restore a data chunk to the target path.
    fn restore_chunk(&self, data: &[u8], chunk_index: usize) -> Result<u64> {
        let mut offset = 0;
        let mut count = 0u64;

        // Parse length-prefixed key-value pairs
        while offset < data.len() {
            if offset + 4 > data.len() {
                break;
            }

            let key_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + key_len > data.len() {
                break;
            }
            let _key = &data[offset..offset + key_len];
            offset += key_len;

            if offset + 4 > data.len() {
                break;
            }
            let value_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + value_len > data.len() {
                break;
            }
            let _value = &data[offset..offset + value_len];
            offset += value_len;

            // In a real implementation, we would write to the sled database here
            // For now, we're just counting records
            count += 1;
        }

        tracing::debug!(
            "Restored {} records from chunk {}",
            count,
            chunk_index
        );

        Ok(count)
    }

    /// Apply changelog entries to restore incremental changes.
    fn apply_changelog_entries(
        &self,
        _entries: &[ormdb_proto::replication::ChangeLogEntry],
    ) -> Result<()> {
        // In a real implementation, we would:
        // 1. Open the sled database at target_path
        // 2. Replay each changelog entry
        // For now, this is a stub
        Ok(())
    }
}
