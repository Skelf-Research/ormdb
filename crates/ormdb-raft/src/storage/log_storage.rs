//! Raft log storage implementation using sled.

use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;

use anyerror::AnyError;
use openraft::storage::{LogFlushed, RaftLogReader, RaftLogStorage};
use openraft::{Entry, LogId, LogState, OptionalSend, StorageError, StorageIOError, Vote};
use parking_lot::RwLock;
use sled::{Db, Tree};

use crate::error::RaftError;
use crate::types::{NodeId, TypeConfig};

/// Tree names for Raft storage in sled.
const RAFT_LOG_TREE: &str = "raft_log";
const RAFT_VOTE_TREE: &str = "raft_vote";
const RAFT_STATE_TREE: &str = "raft_state";

/// Keys in the state tree.
const KEY_LAST_PURGED: &[u8] = b"last_purged_log_id";

/// Raft log storage backed by sled.
///
/// Stores:
/// - Log entries in `raft_log` tree (key = index as big-endian u64)
/// - Vote in `raft_vote` tree
/// - State metadata in `raft_state` tree (last purged, committed index)
pub struct SledRaftLogStorage {
    /// The sled database.
    db: Arc<Db>,
    /// Tree for log entries.
    log_tree: Tree,
    /// Tree for vote storage.
    vote_tree: Tree,
    /// Tree for state metadata.
    state_tree: Tree,
    /// Cached last purged log ID.
    last_purged: RwLock<Option<LogId<NodeId>>>,
}

impl SledRaftLogStorage {
    /// Open or create a new Raft log storage in the given sled database.
    pub fn open(db: Arc<Db>) -> Result<Self, RaftError> {
        let log_tree = db.open_tree(RAFT_LOG_TREE)?;
        let vote_tree = db.open_tree(RAFT_VOTE_TREE)?;
        let state_tree = db.open_tree(RAFT_STATE_TREE)?;

        // Load last purged from state
        let last_purged = Self::load_last_purged(&state_tree)?;

        Ok(Self {
            db,
            log_tree,
            vote_tree,
            state_tree,
            last_purged: RwLock::new(last_purged),
        })
    }

    /// Encode log index as key (big-endian for lexicographic ordering).
    fn log_key(index: u64) -> [u8; 8] {
        index.to_be_bytes()
    }

    /// Load last purged log ID from state tree.
    fn load_last_purged(state_tree: &Tree) -> Result<Option<LogId<NodeId>>, RaftError> {
        match state_tree.get(KEY_LAST_PURGED)? {
            Some(bytes) => {
                let log_id: LogId<NodeId> =
                    serde_json::from_slice(&bytes).map_err(|e| RaftError::Storage(e.to_string()))?;
                Ok(Some(log_id))
            }
            None => Ok(None),
        }
    }

    /// Save last purged log ID to state tree.
    fn save_last_purged(&self, log_id: LogId<NodeId>) -> Result<(), RaftError> {
        let bytes =
            serde_json::to_vec(&log_id).map_err(|e| RaftError::Serialization(e.to_string()))?;
        self.state_tree.insert(KEY_LAST_PURGED, bytes)?;
        *self.last_purged.write() = Some(log_id);
        Ok(())
    }

    /// Get the last log entry's LogId.
    fn get_last_log_id(&self) -> Result<Option<LogId<NodeId>>, RaftError> {
        if let Some(result) = self.log_tree.last()? {
            let (_, value) = result;
            let entry: Entry<TypeConfig> =
                serde_json::from_slice(&value).map_err(|e| RaftError::Storage(e.to_string()))?;
            Ok(Some(entry.log_id))
        } else {
            Ok(None)
        }
    }

    /// Serialize an entry to bytes.
    fn serialize_entry(entry: &Entry<TypeConfig>) -> Result<Vec<u8>, RaftError> {
        serde_json::to_vec(entry).map_err(|e| RaftError::Serialization(e.to_string()))
    }

    /// Deserialize an entry from bytes.
    fn deserialize_entry(bytes: &[u8]) -> Result<Entry<TypeConfig>, RaftError> {
        serde_json::from_slice(bytes).map_err(|e| RaftError::Storage(e.to_string()))
    }
}

impl RaftLogReader<TypeConfig> for SledRaftLogStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
        use std::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(&i) => Self::log_key(i),
            Bound::Excluded(&i) => Self::log_key(i.saturating_add(1)),
            Bound::Unbounded => Self::log_key(0),
        };

        let end = match range.end_bound() {
            Bound::Included(&i) => Some(Self::log_key(i.saturating_add(1))),
            Bound::Excluded(&i) => Some(Self::log_key(i)),
            Bound::Unbounded => None,
        };

        let iter = if let Some(end_key) = end {
            self.log_tree.range(start..end_key)
        } else {
            self.log_tree.range(start..)
        };

        let mut entries = Vec::new();
        for result in iter {
            let (_, value) = result.map_err(|e| StorageIOError::read_logs(AnyError::new(&e)))?;
            let entry =
                Self::deserialize_entry(&value).map_err(|e| StorageIOError::read_logs(AnyError::new(&e)))?;
            entries.push(entry);
        }

        Ok(entries)
    }
}

impl RaftLogStorage<TypeConfig> for SledRaftLogStorage {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let last_purged = *self.last_purged.read();
        let last_log_id = self
            .get_last_log_id()
            .map_err(|e| StorageIOError::read_logs(AnyError::new(&e)))?;

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        // Create a new instance sharing the same sled trees
        Self {
            db: self.db.clone(),
            log_tree: self.log_tree.clone(),
            vote_tree: self.vote_tree.clone(),
            state_tree: self.state_tree.clone(),
            last_purged: RwLock::new(*self.last_purged.read()),
        }
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let bytes = serde_json::to_vec(vote).map_err(|e| StorageIOError::write_vote(AnyError::new(&e)))?;
        self.vote_tree
            .insert(b"vote", bytes)
            .map_err(|e| StorageIOError::write_vote(AnyError::new(&e)))?;
        self.vote_tree
            .flush()
            .map_err(|e| StorageIOError::write_vote(AnyError::new(&e)))?;
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        match self
            .vote_tree
            .get(b"vote")
            .map_err(|e| StorageIOError::read_vote(AnyError::new(&e)))?
        {
            Some(bytes) => {
                let vote: Vote<NodeId> =
                    serde_json::from_slice(&bytes).map_err(|e| StorageIOError::read_vote(AnyError::new(&e)))?;
                Ok(Some(vote))
            }
            None => Ok(None),
        }
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfig>,
    ) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
        I::IntoIter: Send,
    {
        for entry in entries {
            let key = Self::log_key(entry.log_id.index);
            let value =
                Self::serialize_entry(&entry).map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;
            self.log_tree
                .insert(key, value)
                .map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;
        }

        self.log_tree
            .flush()
            .map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;

        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        // Remove all entries with index >= log_id.index
        let start_key = Self::log_key(log_id.index);

        let keys_to_remove: Vec<_> = self
            .log_tree
            .range(start_key..)
            .filter_map(|r| r.ok().map(|(k, _)| k))
            .collect();

        for key in keys_to_remove {
            self.log_tree
                .remove(key)
                .map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;
        }

        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        // Remove all entries with index <= log_id.index
        let end_key = Self::log_key(log_id.index.saturating_add(1));

        let keys_to_remove: Vec<_> = self
            .log_tree
            .range(..end_key)
            .filter_map(|r| r.ok().map(|(k, _)| k))
            .collect();

        for key in keys_to_remove {
            self.log_tree
                .remove(key)
                .map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;
        }

        // Update last purged
        self.save_last_purged(log_id)
            .map_err(|e| StorageIOError::write_logs(AnyError::new(&e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClientRequest;
    use openraft::EntryPayload;

    fn create_test_entry(index: u64, term: u64) -> Entry<TypeConfig> {
        Entry {
            log_id: LogId::new(openraft::CommittedLeaderId::new(term, 1), index),
            payload: EntryPayload::Normal(ClientRequest::Noop),
        }
    }

    /// Helper to insert entries directly into the log tree (bypasses append which needs LogFlushed)
    fn insert_entries_directly(storage: &SledRaftLogStorage, entries: &[Entry<TypeConfig>]) {
        for entry in entries {
            let key = SledRaftLogStorage::log_key(entry.log_id.index);
            let value = SledRaftLogStorage::serialize_entry(entry).unwrap();
            storage.log_tree.insert(key, value).unwrap();
        }
        storage.log_tree.flush().unwrap();
    }

    #[tokio::test]
    async fn test_insert_and_read() {
        let db = Arc::new(sled::Config::new().temporary(true).open().unwrap());
        let mut storage = SledRaftLogStorage::open(db).unwrap();

        // Insert entries directly
        let entries = vec![
            create_test_entry(1, 1),
            create_test_entry(2, 1),
            create_test_entry(3, 1),
        ];
        insert_entries_directly(&storage, &entries);

        // Read entries back
        let read_entries = storage.try_get_log_entries(1..4).await.unwrap();
        assert_eq!(read_entries.len(), 3);
        assert_eq!(read_entries[0].log_id.index, 1);
        assert_eq!(read_entries[2].log_id.index, 3);
    }

    #[tokio::test]
    async fn test_truncate() {
        let db = Arc::new(sled::Config::new().temporary(true).open().unwrap());
        let mut storage = SledRaftLogStorage::open(db).unwrap();

        // Insert entries directly
        let entries = vec![
            create_test_entry(1, 1),
            create_test_entry(2, 1),
            create_test_entry(3, 1),
            create_test_entry(4, 1),
        ];
        insert_entries_directly(&storage, &entries);

        // Truncate from index 3
        let log_id = LogId::new(openraft::CommittedLeaderId::new(1, 1), 3);
        storage.truncate(log_id).await.unwrap();

        // Only entries 1-2 should remain
        let read_entries = storage.try_get_log_entries(1..).await.unwrap();
        assert_eq!(read_entries.len(), 2);
    }

    #[tokio::test]
    async fn test_purge() {
        let db = Arc::new(sled::Config::new().temporary(true).open().unwrap());
        let mut storage = SledRaftLogStorage::open(db).unwrap();

        // Insert entries directly
        let entries = vec![
            create_test_entry(1, 1),
            create_test_entry(2, 1),
            create_test_entry(3, 1),
            create_test_entry(4, 1),
        ];
        insert_entries_directly(&storage, &entries);

        // Purge up to index 2
        let log_id = LogId::new(openraft::CommittedLeaderId::new(1, 1), 2);
        storage.purge(log_id).await.unwrap();

        // Only entries 3-4 should remain
        let read_entries = storage.try_get_log_entries(1..).await.unwrap();
        assert_eq!(read_entries.len(), 2);
        assert_eq!(read_entries[0].log_id.index, 3);

        // Check last purged was updated
        let log_state = storage.get_log_state().await.unwrap();
        assert_eq!(log_state.last_purged_log_id, Some(log_id));
    }

    #[tokio::test]
    async fn test_vote_persistence() {
        let db = Arc::new(sled::Config::new().temporary(true).open().unwrap());
        let mut storage = SledRaftLogStorage::open(db.clone()).unwrap();

        // Initially no vote
        assert!(storage.read_vote().await.unwrap().is_none());

        // Save a vote
        let vote = Vote::new(1, 5);
        storage.save_vote(&vote).await.unwrap();

        // Read it back
        let read_vote = storage.read_vote().await.unwrap();
        assert_eq!(read_vote, Some(vote));

        // Verify persistence by reopening
        drop(storage);
        let mut storage2 = SledRaftLogStorage::open(db).unwrap();
        let read_vote2 = storage2.read_vote().await.unwrap();
        assert_eq!(read_vote2, Some(vote));
    }

    #[tokio::test]
    async fn test_log_state() {
        let db = Arc::new(sled::Config::new().temporary(true).open().unwrap());
        let mut storage = SledRaftLogStorage::open(db).unwrap();

        // Initially empty
        let state = storage.get_log_state().await.unwrap();
        assert!(state.last_purged_log_id.is_none());
        assert!(state.last_log_id.is_none());

        // Insert an entry directly
        let entry = create_test_entry(1, 1);
        insert_entries_directly(&storage, &[entry]);

        // Check state
        let state = storage.get_log_state().await.unwrap();
        assert!(state.last_purged_log_id.is_none());
        assert_eq!(state.last_log_id.unwrap().index, 1);
    }
}
