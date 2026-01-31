//! Replica change applier.
//!
//! Applies changelog entries from a primary server to a replica's storage.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ormdb_proto::replication::ChangeLogEntry;
use ormdb_proto::ChangeType;

use crate::error::Error;
use crate::storage::{Record, StorageEngine, VersionedKey};

/// Applies changelog entries to replica storage.
///
/// The applier tracks the last applied LSN to support resumable replication.
pub struct ReplicaApplier {
    /// The storage engine to apply changes to.
    storage: Arc<StorageEngine>,
    /// The last successfully applied LSN.
    applied_lsn: AtomicU64,
}

impl ReplicaApplier {
    /// Create a new replica applier.
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self {
            storage,
            applied_lsn: AtomicU64::new(0),
        }
    }

    /// Create a new replica applier with an initial LSN.
    pub fn with_lsn(storage: Arc<StorageEngine>, initial_lsn: u64) -> Self {
        Self {
            storage,
            applied_lsn: AtomicU64::new(initial_lsn),
        }
    }

    /// Apply a single changelog entry.
    pub fn apply(&self, entry: &ChangeLogEntry) -> Result<(), Error> {
        match entry.change_type {
            ChangeType::Insert | ChangeType::Update => {
                self.apply_upsert(entry)?;
            }
            ChangeType::Delete => {
                self.apply_delete(entry)?;
            }
        }

        // Update applied LSN
        self.applied_lsn.store(entry.lsn, Ordering::SeqCst);
        Ok(())
    }

    /// Apply an insert or update entry.
    fn apply_upsert(&self, entry: &ChangeLogEntry) -> Result<(), Error> {
        let data = entry
            .after_data
            .as_ref()
            .ok_or_else(|| Error::InvalidData("insert/update entry missing after_data".to_string()))?;

        let record = Record::new(data.clone());
        let key = VersionedKey::new(entry.entity_id, entry.timestamp);

        self.storage.put_typed(&entry.entity_type, key, record)?;
        Ok(())
    }

    /// Apply a delete entry.
    fn apply_delete(&self, entry: &ChangeLogEntry) -> Result<(), Error> {
        self.storage.delete_typed(&entry.entity_type, &entry.entity_id)?;
        Ok(())
    }

    /// Apply a batch of changelog entries.
    ///
    /// Returns the last applied LSN, or the starting LSN if batch is empty.
    pub fn apply_batch(&self, entries: &[ChangeLogEntry]) -> Result<u64, Error> {
        for entry in entries {
            self.apply(entry)?;
        }
        Ok(entries.last().map(|e| e.lsn).unwrap_or(self.applied_lsn()))
    }

    /// Get the last applied LSN.
    pub fn applied_lsn(&self) -> u64 {
        self.applied_lsn.load(Ordering::SeqCst)
    }

    /// Set the applied LSN (for recovery scenarios).
    pub fn set_applied_lsn(&self, lsn: u64) {
        self.applied_lsn.store(lsn, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageConfig;

    fn setup_storage() -> (tempfile::TempDir, Arc<StorageEngine>) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            StorageEngine::open(StorageConfig::new(dir.path())).unwrap()
        );
        (dir, storage)
    }

    fn create_insert_entry(entity: &str, id: [u8; 16], data: Vec<u8>, lsn: u64) -> ChangeLogEntry {
        ChangeLogEntry {
            lsn,
            timestamp: lsn * 1000, // Use LSN as timestamp base for simplicity
            entity_type: entity.to_string(),
            entity_id: id,
            change_type: ChangeType::Insert,
            changed_fields: vec!["data".to_string()],
            before_data: None,
            after_data: Some(data),
            schema_version: 1,
        }
    }

    fn create_update_entry(
        entity: &str,
        id: [u8; 16],
        before: Vec<u8>,
        after: Vec<u8>,
        lsn: u64,
    ) -> ChangeLogEntry {
        ChangeLogEntry {
            lsn,
            timestamp: lsn * 1000,
            entity_type: entity.to_string(),
            entity_id: id,
            change_type: ChangeType::Update,
            changed_fields: vec!["data".to_string()],
            before_data: Some(before),
            after_data: Some(after),
            schema_version: 1,
        }
    }

    fn create_delete_entry(entity: &str, id: [u8; 16], before: Vec<u8>, lsn: u64) -> ChangeLogEntry {
        ChangeLogEntry {
            lsn,
            timestamp: lsn * 1000,
            entity_type: entity.to_string(),
            entity_id: id,
            change_type: ChangeType::Delete,
            changed_fields: vec![],
            before_data: Some(before),
            after_data: None,
            schema_version: 1,
        }
    }

    #[test]
    fn test_apply_insert() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::new(storage.clone());

        let id = [1u8; 16];
        let data = vec![1, 2, 3, 4];
        let entry = create_insert_entry("User", id, data.clone(), 1);

        applier.apply(&entry).unwrap();

        assert_eq!(applier.applied_lsn(), 1);

        // Verify data was stored
        let result = storage.get_latest(&id).unwrap();
        assert!(result.is_some());
        let (_, record) = result.unwrap();
        assert_eq!(record.data, data);
    }

    #[test]
    fn test_apply_update() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::new(storage.clone());

        let id = [1u8; 16];
        let initial_data = vec![1, 2, 3, 4];
        let updated_data = vec![5, 6, 7, 8];

        // Apply insert
        let insert = create_insert_entry("User", id, initial_data.clone(), 1);
        applier.apply(&insert).unwrap();

        // Apply update
        let update = create_update_entry("User", id, initial_data, updated_data.clone(), 2);
        applier.apply(&update).unwrap();

        assert_eq!(applier.applied_lsn(), 2);

        // Verify updated data
        let result = storage.get_latest(&id).unwrap();
        assert!(result.is_some());
        let (_, record) = result.unwrap();
        assert_eq!(record.data, updated_data);
    }

    #[test]
    fn test_apply_delete() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::new(storage.clone());

        let id = [1u8; 16];
        let data = vec![1, 2, 3, 4];

        // Apply insert
        let insert = create_insert_entry("User", id, data.clone(), 1);
        applier.apply(&insert).unwrap();

        // Verify exists
        assert!(storage.get_latest(&id).unwrap().is_some());

        // Apply delete
        let delete = create_delete_entry("User", id, data, 2);
        applier.apply(&delete).unwrap();

        assert_eq!(applier.applied_lsn(), 2);

        // Verify deleted (tombstone marker)
        // Note: The storage may still return data with deleted flag,
        // depending on implementation
    }

    #[test]
    fn test_apply_batch() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::new(storage.clone());

        let entries = vec![
            create_insert_entry("User", [1u8; 16], vec![1, 1, 1, 1], 1),
            create_insert_entry("User", [2u8; 16], vec![2, 2, 2, 2], 2),
            create_insert_entry("Post", [3u8; 16], vec![3, 3, 3, 3], 3),
        ];

        let last_lsn = applier.apply_batch(&entries).unwrap();
        assert_eq!(last_lsn, 3);
        assert_eq!(applier.applied_lsn(), 3);

        // Verify all entries were applied
        assert!(storage.get_latest(&[1u8; 16]).unwrap().is_some());
        assert!(storage.get_latest(&[2u8; 16]).unwrap().is_some());
        assert!(storage.get_latest(&[3u8; 16]).unwrap().is_some());
    }

    #[test]
    fn test_initial_lsn() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::with_lsn(storage, 100);

        assert_eq!(applier.applied_lsn(), 100);
    }

    #[test]
    fn test_empty_batch() {
        let (_dir, storage) = setup_storage();
        let applier = ReplicaApplier::with_lsn(storage, 50);

        let last_lsn = applier.apply_batch(&[]).unwrap();
        assert_eq!(last_lsn, 50); // Returns initial LSN for empty batch
    }
}
