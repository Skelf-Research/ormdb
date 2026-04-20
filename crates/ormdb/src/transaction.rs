//! Transaction support for atomic operations.
//!
//! Transactions in ORMDB use optimistic concurrency control (OCC).
//! Multiple transactions can run concurrently without blocking.
//! Conflicts are detected at commit time based on version tracking.
//!
//! # Example
//!
//! ```ignore
//! use ormdb::Database;
//!
//! let db = Database::open_memory().unwrap();
//!
//! // Using with_transaction (auto-commit/rollback)
//! db.with_transaction(|tx| {
//!     tx.insert("User").set("name", "Alice").execute()?;
//!     tx.insert("User").set("name", "Bob").execute()?;
//!     Ok(())
//! }).unwrap();
//!
//! // Manual transaction control
//! let mut tx = db.transaction();
//! tx.insert("User").set("name", "Charlie").execute()?;
//! tx.commit()?;
//! ```

use ormdb_core::query::encode_entity;
use ormdb_core::storage::{Record, StorageEngine, Transaction as CoreTransaction};
use ormdb_proto::Value;

use crate::database::Database;
use crate::error::{Error, Result};

/// A database transaction.
///
/// Transactions collect operations and execute them atomically on commit.
/// Uses optimistic concurrency control - conflicts are detected at commit time.
///
/// # Conflict Handling
///
/// If two transactions modify the same entity, one will succeed and the other
/// will fail with `Error::TransactionConflict` at commit time.
///
/// ```rust,no_run
/// use ormdb::Database;
/// use std::thread;
///
/// let db = Database::open("./data").unwrap();
///
/// // Thread 1 and 2 both try to update the same entity
/// // One will succeed, the other will get TransactionConflict
/// ```
pub struct Transaction<'db> {
    db: &'db Database,
    inner: Option<CoreTransaction<'db>>,
}

impl<'db> Transaction<'db> {
    /// Create a new transaction.
    pub(crate) fn new(db: &'db Database) -> Self {
        Self {
            db,
            inner: Some(db.storage().transaction()),
        }
    }

    /// Get a mutable reference to the inner transaction.
    /// Panics if the transaction has already been committed or rolled back.
    fn inner(&mut self) -> &mut CoreTransaction<'db> {
        self.inner.as_mut().expect("transaction already consumed")
    }

    /// Insert a new entity within this transaction.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut tx = db.transaction();
    /// let id = tx.insert("User").set("name", "Alice").execute()?;
    /// tx.commit()?;
    /// ```
    pub fn insert(&mut self, entity: &str) -> TransactionalInsert<'_, 'db> {
        TransactionalInsert::new(self, entity)
    }

    /// Update an entity within this transaction.
    pub fn update(&mut self, entity: &str, id: [u8; 16]) -> TransactionalUpdate<'_, 'db> {
        TransactionalUpdate::new(self, entity, id)
    }

    /// Delete an entity within this transaction.
    ///
    /// The delete is recorded but not executed until commit.
    pub fn delete(&mut self, entity: &str, id: [u8; 16]) {
        self.inner().delete_typed(entity, id);
    }

    /// Read an entity within the transaction.
    ///
    /// This reads from uncommitted writes first, then from storage.
    /// Versions are tracked for conflict detection.
    pub fn read(&mut self, id: &[u8; 16]) -> Result<Option<Vec<(String, Value)>>> {
        match self.inner().read(id)? {
            Some(record) => {
                let fields = ormdb_core::query::decode_entity(&record.data)
                    .map_err(|e| Error::Serialization(e.to_string()))?;
                Ok(Some(fields))
            }
            None => Ok(None),
        }
    }

    /// Check if an entity exists within the transaction.
    ///
    /// This sees uncommitted writes and tracks versions.
    pub fn exists(&mut self, id: &[u8; 16]) -> Result<bool> {
        self.inner().exists(id).map_err(Into::into)
    }

    /// Commit the transaction.
    ///
    /// All operations are executed atomically. If any operation fails
    /// or there's a version conflict, all changes are rolled back.
    ///
    /// # Errors
    ///
    /// - `Error::TransactionConflict` if another transaction modified an entity
    ///   we read or wrote
    /// - `Error::ConstraintViolation` if any constraint is violated
    pub fn commit(mut self) -> Result<()> {
        let inner = self.inner.take().expect("transaction already consumed");
        inner.commit().map_err(Into::into)
    }

    /// Rollback the transaction, discarding all pending operations.
    pub fn rollback(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }

    /// Get a reference to the inner transaction for internal operations.
    pub(crate) fn inner_mut(&mut self) -> &mut CoreTransaction<'db> {
        self.inner()
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        // If not committed or rolled back, rollback automatically
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }
}

/// Builder for inserting within a transaction.
pub struct TransactionalInsert<'tx, 'db> {
    tx: &'tx mut Transaction<'db>,
    entity_type: String,
    fields: Vec<(String, Value)>,
}

impl<'tx, 'db> TransactionalInsert<'tx, 'db> {
    fn new(tx: &'tx mut Transaction<'db>, entity_type: &str) -> Self {
        Self {
            tx,
            entity_type: entity_type.to_string(),
            fields: Vec::new(),
        }
    }

    /// Set a field value.
    pub fn set(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.fields.push((field.to_string(), value.into()));
        self
    }

    /// Execute the insert within the transaction.
    ///
    /// Returns the generated entity ID. The actual insert happens at commit time.
    pub fn execute(self) -> Result<[u8; 16]> {
        let id = StorageEngine::generate_id();

        // Add ID to fields
        let mut all_fields = vec![("id".to_string(), Value::Uuid(id))];
        all_fields.extend(self.fields);

        // Encode the entity
        let data = encode_entity(&all_fields)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Add to transaction
        self.tx.inner_mut().insert(&self.entity_type, id, Record::new(data));

        Ok(id)
    }
}

/// Builder for updating within a transaction.
pub struct TransactionalUpdate<'tx, 'db> {
    tx: &'tx mut Transaction<'db>,
    entity_type: String,
    entity_id: [u8; 16],
    fields: Vec<(String, Value)>,
}

impl<'tx, 'db> TransactionalUpdate<'tx, 'db> {
    fn new(tx: &'tx mut Transaction<'db>, entity_type: &str, entity_id: [u8; 16]) -> Self {
        Self {
            tx,
            entity_type: entity_type.to_string(),
            entity_id,
            fields: Vec::new(),
        }
    }

    /// Set a field value.
    pub fn set(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.fields.push((field.to_string(), value.into()));
        self
    }

    /// Execute the update within the transaction.
    pub fn execute(self) -> Result<bool> {
        // Read current entity (this tracks the version for conflict detection)
        let current = self.tx.read(&self.entity_id)?;

        let mut current_fields = match current {
            Some(fields) => fields,
            None => return Ok(false), // Entity not found
        };

        // Update fields
        for (field, value) in self.fields {
            if let Some(existing) = current_fields.iter_mut().find(|(f, _)| *f == field) {
                existing.1 = value;
            } else {
                current_fields.push((field, value));
            }
        }

        // Encode and add to transaction
        let data = encode_entity(&current_fields)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        self.tx
            .inner_mut()
            .update(&self.entity_type, self.entity_id, Record::new(data));

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_insert() {
        let db = Database::open_memory().unwrap();

        let mut tx = db.transaction();
        let id = tx.insert("TestEntity").set("name", "test").execute().unwrap();
        tx.commit().unwrap();

        assert_ne!(id, [0; 16]);
    }

    #[test]
    fn test_transaction_rollback() {
        let db = Database::open_memory().unwrap();

        let mut tx = db.transaction();
        tx.insert("TestEntity").set("name", "test").execute().unwrap();
        tx.rollback();

        // Entity should not exist after rollback
    }

    #[test]
    fn test_with_transaction() {
        let db = Database::open_memory().unwrap();

        let result = db.with_transaction(|tx| {
            tx.insert("TestEntity").set("name", "Alice").execute()?;
            tx.insert("TestEntity").set("name", "Bob").execute()?;
            Ok("success")
        });

        assert_eq!(result.unwrap(), "success");
    }
}
