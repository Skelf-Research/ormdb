//! Transaction support for atomic multi-key operations.

use super::{Record, StorageEngine, VersionedKey};
use crate::error::Error;
use sled::transaction::{ConflictableTransactionError, TransactionalTree};
use sled::Transactional;

/// Prefix for latest version pointers in meta tree.
const LATEST_PREFIX: &[u8] = b"latest:";

/// A pending operation in a transaction.
#[derive(Debug)]
enum TransactionOp {
    /// Put a versioned record.
    Put { key: VersionedKey, record: Record },
    /// Delete (soft delete via tombstone).
    Delete { entity_id: [u8; 16] },
}

/// A transaction for atomic multi-key operations.
///
/// Operations are collected and executed atomically on commit.
pub struct Transaction<'a> {
    engine: &'a StorageEngine,
    ops: Vec<TransactionOp>,
}

impl<'a> Transaction<'a> {
    /// Create a new transaction.
    pub(crate) fn new(engine: &'a StorageEngine) -> Self {
        Self {
            engine,
            ops: Vec::new(),
        }
    }

    /// Queue a put operation.
    pub fn put(&mut self, key: VersionedKey, record: Record) -> &mut Self {
        self.ops.push(TransactionOp::Put { key, record });
        self
    }

    /// Queue a delete operation (soft delete via tombstone).
    pub fn delete(&mut self, entity_id: [u8; 16]) -> &mut Self {
        self.ops.push(TransactionOp::Delete { entity_id });
        self
    }

    /// Commit the transaction atomically.
    ///
    /// All operations succeed or none do.
    pub fn commit(self) -> Result<(), Error> {
        if self.ops.is_empty() {
            return Ok(());
        }

        let data_tree = self.engine.data_tree();
        let meta_tree = self.engine.meta_tree();

        // Execute all operations in a sled transaction
        let result: Result<(), sled::transaction::TransactionError<Error>> = (data_tree, meta_tree)
            .transaction(|(data_tx, meta_tx)| {
                for op in &self.ops {
                    match op {
                        TransactionOp::Put { key, record } => {
                            Self::execute_put(data_tx, meta_tx, key, record)?;
                        }
                        TransactionOp::Delete { entity_id } => {
                            Self::execute_delete(data_tx, meta_tx, entity_id)?;
                        }
                    }
                }
                Ok(())
            });

        match result {
            Ok(()) => Ok(()),
            Err(sled::transaction::TransactionError::Abort(e)) => Err(e),
            Err(sled::transaction::TransactionError::Storage(e)) => Err(Error::Storage(e)),
        }
    }

    /// Rollback the transaction (discard all pending operations).
    pub fn rollback(self) {
        // Simply drop self, discarding all pending operations
        drop(self.ops);
    }

    /// Execute a put operation within a transaction.
    fn execute_put(
        data_tx: &TransactionalTree,
        meta_tx: &TransactionalTree,
        key: &VersionedKey,
        record: &Record,
    ) -> Result<(), ConflictableTransactionError<Error>> {
        let key_bytes = key.encode();
        let value_bytes = record
            .to_bytes()
            .map_err(ConflictableTransactionError::Abort)?;

        // Insert the versioned record
        data_tx.insert(&key_bytes, value_bytes)?;

        // Update the latest version pointer
        let latest_key = Self::latest_key(&key.entity_id);
        meta_tx.insert(latest_key, &key.version_ts.to_be_bytes())?;

        Ok(())
    }

    /// Execute a delete operation within a transaction.
    fn execute_delete(
        data_tx: &TransactionalTree,
        meta_tx: &TransactionalTree,
        entity_id: &[u8; 16],
    ) -> Result<(), ConflictableTransactionError<Error>> {
        let key = VersionedKey::now(*entity_id);
        let record = Record::tombstone();

        Self::execute_put(data_tx, meta_tx, &key, &record)
    }

    /// Get the metadata key for the latest version pointer.
    fn latest_key(entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(LATEST_PREFIX.len() + 16);
        key.extend_from_slice(LATEST_PREFIX);
        key.extend_from_slice(entity_id);
        key
    }
}

impl StorageEngine {
    /// Begin a new transaction.
    pub fn transaction(&self) -> Transaction<'_> {
        Transaction::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageConfig;

    fn test_engine() -> StorageEngine {
        StorageEngine::open(StorageConfig::temporary()).unwrap()
    }

    #[test]
    fn test_transaction_commit() {
        let engine = test_engine();
        let id1 = StorageEngine::generate_id();
        let id2 = StorageEngine::generate_id();

        // Insert two records atomically
        let mut tx = engine.transaction();
        tx.put(VersionedKey::new(id1, 100), Record::new(vec![1]));
        tx.put(VersionedKey::new(id2, 100), Record::new(vec![2]));
        tx.commit().unwrap();

        // Both should exist
        assert!(engine.get(&id1, 100).unwrap().is_some());
        assert!(engine.get(&id2, 100).unwrap().is_some());
    }

    #[test]
    fn test_transaction_rollback() {
        let engine = test_engine();
        let id1 = StorageEngine::generate_id();

        // Start a transaction but rollback
        let mut tx = engine.transaction();
        tx.put(VersionedKey::new(id1, 100), Record::new(vec![1]));
        tx.rollback();

        // Record should not exist
        assert!(engine.get(&id1, 100).unwrap().is_none());
    }

    #[test]
    fn test_transaction_multiple_versions() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        // Insert multiple versions of the same entity atomically
        let mut tx = engine.transaction();
        tx.put(VersionedKey::new(entity_id, 100), Record::new(vec![1]));
        tx.put(VersionedKey::new(entity_id, 200), Record::new(vec![2]));
        tx.put(VersionedKey::new(entity_id, 300), Record::new(vec![3]));
        tx.commit().unwrap();

        // All versions should exist
        assert_eq!(engine.get(&entity_id, 100).unwrap().unwrap().data, vec![1]);
        assert_eq!(engine.get(&entity_id, 200).unwrap().unwrap().data, vec![2]);
        assert_eq!(engine.get(&entity_id, 300).unwrap().unwrap().data, vec![3]);

        // Latest should be version 300
        let (version, record) = engine.get_latest(&entity_id).unwrap().unwrap();
        assert_eq!(version, 300);
        assert_eq!(record.data, vec![3]);
    }

    #[test]
    fn test_transaction_delete() {
        let engine = test_engine();
        let entity_id = StorageEngine::generate_id();

        // Insert a record
        engine
            .put(VersionedKey::new(entity_id, 100), Record::new(vec![1]))
            .unwrap();

        // Delete in a transaction
        let mut tx = engine.transaction();
        tx.delete(entity_id);
        tx.commit().unwrap();

        // Latest should be None (tombstone)
        assert!(engine.get_latest(&entity_id).unwrap().is_none());
    }

    #[test]
    fn test_empty_transaction() {
        let engine = test_engine();

        // Empty transaction should succeed
        let tx = engine.transaction();
        tx.commit().unwrap();
    }
}
