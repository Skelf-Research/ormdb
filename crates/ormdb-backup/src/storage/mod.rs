//! Storage backends for backup operations.

mod s3;

pub use s3::S3Storage;

use async_trait::async_trait;
use bytes::Bytes;

use crate::config::BackupConfig;
use crate::error::Result;
use crate::{BackupStatus, IncrementalInfo, SnapshotInfo};

/// Trait for backup storage backends.
#[async_trait]
pub trait BackupStorage: Send + Sync {
    /// Put an object into storage.
    async fn put(&self, path: &str, data: Bytes) -> Result<()>;

    /// Get an object from storage.
    async fn get(&self, path: &str) -> Result<Bytes>;

    /// Check if an object exists.
    async fn exists(&self, path: &str) -> Result<bool>;

    /// List objects with a prefix.
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Delete an object.
    async fn delete(&self, path: &str) -> Result<()>;

    /// Get the last backed up LSN.
    async fn get_last_backed_up_lsn(&self) -> Result<Option<u64>>;

    /// Set the last backed up LSN.
    async fn set_last_backed_up_lsn(&self, lsn: u64) -> Result<()>;

    /// Get backup status.
    async fn get_backup_status(&self) -> Result<BackupStatus>;

    /// Update backup status with new snapshot.
    async fn record_snapshot(&self, info: SnapshotInfo) -> Result<()>;

    /// Update backup status with new incremental.
    async fn record_incremental(&self, info: IncrementalInfo) -> Result<()>;
}

/// Create a storage backend from configuration.
pub async fn create_storage(config: &BackupConfig) -> Result<Box<dyn BackupStorage>> {
    Ok(Box::new(S3Storage::new(config).await?))
}
