//! Backup integrity verification.

use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::snapshot::SnapshotManifest;
use crate::storage::BackupStorage;

/// Integrity checker for verifying backups.
pub struct IntegrityChecker;

impl IntegrityChecker {
    /// Verify a snapshot's integrity.
    pub async fn verify_snapshot(
        storage: &dyn BackupStorage,
        snapshot_path: &str,
    ) -> Result<VerificationResult> {
        // Load manifest
        let manifest_path = format!("{}manifest.json", snapshot_path);
        let manifest_bytes = storage.get(&manifest_path).await?;
        let manifest: SnapshotManifest = serde_json::from_slice(&manifest_bytes)?;

        let mut overall_hasher = Sha256::new();
        let mut chunks_verified = 0;
        let mut errors = Vec::new();

        // Verify each chunk
        for chunk in &manifest.chunks {
            let chunk_relative = chunk
                .path
                .strip_prefix(&format!("{}snapshots/", ""))
                .unwrap_or(&chunk.path);

            match storage.get(chunk_relative).await {
                Ok(data) => {
                    let checksum = format!("{:x}", Sha256::digest(&data));
                    if checksum != chunk.checksum {
                        errors.push(format!(
                            "Chunk {}: checksum mismatch (expected {}, got {})",
                            chunk.index, chunk.checksum, checksum
                        ));
                    } else {
                        overall_hasher.update(&data);
                        chunks_verified += 1;
                    }
                }
                Err(e) => {
                    errors.push(format!("Chunk {}: failed to read: {}", chunk.index, e));
                }
            }
        }

        // Verify overall checksum if all chunks passed
        let overall_valid = if errors.is_empty() {
            let overall = format!("{:x}", overall_hasher.finalize());
            if overall != manifest.checksum {
                errors.push(format!(
                    "Overall checksum mismatch (expected {}, got {})",
                    manifest.checksum, overall
                ));
                false
            } else {
                true
            }
        } else {
            false
        };

        Ok(VerificationResult {
            snapshot_path: snapshot_path.to_string(),
            manifest_lsn: manifest.lsn,
            chunks_total: manifest.chunks.len(),
            chunks_verified,
            overall_checksum_valid: overall_valid,
            errors,
        })
    }

    /// Verify an incremental backup's integrity.
    pub async fn verify_incremental(
        storage: &dyn BackupStorage,
        path: &str,
    ) -> Result<VerificationResult> {
        // Load manifest
        let manifest_path = path.replace(".ormdb.gz", ".manifest.json");
        let manifest_bytes = storage.get(&manifest_path).await?;
        let manifest: crate::incremental::IncrementalManifest =
            serde_json::from_slice(&manifest_bytes)?;

        let mut errors = Vec::new();

        // Verify data file
        match storage.get(path).await {
            Ok(data) => {
                let checksum = format!("{:x}", Sha256::digest(&data));
                if checksum != manifest.checksum {
                    errors.push(format!(
                        "Checksum mismatch (expected {}, got {})",
                        manifest.checksum, checksum
                    ));
                }
            }
            Err(e) => {
                errors.push(format!("Failed to read backup file: {}", e));
            }
        }

        Ok(VerificationResult {
            snapshot_path: path.to_string(),
            manifest_lsn: manifest.lsn_end,
            chunks_total: 1,
            chunks_verified: if errors.is_empty() { 1 } else { 0 },
            overall_checksum_valid: errors.is_empty(),
            errors,
        })
    }

    /// Verify the entire backup chain.
    pub async fn verify_chain(storage: &dyn BackupStorage) -> Result<ChainVerificationResult> {
        let status = storage.get_backup_status().await?;
        let mut results = Vec::new();
        let mut chain_valid = true;
        let mut expected_lsn = 0u64;

        for entry in &status.backup_chain {
            // Verify LSN continuity
            if entry.lsn_start > expected_lsn + 1 {
                chain_valid = false;
                results.push(ChainEntryResult {
                    path: entry.path.clone(),
                    lsn_range: (entry.lsn_start, entry.lsn_end),
                    valid: false,
                    error: Some(format!(
                        "LSN gap: expected start <= {}, got {}",
                        expected_lsn + 1,
                        entry.lsn_start
                    )),
                });
            } else {
                results.push(ChainEntryResult {
                    path: entry.path.clone(),
                    lsn_range: (entry.lsn_start, entry.lsn_end),
                    valid: true,
                    error: None,
                });
            }

            expected_lsn = entry.lsn_end;
        }

        Ok(ChainVerificationResult {
            entries: results,
            chain_valid,
            latest_lsn: expected_lsn,
        })
    }
}

/// Result of verifying a single backup.
#[derive(Debug)]
pub struct VerificationResult {
    /// Path of the verified backup.
    pub snapshot_path: String,
    /// LSN in the manifest.
    pub manifest_lsn: u64,
    /// Total number of chunks.
    pub chunks_total: usize,
    /// Number of chunks successfully verified.
    pub chunks_verified: usize,
    /// Whether the overall checksum is valid.
    pub overall_checksum_valid: bool,
    /// List of errors encountered.
    pub errors: Vec<String>,
}

impl VerificationResult {
    /// Check if verification passed.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.overall_checksum_valid
    }
}

/// Result of a single chain entry verification.
#[derive(Debug)]
pub struct ChainEntryResult {
    /// Path of the backup.
    pub path: String,
    /// LSN range (start, end).
    pub lsn_range: (u64, u64),
    /// Whether this entry is valid.
    pub valid: bool,
    /// Error message if invalid.
    pub error: Option<String>,
}

/// Result of verifying the entire backup chain.
#[derive(Debug)]
pub struct ChainVerificationResult {
    /// Results for each chain entry.
    pub entries: Vec<ChainEntryResult>,
    /// Whether the chain is valid (no gaps).
    pub chain_valid: bool,
    /// Latest LSN in the chain.
    pub latest_lsn: u64,
}
