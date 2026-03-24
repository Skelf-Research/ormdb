//! Snapshot building and restoration for Raft.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use anyerror::AnyError;
use openraft::storage::{RaftSnapshotBuilder, Snapshot};
use openraft::{BasicNode, LogId, StorageError, StorageIOError};

use ormdb_core::storage::StorageEngine;

use crate::types::{NodeId, SnapshotMeta, StoredMembership, TypeConfig};

/// Builds snapshots of the ORMDB state.
///
/// Snapshots capture the current state of the database at a specific log position,
/// allowing new nodes to quickly catch up without replaying the entire log.
pub struct SnapshotBuilder {
    /// The storage engine to snapshot.
    storage: Arc<StorageEngine>,
    /// Directory to store snapshots.
    snapshot_dir: PathBuf,
    /// Last applied log ID at snapshot time.
    last_applied: Option<LogId<NodeId>>,
    /// Current membership at snapshot time.
    membership: StoredMembership,
}

impl SnapshotBuilder {
    /// Create a new snapshot builder.
    pub fn new(
        storage: Arc<StorageEngine>,
        snapshot_dir: PathBuf,
        last_applied: Option<LogId<NodeId>>,
        membership: StoredMembership,
    ) -> Self {
        Self {
            storage,
            snapshot_dir,
            last_applied,
            membership,
        }
    }

    /// Generate a unique snapshot ID.
    fn generate_snapshot_id(&self) -> String {
        let index = self.last_applied.map(|l| l.index).unwrap_or(0);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        format!("snap-{}-{}", index, timestamp)
    }
}

impl RaftSnapshotBuilder<TypeConfig> for SnapshotBuilder {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let snapshot_id = self.generate_snapshot_id();

        // Create snapshot metadata
        let meta: SnapshotMeta = openraft::SnapshotMeta {
            last_log_id: self.last_applied,
            last_membership: self.membership.clone(),
            snapshot_id: snapshot_id.clone(),
        };

        // Build snapshot data
        // For a production implementation, this would:
        // 1. Create a consistent view of the database
        // 2. Serialize all entity data
        // 3. Compress the data
        //
        // For now, we create a minimal snapshot with just metadata
        let snapshot_data = self.build_snapshot_data().await
            .map_err(|e| StorageIOError::write_snapshot(None, AnyError::new(&e)))?;

        // Save snapshot to disk for durability
        self.save_snapshot(&meta, &snapshot_data)
            .map_err(|e| StorageIOError::write_snapshot(None, AnyError::new(&e)))?;

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(snapshot_data)),
        })
    }
}

impl SnapshotBuilder {
    /// Build the snapshot data.
    async fn build_snapshot_data(&self) -> Result<Vec<u8>, std::io::Error> {
        // Create a snapshot structure
        let snapshot = SnapshotData {
            version: 1,
            last_log_index: self.last_applied.map(|l| l.index).unwrap_or(0),
            last_log_term: self.last_applied.map(|l| l.leader_id.term).unwrap_or(0),
            // In a full implementation, we would include:
            // - All entity data from storage
            // - Catalog/schema information
            // - Index data
            data: Vec::new(),
        };

        serde_json::to_vec(&snapshot).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })
    }

    /// Save snapshot to disk.
    fn save_snapshot(&self, meta: &SnapshotMeta, data: &[u8]) -> Result<(), std::io::Error> {
        // Ensure snapshot directory exists
        std::fs::create_dir_all(&self.snapshot_dir)?;

        // Save metadata
        let meta_path = self.snapshot_dir.join("current.meta");
        let meta_bytes = serde_json::to_vec(meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(&meta_path, meta_bytes)?;

        // Save data
        let data_path = self.snapshot_dir.join("current.snap");
        std::fs::write(&data_path, data)?;

        tracing::info!(
            "Saved snapshot {} with {} bytes",
            meta.snapshot_id,
            data.len()
        );

        Ok(())
    }
}

/// Snapshot data structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SnapshotData {
    /// Snapshot format version.
    version: u32,
    /// Last log index included in snapshot.
    last_log_index: u64,
    /// Last log term included in snapshot.
    last_log_term: u64,
    /// Serialized database state.
    data: Vec<u8>,
}

/// Restore database state from a snapshot.
pub struct SnapshotRestorer {
    storage: Arc<StorageEngine>,
}

impl SnapshotRestorer {
    /// Create a new snapshot restorer.
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self { storage }
    }

    /// Restore state from snapshot data.
    pub async fn restore(&self, data: &[u8]) -> Result<(), std::io::Error> {
        if data.is_empty() {
            return Ok(());
        }

        let snapshot: SnapshotData = serde_json::from_slice(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        tracing::info!(
            "Restoring snapshot version {} at index {}",
            snapshot.version,
            snapshot.last_log_index
        );

        // In a full implementation, we would:
        // 1. Clear existing data
        // 2. Deserialize and insert all entities
        // 3. Rebuild indexes
        //
        // For now, we just validate the snapshot format

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::StorageConfig;
    use std::collections::BTreeSet;
    use crate::types::Membership;

    fn create_test_storage() -> Arc<StorageEngine> {
        let config = StorageConfig::temporary();
        Arc::new(StorageEngine::open(config).unwrap())
    }

    #[tokio::test]
    async fn test_build_snapshot() {
        let storage = create_test_storage();
        let snapshot_dir = tempfile::tempdir().unwrap();

        let last_applied = Some(LogId::new(openraft::CommittedLeaderId::new(1, 1), 10));
        let membership = Membership::new(vec![BTreeSet::from([1, 2, 3])], None);
        let stored_membership = StoredMembership::new(None, membership.clone());

        let mut builder = SnapshotBuilder::new(
            storage,
            snapshot_dir.path().to_path_buf(),
            last_applied,
            stored_membership,
        );

        let snapshot = builder.build_snapshot().await.unwrap();

        // Verify metadata
        assert_eq!(snapshot.meta.last_log_id, last_applied);
        assert!(snapshot.meta.snapshot_id.starts_with("snap-10-"));

        // Verify files were created
        assert!(snapshot_dir.path().join("current.meta").exists());
        assert!(snapshot_dir.path().join("current.snap").exists());
    }

    #[tokio::test]
    async fn test_snapshot_id_generation() {
        let storage = create_test_storage();
        let snapshot_dir = tempfile::tempdir().unwrap();

        let builder = SnapshotBuilder::new(
            storage,
            snapshot_dir.path().to_path_buf(),
            Some(LogId::new(openraft::CommittedLeaderId::new(1, 1), 42)),
            StoredMembership::new(None, Membership::new(vec![], None)),
        );

        let id = builder.generate_snapshot_id();
        assert!(id.starts_with("snap-42-"));
    }

    #[tokio::test]
    async fn test_restore_empty_snapshot() {
        let storage = create_test_storage();
        let restorer = SnapshotRestorer::new(storage);

        // Empty data should succeed
        restorer.restore(&[]).await.unwrap();
    }

    #[tokio::test]
    async fn test_restore_snapshot() {
        let storage = create_test_storage();
        let restorer = SnapshotRestorer::new(storage);

        let snapshot = SnapshotData {
            version: 1,
            last_log_index: 100,
            last_log_term: 5,
            data: Vec::new(),
        };

        let bytes = serde_json::to_vec(&snapshot).unwrap();
        restorer.restore(&bytes).await.unwrap();
    }
}
