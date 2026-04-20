//! ORMDB Backup - S3-compatible backup and restore.
//!
//! This crate provides backup and restore functionality for ORMDB databases:
//!
//! - **Full snapshots**: Complete database state at a point in time
//! - **Incremental backups**: Efficient backups using the changelog/CDC system
//! - **Point-in-time recovery**: Restore to any LSN in the backup chain
//! - **S3-compatible storage**: AWS S3, MinIO, DigitalOcean Spaces, Backblaze B2
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ormdb_backup::{BackupConfig, BackupManager};
//!
//! // Configure S3 backup
//! let config = BackupConfig::builder()
//!     .bucket("my-backups")
//!     .prefix("ormdb/")
//!     .build()?;
//!
//! // Create backup manager
//! let manager = BackupManager::new(storage_engine, changelog, config).await?;
//!
//! // Create full snapshot
//! let manifest = manager.create_snapshot().await?;
//!
//! // Create incremental backup
//! let incremental = manager.create_incremental().await?;
//! ```

pub mod config;
pub mod error;
pub mod incremental;
pub mod restore;
pub mod snapshot;
pub mod storage;
mod verify;

pub use config::{BackupConfig, BackupConfigBuilder};
pub use error::{BackupError, Result};
pub use incremental::IncrementalBackup;
pub use restore::{RestoreEngine, RestoreResult};
pub use snapshot::{SnapshotCreator, SnapshotManifest};
pub use storage::{BackupStorage, S3Storage};
pub use verify::IntegrityChecker;

use std::sync::Arc;

use ormdb_core::replication::ChangeLog;
use ormdb_core::StorageEngine;

/// High-level backup manager for creating and managing backups.
pub struct BackupManager {
    storage_engine: Arc<StorageEngine>,
    changelog: Arc<ChangeLog>,
    backup_storage: Box<dyn BackupStorage>,
    config: BackupConfig,
}

impl BackupManager {
    /// Create a new backup manager.
    pub async fn new(
        storage_engine: Arc<StorageEngine>,
        changelog: Arc<ChangeLog>,
        config: BackupConfig,
    ) -> Result<Self> {
        let backup_storage = storage::create_storage(&config).await?;
        Ok(Self {
            storage_engine,
            changelog,
            backup_storage,
            config,
        })
    }

    /// Create a full snapshot.
    pub async fn create_snapshot(&self) -> Result<SnapshotManifest> {
        let creator = SnapshotCreator::new(
            self.storage_engine.clone(),
            self.changelog.clone(),
            self.config.clone(),
        );
        creator
            .create_snapshot(self.backup_storage.as_ref())
            .await
    }

    /// Create an incremental backup since the last backup.
    pub async fn create_incremental(&self) -> Result<incremental::IncrementalManifest> {
        // Get last backed up LSN from storage
        let last_lsn = self
            .backup_storage
            .get_last_backed_up_lsn()
            .await?
            .unwrap_or(0);

        let backup = IncrementalBackup::new(self.changelog.clone());
        backup
            .create_backup(self.backup_storage.as_ref(), last_lsn)
            .await
    }

    /// Get backup status and history.
    pub async fn get_status(&self) -> Result<BackupStatus> {
        self.backup_storage.get_backup_status().await
    }

    /// Restore from backup to a new path.
    pub async fn restore(
        config: BackupConfig,
        target_path: std::path::PathBuf,
        target_lsn: Option<u64>,
    ) -> Result<RestoreResult> {
        let backup_storage = storage::create_storage(&config).await?;
        let engine = RestoreEngine::new(target_path);

        if let Some(lsn) = target_lsn {
            engine
                .restore_to_point_in_time(backup_storage.as_ref(), lsn)
                .await
        } else {
            // Find latest snapshot
            let status = backup_storage.get_backup_status().await?;
            let latest = status
                .latest_snapshot
                .ok_or(BackupError::NoBackupFound)?;
            engine
                .restore_full(backup_storage.as_ref(), &latest.path)
                .await
        }
    }
}

/// Backup status and history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupStatus {
    /// Latest full snapshot info.
    pub latest_snapshot: Option<SnapshotInfo>,
    /// Latest incremental backup info.
    pub latest_incremental: Option<IncrementalInfo>,
    /// Full backup chain.
    pub backup_chain: Vec<BackupChainEntry>,
}

/// Information about a full snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotInfo {
    /// Timestamp when snapshot was created.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// LSN at snapshot time.
    pub lsn: u64,
    /// Path in storage.
    pub path: String,
    /// Total size in bytes.
    pub total_bytes: u64,
}

/// Information about an incremental backup.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IncrementalInfo {
    /// Timestamp when backup was created.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Start LSN.
    pub lsn_start: u64,
    /// End LSN.
    pub lsn_end: u64,
    /// Path in storage.
    pub path: String,
}

/// Entry in the backup chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupChainEntry {
    /// Type of backup.
    pub backup_type: BackupType,
    /// LSN range.
    pub lsn_start: u64,
    pub lsn_end: u64,
    /// Path in storage.
    pub path: String,
    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Type of backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    /// Full snapshot.
    Full,
    /// Incremental backup.
    Incremental,
}
