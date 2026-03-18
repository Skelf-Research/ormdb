//! Persistent change log for CDC and replication.
//!
//! The changelog stores a sequence of mutations with monotonically increasing
//! Log Sequence Numbers (LSNs). This enables:
//! - Change Data Capture (CDC) for streaming changes to subscribers
//! - Replication by streaming changes to replicas
//! - Point-in-time recovery by replaying changes

use std::sync::atomic::{AtomicU64, Ordering};

use ormdb_proto::replication::ChangeLogEntry;
use sled::{Db, Tree};

use crate::error::Error;

/// Persistent changelog backed by sled.
///
/// Each entry is stored with its LSN as the key (big-endian for ordering).
/// The changelog supports:
/// - Appending new entries (returns assigned LSN)
/// - Scanning entries from a given LSN
/// - Truncating old entries for compaction
pub struct ChangeLog {
    /// The sled tree storing changelog entries.
    tree: Tree,
    /// Current (highest assigned) LSN.
    current_lsn: AtomicU64,
}

impl ChangeLog {
    /// Open or create a changelog in the given sled database.
    pub fn open(db: &Db) -> Result<Self, Error> {
        let tree = db.open_tree("changelog")?;
        let current_lsn = Self::load_last_lsn(&tree)?;

        Ok(Self {
            tree,
            current_lsn: AtomicU64::new(current_lsn),
        })
    }

    /// Load the last LSN from the tree, or 0 if empty.
    fn load_last_lsn(tree: &Tree) -> Result<u64, Error> {
        // Get the last entry (highest key)
        if let Some(result) = tree.last()? {
            let (key, _) = result;
            if key.len() == 8 {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&key);
                return Ok(u64::from_be_bytes(bytes));
            }
        }
        Ok(0)
    }

    /// Append an entry to the changelog and return its assigned LSN.
    ///
    /// The entry's LSN field will be set to the assigned value.
    pub fn append(&self, mut entry: ChangeLogEntry) -> Result<u64, Error> {
        // Atomically increment and get the new LSN
        let lsn = self.current_lsn.fetch_add(1, Ordering::SeqCst) + 1;
        entry.lsn = lsn;

        // Serialize the entry
        let value = rkyv::to_bytes::<rkyv::rancor::Error>(&entry)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store with LSN as key (big-endian for ordering)
        let key = lsn.to_be_bytes();
        self.tree.insert(key, value.as_slice())?;

        Ok(lsn)
    }

    /// Get the current (highest) LSN.
    pub fn current_lsn(&self) -> u64 {
        self.current_lsn.load(Ordering::SeqCst)
    }

    /// Get an entry by its LSN.
    pub fn get(&self, lsn: u64) -> Result<Option<ChangeLogEntry>, Error> {
        let key = lsn.to_be_bytes();
        match self.tree.get(key)? {
            Some(value) => {
                let entry = Self::deserialize_entry(&value)?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Scan entries starting from the given LSN (inclusive).
    ///
    /// Returns an iterator over entries in LSN order.
    pub fn scan_from(&self, from_lsn: u64) -> impl Iterator<Item = Result<ChangeLogEntry, Error>> + '_ {
        let start_key = from_lsn.to_be_bytes();
        self.tree.range(start_key..).map(|result| {
            let (_, value) = result?;
            Self::deserialize_entry(&value)
        })
    }

    /// Scan a batch of entries starting from the given LSN.
    ///
    /// Returns up to `batch_size` entries and a boolean indicating if there are more.
    pub fn scan_batch(
        &self,
        from_lsn: u64,
        batch_size: usize,
    ) -> Result<(Vec<ChangeLogEntry>, bool), Error> {
        let mut entries = Vec::with_capacity(batch_size.min(1000));
        let mut count = 0;
        let mut has_more = false;

        for result in self.scan_from(from_lsn) {
            if count >= batch_size {
                has_more = true;
                break;
            }
            entries.push(result?);
            count += 1;
        }

        Ok((entries, has_more))
    }

    /// Scan entries with an optional entity type filter.
    pub fn scan_filtered(
        &self,
        from_lsn: u64,
        batch_size: usize,
        entity_filter: Option<&[String]>,
    ) -> Result<(Vec<ChangeLogEntry>, bool), Error> {
        let mut entries = Vec::with_capacity(batch_size.min(1000));
        let mut has_more = false;

        for result in self.scan_from(from_lsn) {
            let entry = result?;

            // Apply entity filter if specified
            if let Some(filter) = entity_filter {
                if !filter.iter().any(|e| e == &entry.entity_type) {
                    continue;
                }
            }

            if entries.len() >= batch_size {
                has_more = true;
                break;
            }
            entries.push(entry);
        }

        Ok((entries, has_more))
    }

    /// Truncate entries before the given LSN (exclusive).
    ///
    /// This is used for compaction to remove old changelog entries.
    /// Returns the number of entries removed.
    pub fn truncate_before(&self, before_lsn: u64) -> Result<u64, Error> {
        let mut removed = 0u64;
        let end_key = before_lsn.to_be_bytes();

        // Iterate and remove entries before the given LSN
        for result in self.tree.range(..end_key) {
            let (key, _) = result?;
            self.tree.remove(key)?;
            removed += 1;
        }

        Ok(removed)
    }

    /// Get the number of entries in the changelog.
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Check if the changelog is empty.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Flush the changelog to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.tree.flush()?;
        Ok(())
    }

    /// Deserialize a changelog entry from bytes.
    fn deserialize_entry(bytes: &[u8]) -> Result<ChangeLogEntry, Error> {
        let archived = rkyv::access::<
            ormdb_proto::replication::ArchivedChangeLogEntry,
            rkyv::rancor::Error,
        >(bytes)
        .map_err(|e| Error::Deserialization(e.to_string()))?;

        rkyv::deserialize::<ChangeLogEntry, rkyv::rancor::Error>(archived)
            .map_err(|e| Error::Deserialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_proto::ChangeType;

    fn create_test_entry(entity: &str, id: [u8; 16]) -> ChangeLogEntry {
        ChangeLogEntry {
            lsn: 0,
            timestamp: 1234567890,
            entity_type: entity.to_string(),
            entity_id: id,
            change_type: ChangeType::Insert,
            changed_fields: vec!["name".to_string()],
            before_data: None,
            after_data: Some(vec![1, 2, 3, 4]),
            schema_version: 1,
        }
    }

    #[test]
    fn test_append_and_get() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let changelog = ChangeLog::open(&db).unwrap();

        assert_eq!(changelog.current_lsn(), 0);

        let entry1 = create_test_entry("User", [1u8; 16]);
        let lsn1 = changelog.append(entry1).unwrap();
        assert_eq!(lsn1, 1);
        assert_eq!(changelog.current_lsn(), 1);

        let entry2 = create_test_entry("Post", [2u8; 16]);
        let lsn2 = changelog.append(entry2).unwrap();
        assert_eq!(lsn2, 2);
        assert_eq!(changelog.current_lsn(), 2);

        // Get entries
        let retrieved1 = changelog.get(1).unwrap().unwrap();
        assert_eq!(retrieved1.entity_type, "User");
        assert_eq!(retrieved1.lsn, 1);

        let retrieved2 = changelog.get(2).unwrap().unwrap();
        assert_eq!(retrieved2.entity_type, "Post");
        assert_eq!(retrieved2.lsn, 2);

        // Non-existent LSN
        assert!(changelog.get(999).unwrap().is_none());
    }

    #[test]
    fn test_scan_from() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let changelog = ChangeLog::open(&db).unwrap();

        // Append several entries
        for i in 0..5 {
            let entry = create_test_entry(&format!("Entity{}", i), [i as u8; 16]);
            changelog.append(entry).unwrap();
        }

        // Scan from LSN 3
        let entries: Vec<_> = changelog.scan_from(3).collect::<Result<_, _>>().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].lsn, 3);
        assert_eq!(entries[1].lsn, 4);
        assert_eq!(entries[2].lsn, 5);
    }

    #[test]
    fn test_scan_batch() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let changelog = ChangeLog::open(&db).unwrap();

        // Append 10 entries
        for i in 0..10 {
            let entry = create_test_entry("User", [i as u8; 16]);
            changelog.append(entry).unwrap();
        }

        // Scan with batch size 3
        let (entries, has_more) = changelog.scan_batch(1, 3).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(has_more);
        assert_eq!(entries[0].lsn, 1);
        assert_eq!(entries[2].lsn, 3);

        // Continue from next LSN
        let (entries, has_more) = changelog.scan_batch(4, 3).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(has_more);

        // Last batch
        let (entries, has_more) = changelog.scan_batch(8, 10).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(!has_more);
    }

    #[test]
    fn test_scan_filtered() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let changelog = ChangeLog::open(&db).unwrap();

        // Append mixed entries
        changelog.append(create_test_entry("User", [1u8; 16])).unwrap();
        changelog.append(create_test_entry("Post", [2u8; 16])).unwrap();
        changelog.append(create_test_entry("User", [3u8; 16])).unwrap();
        changelog.append(create_test_entry("Comment", [4u8; 16])).unwrap();
        changelog.append(create_test_entry("User", [5u8; 16])).unwrap();

        // Filter for User only
        let filter = vec!["User".to_string()];
        let (entries, _) = changelog.scan_filtered(1, 10, Some(&filter)).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.entity_type == "User"));

        // Filter for User and Post
        let filter = vec!["User".to_string(), "Post".to_string()];
        let (entries, _) = changelog.scan_filtered(1, 10, Some(&filter)).unwrap();
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn test_truncate_before() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let changelog = ChangeLog::open(&db).unwrap();

        // Append 5 entries
        for i in 0..5 {
            let entry = create_test_entry("User", [i as u8; 16]);
            changelog.append(entry).unwrap();
        }

        assert_eq!(changelog.len(), 5);

        // Truncate before LSN 3 (removes LSNs 1, 2)
        let removed = changelog.truncate_before(3).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(changelog.len(), 3);

        // Verify remaining entries
        assert!(changelog.get(1).unwrap().is_none());
        assert!(changelog.get(2).unwrap().is_none());
        assert!(changelog.get(3).unwrap().is_some());
        assert!(changelog.get(4).unwrap().is_some());
        assert!(changelog.get(5).unwrap().is_some());
    }

    #[test]
    fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("changelog_test");

        // Write some entries
        {
            let db = sled::open(&db_path).unwrap();
            let changelog = ChangeLog::open(&db).unwrap();

            changelog.append(create_test_entry("User", [1u8; 16])).unwrap();
            changelog.append(create_test_entry("Post", [2u8; 16])).unwrap();
            changelog.flush().unwrap();
        }

        // Reopen and verify
        {
            let db = sled::open(&db_path).unwrap();
            let changelog = ChangeLog::open(&db).unwrap();

            assert_eq!(changelog.current_lsn(), 2);
            assert_eq!(changelog.len(), 2);

            let entry = changelog.get(1).unwrap().unwrap();
            assert_eq!(entry.entity_type, "User");
        }
    }

    #[test]
    fn test_lsn_continuity_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("lsn_test");

        // Write entries and close
        {
            let db = sled::open(&db_path).unwrap();
            let changelog = ChangeLog::open(&db).unwrap();

            changelog.append(create_test_entry("User", [1u8; 16])).unwrap();
            changelog.append(create_test_entry("User", [2u8; 16])).unwrap();
            changelog.flush().unwrap();
        }

        // Reopen and append more
        {
            let db = sled::open(&db_path).unwrap();
            let changelog = ChangeLog::open(&db).unwrap();

            assert_eq!(changelog.current_lsn(), 2);

            let lsn = changelog.append(create_test_entry("User", [3u8; 16])).unwrap();
            assert_eq!(lsn, 3); // Should continue from 3, not restart at 1
        }
    }
}
