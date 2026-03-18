//! Storage engine implementation.

use super::{Record, StorageConfig, VersionedKey};
use crate::error::Error;
use sled::{Db, Tree};

/// Tree name for entity data.
const DATA_TREE: &str = "data";

/// Tree name for metadata (latest versions, etc.).
const META_TREE: &str = "meta";

/// Tree name for entity type index.
const TYPE_INDEX_TREE: &str = "index:entity_type";

/// Prefix for latest version pointers in meta tree.
const LATEST_PREFIX: &[u8] = b"latest:";

/// The main storage engine wrapping sled.
pub struct StorageEngine {
    /// The underlying sled database.
    db: Db,

    /// Tree for entity data (versioned records).
    data_tree: Tree,

    /// Tree for metadata.
    meta_tree: Tree,

    /// Tree for entity type index (entity_type + entity_id -> empty).
    type_index_tree: Tree,
}

impl StorageEngine {
    /// Open or create a storage engine with the given configuration.
    pub fn open(config: StorageConfig) -> Result<Self, Error> {
        let sled_config = config.to_sled_config();
        let db = sled_config.open()?;
        let data_tree = db.open_tree(DATA_TREE)?;
        let meta_tree = db.open_tree(META_TREE)?;
        let type_index_tree = db.open_tree(TYPE_INDEX_TREE)?;

        Ok(Self {
            db,
            data_tree,
            meta_tree,
            type_index_tree,
        })
    }

    /// Check if the database was recovered from a previous crash.
    pub fn was_recovered(&self) -> bool {
        self.db.was_recovered()
    }

    /// Put a new versioned record.
    ///
    /// This creates a new version of the entity, never overwriting existing versions.
    pub fn put(&self, key: VersionedKey, record: Record) -> Result<(), Error> {
        let key_bytes = key.encode();
        let value_bytes = record.to_bytes()?;

        // Insert the versioned record
        self.data_tree.insert(key_bytes, value_bytes)?;

        // Update the latest version pointer
        self.update_latest(&key.entity_id, key.version_ts)?;

        Ok(())
    }

    /// Get a specific version of an entity.
    pub fn get(&self, entity_id: &[u8; 16], version_ts: u64) -> Result<Option<Record>, Error> {
        let key = VersionedKey::new(*entity_id, version_ts);
        let key_bytes = key.encode();

        match self.data_tree.get(key_bytes)? {
            Some(bytes) => {
                let record = Record::from_bytes(&bytes)?;
                if record.deleted {
                    Ok(None)
                } else {
                    Ok(Some(record))
                }
            }
            None => Ok(None),
        }
    }

    /// Get the latest version of an entity.
    ///
    /// Returns the version timestamp and record if found.
    pub fn get_latest(&self, entity_id: &[u8; 16]) -> Result<Option<(u64, Record)>, Error> {
        // Get the latest version timestamp from metadata
        let latest_key = self.latest_key(entity_id);
        let version_ts = match self.meta_tree.get(&latest_key)? {
            Some(bytes) => {
                let mut ts_bytes = [0u8; 8];
                ts_bytes.copy_from_slice(&bytes);
                u64::from_be_bytes(ts_bytes)
            }
            None => return Ok(None),
        };

        // Get the record at that version
        match self.get(entity_id, version_ts)? {
            Some(record) => Ok(Some((version_ts, record))),
            None => Ok(None),
        }
    }

    /// Get the version of an entity at or before a given timestamp.
    ///
    /// This is useful for point-in-time queries.
    pub fn get_at(&self, entity_id: &[u8; 16], at_ts: u64) -> Result<Option<(u64, Record)>, Error> {
        // Scan backwards from the requested timestamp
        let max_key = VersionedKey::new(*entity_id, at_ts);
        let min_key = VersionedKey::min_for_entity(*entity_id);

        for result in self
            .data_tree
            .range(min_key.encode()..=max_key.encode())
            .rev()
        {
            let (key_bytes, value_bytes) = result?;
            let key = VersionedKey::decode(&key_bytes).ok_or(Error::InvalidKey)?;

            // Verify this is for the correct entity
            if key.entity_id != *entity_id {
                continue;
            }

            let record = Record::from_bytes(&value_bytes)?;
            if !record.deleted {
                return Ok(Some((key.version_ts, record)));
            }
        }

        Ok(None)
    }

    /// Scan all versions of an entity.
    ///
    /// Returns versions in chronological order (oldest first).
    pub fn scan_versions(
        &self,
        entity_id: &[u8; 16],
    ) -> impl Iterator<Item = Result<(u64, Record), Error>> + '_ {
        let min_key = VersionedKey::min_for_entity(*entity_id);
        let max_key = VersionedKey::max_for_entity(*entity_id);
        let entity_id = *entity_id;

        self.data_tree
            .range(min_key.encode()..=max_key.encode())
            .map(move |result| {
                let (key_bytes, value_bytes) = result?;
                let key = VersionedKey::decode(&key_bytes).ok_or(Error::InvalidKey)?;

                // Verify this is for the correct entity
                if key.entity_id != entity_id {
                    return Err(Error::InvalidKey);
                }

                let record = Record::from_bytes(&value_bytes)?;
                Ok((key.version_ts, record))
            })
    }

    /// Soft delete an entity by writing a tombstone record.
    pub fn delete(&self, entity_id: &[u8; 16]) -> Result<u64, Error> {
        let key = VersionedKey::now(*entity_id);
        let record = Record::tombstone();

        self.put(key, record)?;
        Ok(key.version_ts)
    }

    // ========== Entity Type-Aware Methods ==========

    /// Put a versioned record with entity type indexing.
    ///
    /// This stores the record and also indexes it by entity type for efficient scanning.
    pub fn put_typed(
        &self,
        entity_type: &str,
        key: VersionedKey,
        record: Record,
    ) -> Result<(), Error> {
        // Store the record using the standard put
        self.put(key, record)?;

        // Add to entity type index
        let index_key = self.type_index_key(entity_type, &key.entity_id);
        self.type_index_tree.insert(index_key, &[])?;

        Ok(())
    }

    /// Soft delete an entity with type indexing.
    ///
    /// Note: We don't remove from the type index because the entity still exists
    /// as a tombstone. The scan will filter out deleted entities.
    pub fn delete_typed(&self, entity_type: &str, entity_id: &[u8; 16]) -> Result<u64, Error> {
        let _ = entity_type; // Type index entry remains (can still scan history)
        self.delete(entity_id)
    }

    /// Scan all entities of a given type.
    ///
    /// Returns an iterator over (entity_id, version_ts, Record) tuples for all
    /// non-deleted entities of the specified type.
    pub fn scan_entity_type(
        &self,
        entity_type: &str,
    ) -> impl Iterator<Item = Result<([u8; 16], u64, Record), Error>> + '_ {
        let prefix = self.type_index_prefix(entity_type);
        let prefix_len = prefix.len();

        self.type_index_tree
            .scan_prefix(&prefix)
            .filter_map(move |result| {
                match result {
                    Ok((key, _)) => {
                        // Extract entity_id from index key (after the prefix)
                        if key.len() != prefix_len + 16 {
                            return Some(Err(Error::InvalidKey));
                        }
                        let mut entity_id = [0u8; 16];
                        entity_id.copy_from_slice(&key[prefix_len..]);

                        // Get the latest version of this entity
                        match self.get_latest(&entity_id) {
                            Ok(Some((version_ts, record))) => {
                                Some(Ok((entity_id, version_ts, record)))
                            }
                            Ok(None) => None, // Deleted or doesn't exist
                            Err(e) => Some(Err(e)),
                        }
                    }
                    Err(e) => Some(Err(e.into())),
                }
            })
    }

    /// Get all entity IDs of a given type (including deleted).
    ///
    /// This is useful for getting all IDs without loading the records.
    pub fn list_entity_ids(&self, entity_type: &str) -> impl Iterator<Item = Result<[u8; 16], Error>> + '_ {
        let prefix = self.type_index_prefix(entity_type);
        let prefix_len = prefix.len();

        self.type_index_tree.scan_prefix(&prefix).map(move |result| {
            let (key, _) = result?;
            if key.len() != prefix_len + 16 {
                return Err(Error::InvalidKey);
            }
            let mut entity_id = [0u8; 16];
            entity_id.copy_from_slice(&key[prefix_len..]);
            Ok(entity_id)
        })
    }

    /// Get the index key for an entity type + entity ID.
    fn type_index_key(&self, entity_type: &str, entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(entity_type.len() + 1 + 16);
        key.extend_from_slice(entity_type.as_bytes());
        key.push(0); // Null separator
        key.extend_from_slice(entity_id);
        key
    }

    /// Get the prefix for scanning all entities of a type.
    fn type_index_prefix(&self, entity_type: &str) -> Vec<u8> {
        let mut prefix = Vec::with_capacity(entity_type.len() + 1);
        prefix.extend_from_slice(entity_type.as_bytes());
        prefix.push(0); // Null separator
        prefix
    }

    // ========== End Entity Type-Aware Methods ==========

    /// Flush all pending writes to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush()?;
        Ok(())
    }

    /// Get database size in bytes.
    pub fn size_on_disk(&self) -> Result<u64, Error> {
        Ok(self.db.size_on_disk()?)
    }

    /// Generate a new entity ID (UUID v4 bytes).
    pub fn generate_id() -> [u8; 16] {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        // Counter to ensure uniqueness even with same timestamp
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // Combine timestamp with monotonically increasing counter
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);

        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&now.to_le_bytes());
        id[8..16].copy_from_slice(&counter.to_le_bytes());

        // Set UUID version 4 bits
        id[6] = (id[6] & 0x0f) | 0x40;
        id[8] = (id[8] & 0x3f) | 0x80;

        id
    }

    /// Update the latest version pointer for an entity.
    fn update_latest(&self, entity_id: &[u8; 16], version_ts: u64) -> Result<(), Error> {
        let latest_key = self.latest_key(entity_id);
        self.meta_tree
            .insert(&latest_key, &version_ts.to_be_bytes())?;
        Ok(())
    }

    /// Get the metadata key for the latest version pointer.
    fn latest_key(&self, entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(LATEST_PREFIX.len() + 16);
        key.extend_from_slice(LATEST_PREFIX);
        key.extend_from_slice(entity_id);
        key
    }

    /// Get access to the underlying data tree (for transactions).
    pub(crate) fn data_tree(&self) -> &Tree {
        &self.data_tree
    }

    /// Get access to the underlying meta tree (for transactions).
    pub(crate) fn meta_tree(&self) -> &Tree {
        &self.meta_tree
    }

    /// Get access to the type index tree (for transactions).
    pub(crate) fn type_index_tree(&self) -> &Tree {
        &self.type_index_tree
    }

    /// Get the underlying sled database (for opening new trees).
    pub fn db(&self) -> &Db {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDb {
        engine: StorageEngine,
        _dir: tempfile::TempDir, // Keep the temp dir alive
    }

    impl std::ops::Deref for TestDb {
        type Target = StorageEngine;
        fn deref(&self) -> &Self::Target {
            &self.engine
        }
    }

    fn test_engine() -> TestDb {
        let dir = tempfile::tempdir().unwrap();
        let engine = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();
        TestDb { engine, _dir: dir }
    }

    #[test]
    fn test_put_and_get() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();
        let record = Record::new(vec![1, 2, 3, 4, 5]);
        let key = VersionedKey::now(entity_id);

        engine.put(key, record.clone()).unwrap();

        let retrieved = engine.get(&entity_id, key.version_ts).unwrap().unwrap();
        assert_eq!(retrieved.data, record.data);
    }

    #[test]
    fn test_get_latest() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        // Insert multiple versions
        let record1 = Record::new(vec![1]);
        let key1 = VersionedKey::new(entity_id, 100);
        engine.put(key1, record1).unwrap();

        let record2 = Record::new(vec![2]);
        let key2 = VersionedKey::new(entity_id, 200);
        engine.put(key2, record2.clone()).unwrap();

        let record3 = Record::new(vec![3]);
        let key3 = VersionedKey::new(entity_id, 300);
        engine.put(key3, record3.clone()).unwrap();

        // Get latest should return version 300
        let (version, latest) = engine.get_latest(&entity_id).unwrap().unwrap();
        assert_eq!(version, 300);
        assert_eq!(latest.data, vec![3]);
    }

    #[test]
    fn test_get_at_timestamp() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        // Insert versions at timestamps 100, 200, 300
        engine
            .put(VersionedKey::new(entity_id, 100), Record::new(vec![1]))
            .unwrap();
        engine
            .put(VersionedKey::new(entity_id, 200), Record::new(vec![2]))
            .unwrap();
        engine
            .put(VersionedKey::new(entity_id, 300), Record::new(vec![3]))
            .unwrap();

        // Query at timestamp 150 should return version 100
        let (version, record) = engine.get_at(&entity_id, 150).unwrap().unwrap();
        assert_eq!(version, 100);
        assert_eq!(record.data, vec![1]);

        // Query at timestamp 250 should return version 200
        let (version, record) = engine.get_at(&entity_id, 250).unwrap().unwrap();
        assert_eq!(version, 200);
        assert_eq!(record.data, vec![2]);
    }

    #[test]
    fn test_scan_versions() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        engine
            .put(VersionedKey::new(entity_id, 100), Record::new(vec![1]))
            .unwrap();
        engine
            .put(VersionedKey::new(entity_id, 200), Record::new(vec![2]))
            .unwrap();
        engine
            .put(VersionedKey::new(entity_id, 300), Record::new(vec![3]))
            .unwrap();

        let versions: Vec<_> = engine
            .scan_versions(&entity_id)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].0, 100);
        assert_eq!(versions[1].0, 200);
        assert_eq!(versions[2].0, 300);
    }

    #[test]
    fn test_soft_delete() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        // Insert a record
        let key = VersionedKey::new(entity_id, 100);
        engine.put(key, Record::new(vec![1, 2, 3])).unwrap();

        // Verify it exists
        assert!(engine.get_latest(&entity_id).unwrap().is_some());

        // Soft delete
        engine.delete(&entity_id).unwrap();

        // get_latest should return None (tombstone)
        assert!(engine.get_latest(&entity_id).unwrap().is_none());

        // But we can still get the old version directly
        let old = engine.get(&entity_id, 100).unwrap().unwrap();
        assert_eq!(old.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let config = StorageConfig::new(dir.path());

        let entity_id = StorageEngine::generate_id();
        let key = VersionedKey::new(entity_id, 12345);

        // Write data
        {
            let engine = StorageEngine::open(config.clone()).unwrap();
            engine.put(key, Record::new(vec![1, 2, 3])).unwrap();
            engine.flush().unwrap();
        }

        // Reopen and verify
        {
            let engine = StorageEngine::open(config).unwrap();
            let record = engine.get(&entity_id, 12345).unwrap().unwrap();
            assert_eq!(record.data, vec![1, 2, 3]);
        }
    }

    #[test]
    fn test_put_typed_and_scan() {
        let engine = test_engine();

        // Create entities of different types
        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        let post1_id = StorageEngine::generate_id();

        // Insert users
        engine
            .put_typed(
                "ScanTestUser",
                VersionedKey::new(user1_id, 100),
                Record::new(vec![1]),
            )
            .unwrap();
        engine
            .put_typed(
                "ScanTestUser",
                VersionedKey::new(user2_id, 100),
                Record::new(vec![2]),
            )
            .unwrap();

        // Insert post
        engine
            .put_typed(
                "ScanTestPost",
                VersionedKey::new(post1_id, 100),
                Record::new(vec![3]),
            )
            .unwrap();

        // Flush to ensure data is persisted
        engine.flush().unwrap();

        // Scan users - should return 2
        let users: Vec<_> = engine
            .scan_entity_type("ScanTestUser")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(users.len(), 2);

        // Scan posts - should return 1
        let posts: Vec<_> = engine
            .scan_entity_type("ScanTestPost")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, post1_id);
        assert_eq!(posts[0].2.data, vec![3]);

        // Scan unknown type - should return 0
        let comments: Vec<_> = engine
            .scan_entity_type("ScanTestComment")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(comments.len(), 0);
    }

    #[test]
    fn test_scan_excludes_deleted() {
        let engine = test_engine();

        let id1 = StorageEngine::generate_id();
        let id2 = StorageEngine::generate_id();

        // Insert two entities
        engine
            .put_typed("DeleteTestUser", VersionedKey::new(id1, 100), Record::new(vec![1]))
            .unwrap();
        engine
            .put_typed("DeleteTestUser", VersionedKey::new(id2, 100), Record::new(vec![2]))
            .unwrap();

        // Flush to ensure data is persisted
        engine.flush().unwrap();

        // Both should be returned
        let users: Vec<_> = engine
            .scan_entity_type("DeleteTestUser")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(users.len(), 2);

        // Delete one
        engine.delete_typed("DeleteTestUser", &id1).unwrap();
        engine.flush().unwrap();

        // Now only one should be returned
        let users: Vec<_> = engine
            .scan_entity_type("DeleteTestUser")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].0, id2);
    }

    #[test]
    fn test_list_entity_ids() {
        let engine = test_engine();

        let id1 = StorageEngine::generate_id();
        let id2 = StorageEngine::generate_id();

        engine
            .put_typed("ListTestItem", VersionedKey::new(id1, 100), Record::new(vec![1]))
            .unwrap();
        engine
            .put_typed("ListTestItem", VersionedKey::new(id2, 100), Record::new(vec![2]))
            .unwrap();

        // Flush to ensure data is persisted
        engine.flush().unwrap();

        let ids: Vec<_> = engine
            .list_entity_ids("ListTestItem")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}
