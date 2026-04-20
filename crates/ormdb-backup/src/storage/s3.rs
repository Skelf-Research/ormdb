//! S3-compatible storage backend.

use bytes::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path;
use object_store::{ObjectStore, PutPayload};
use std::sync::Arc;

use super::BackupStorage;
use crate::config::BackupConfig;
use crate::error::{BackupError, Result};
use crate::{BackupChainEntry, BackupStatus, BackupType, IncrementalInfo, SnapshotInfo};

/// S3-compatible storage backend using object_store.
pub struct S3Storage {
    store: Arc<dyn ObjectStore>,
    config: BackupConfig,
}

impl S3Storage {
    /// Create a new S3 storage backend.
    pub async fn new(config: &BackupConfig) -> Result<Self> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&config.bucket)
            .with_region(&config.region);

        // Use custom endpoint for S3-compatible services
        if let Some(endpoint) = &config.endpoint {
            builder = builder.with_endpoint(endpoint).with_allow_http(true);
        }

        // Use environment variables for credentials (standard AWS pattern)
        let store = builder.build().map_err(|e| {
            BackupError::InvalidConfig(format!("failed to create S3 client: {}", e))
        })?;

        Ok(Self {
            store: Arc::new(store),
            config: config.clone(),
        })
    }

    /// Get the metadata path for backup state.
    fn backup_state_path(&self) -> Path {
        Path::from(format!("{}backup-state.json", self.config.metadata_path()))
    }

    /// Load backup state from storage.
    async fn load_backup_state(&self) -> Result<BackupState> {
        let path = self.backup_state_path();
        match self.store.get(&path).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                let state: BackupState = serde_json::from_slice(&bytes)?;
                Ok(state)
            }
            Err(object_store::Error::NotFound { .. }) => Ok(BackupState::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Save backup state to storage.
    async fn save_backup_state(&self, state: &BackupState) -> Result<()> {
        let path = self.backup_state_path();
        let bytes = serde_json::to_vec_pretty(state)?;
        self.store.put(&path, PutPayload::from(bytes)).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl BackupStorage for S3Storage {
    async fn put(&self, path: &str, data: Bytes) -> Result<()> {
        let full_path = Path::from(format!("{}{}", self.config.prefix, path));
        self.store.put(&full_path, PutPayload::from_bytes(data)).await?;
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Bytes> {
        let full_path = Path::from(format!("{}{}", self.config.prefix, path));
        let result = self.store.get(&full_path).await?;
        Ok(result.bytes().await?)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let full_path = Path::from(format!("{}{}", self.config.prefix, path));
        match self.store.head(&full_path).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        use futures::TryStreamExt;

        let full_prefix = Path::from(format!("{}{}", self.config.prefix, prefix));
        let list_result = self.store.list(Some(&full_prefix));

        let objects: Vec<_> = list_result.try_collect().await?;
        Ok(objects.into_iter().map(|o| o.location.to_string()).collect())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let full_path = Path::from(format!("{}{}", self.config.prefix, path));
        self.store.delete(&full_path).await?;
        Ok(())
    }

    async fn get_last_backed_up_lsn(&self) -> Result<Option<u64>> {
        let state = self.load_backup_state().await?;
        Ok(state.last_backed_up_lsn)
    }

    async fn set_last_backed_up_lsn(&self, lsn: u64) -> Result<()> {
        let mut state = self.load_backup_state().await?;
        state.last_backed_up_lsn = Some(lsn);
        self.save_backup_state(&state).await
    }

    async fn get_backup_status(&self) -> Result<BackupStatus> {
        let state = self.load_backup_state().await?;
        Ok(BackupStatus {
            latest_snapshot: state.latest_snapshot,
            latest_incremental: state.latest_incremental,
            backup_chain: state.backup_chain,
        })
    }

    async fn record_snapshot(&self, info: SnapshotInfo) -> Result<()> {
        let mut state = self.load_backup_state().await?;

        // Add to chain
        state.backup_chain.push(BackupChainEntry {
            backup_type: BackupType::Full,
            lsn_start: 0,
            lsn_end: info.lsn,
            path: info.path.clone(),
            timestamp: info.timestamp,
        });

        // Update latest
        state.latest_snapshot = Some(info);
        state.last_backed_up_lsn = state.latest_snapshot.as_ref().map(|s| s.lsn);

        self.save_backup_state(&state).await
    }

    async fn record_incremental(&self, info: IncrementalInfo) -> Result<()> {
        let mut state = self.load_backup_state().await?;

        // Add to chain
        state.backup_chain.push(BackupChainEntry {
            backup_type: BackupType::Incremental,
            lsn_start: info.lsn_start,
            lsn_end: info.lsn_end,
            path: info.path.clone(),
            timestamp: info.timestamp,
        });

        // Update latest
        state.last_backed_up_lsn = Some(info.lsn_end);
        state.latest_incremental = Some(info);

        self.save_backup_state(&state).await
    }
}

/// Internal backup state stored in S3.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct BackupState {
    last_backed_up_lsn: Option<u64>,
    latest_snapshot: Option<SnapshotInfo>,
    latest_incremental: Option<IncrementalInfo>,
    backup_chain: Vec<BackupChainEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_state_serialization() {
        let state = BackupState {
            last_backed_up_lsn: Some(12345),
            latest_snapshot: Some(SnapshotInfo {
                timestamp: chrono::Utc::now(),
                lsn: 12345,
                path: "snapshots/2024-01-01/".to_string(),
                total_bytes: 1000000,
            }),
            latest_incremental: None,
            backup_chain: vec![],
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: BackupState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.last_backed_up_lsn, Some(12345));
    }
}
