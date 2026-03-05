//! Mutation builders for Insert, Update, and Delete operations.
//!
//! # Example
//!
//! ```rust,no_run
//! use ormdb::Database;
//!
//! let db = Database::open_memory().unwrap();
//!
//! // Insert
//! let user_id = db.insert("User")
//!     .set("name", "Alice")
//!     .set("email", "alice@example.com")
//!     .execute()
//!     .unwrap();
//!
//! // Update
//! db.update("User", user_id)
//!     .set("name", "Alice Smith")
//!     .execute()
//!     .unwrap();
//!
//! // Delete
//! db.delete("User", user_id)
//!     .execute()
//!     .unwrap();
//! ```

use ormdb_core::query::encode_entity;
use ormdb_core::storage::{Record, StorageEngine, VersionedKey};
use ormdb_proto::Value;

use crate::database::Database;
use crate::error::{Error, Result};

/// Builder for insert operations.
pub struct Insert<'db> {
    db: &'db Database,
    entity_type: String,
    fields: Vec<(String, Value)>,
}

impl<'db> Insert<'db> {
    /// Create a new insert builder.
    pub(crate) fn new(db: &'db Database, entity_type: &str) -> Self {
        Self {
            db,
            entity_type: entity_type.to_string(),
            fields: Vec::new(),
        }
    }

    /// Set a field value.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.insert("User")
    ///     .set("name", "Alice")
    ///     .set("age", 30)
    ///     .set("active", true)
    /// ```
    pub fn set(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.fields.push((field.to_string(), value.into()));
        self
    }

    /// Set multiple fields at once.
    pub fn set_many(mut self, fields: impl IntoIterator<Item = (&'db str, Value)>) -> Self {
        for (field, value) in fields {
            self.fields.push((field.to_string(), value));
        }
        self
    }

    /// Execute the insert and return the new entity ID.
    ///
    /// The ID is automatically generated as a UUID.
    pub fn execute(self) -> Result<[u8; 16]> {
        let id = StorageEngine::generate_id();

        // Add ID to fields
        let mut all_fields = vec![("id".to_string(), Value::Uuid(id))];
        all_fields.extend(self.fields);

        // Encode the entity
        let data = encode_entity(&all_fields)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Create versioned key with current timestamp
        let key = VersionedKey::now(id);

        // Insert into storage
        self.db
            .storage()
            .put_typed(&self.entity_type, key, Record::new(data))
            .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(id)
    }

    /// Execute the insert with a specific ID.
    ///
    /// Use this when you need to control the entity ID (e.g., for migrations).
    pub fn execute_with_id(mut self, id: [u8; 16]) -> Result<[u8; 16]> {
        // Add ID to fields
        self.fields.insert(0, ("id".to_string(), Value::Uuid(id)));

        // Encode the entity
        let data = encode_entity(&self.fields)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Create versioned key
        let key = VersionedKey::now(id);

        // Insert into storage
        self.db
            .storage()
            .put_typed(&self.entity_type, key, Record::new(data))
            .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(id)
    }
}

/// Builder for update operations.
pub struct Update<'db> {
    db: &'db Database,
    entity_type: String,
    entity_id: [u8; 16],
    fields: Vec<(String, Value)>,
}

impl<'db> Update<'db> {
    /// Create a new update builder.
    pub(crate) fn new(db: &'db Database, entity_type: &str, entity_id: [u8; 16]) -> Self {
        Self {
            db,
            entity_type: entity_type.to_string(),
            entity_id,
            fields: Vec::new(),
        }
    }

    /// Set a field value.
    ///
    /// Only the specified fields will be updated; other fields retain their values.
    pub fn set(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.fields.push((field.to_string(), value.into()));
        self
    }

    /// Set multiple fields at once.
    pub fn set_many(mut self, fields: impl IntoIterator<Item = (&'db str, Value)>) -> Self {
        for (field, value) in fields {
            self.fields.push((field.to_string(), value));
        }
        self
    }

    /// Execute the update.
    ///
    /// Returns `true` if the entity was found and updated.
    pub fn execute(self) -> Result<bool> {
        // Get the current record
        let current = self
            .db
            .storage()
            .get_latest(&self.entity_id)
            .map_err(|e| Error::Storage(e.to_string()))?;

        let (_, current_record) = match current {
            Some(r) => r,
            None => return Ok(false), // Entity not found
        };

        // Decode current fields
        let mut current_fields: Vec<(String, Value)> =
            ormdb_core::query::decode_entity(&current_record.data)
                .map_err(|e| Error::Serialization(e.to_string()))?;

        // Update fields
        for (field, value) in self.fields {
            if let Some(existing) = current_fields.iter_mut().find(|(f, _)| *f == field) {
                existing.1 = value;
            } else {
                current_fields.push((field, value));
            }
        }

        // Encode updated entity
        let data = encode_entity(&current_fields)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Write new version
        let key = VersionedKey::now(self.entity_id);
        self.db
            .storage()
            .put_typed(&self.entity_type, key, Record::new(data))
            .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(true)
    }
}

/// Builder for delete operations.
pub struct Delete<'db> {
    db: &'db Database,
    entity_type: String,
    entity_id: [u8; 16],
}

impl<'db> Delete<'db> {
    /// Create a new delete builder.
    pub(crate) fn new(db: &'db Database, entity_type: &str, entity_id: [u8; 16]) -> Self {
        Self {
            db,
            entity_type: entity_type.to_string(),
            entity_id,
        }
    }

    /// Execute the delete.
    ///
    /// Returns `true` if the entity was found and deleted.
    pub fn execute(self) -> Result<bool> {
        // Check if entity exists
        let exists = self
            .db
            .storage()
            .get_latest(&self.entity_id)
            .map_err(|e| Error::Storage(e.to_string()))?
            .is_some();

        if !exists {
            return Ok(false);
        }

        // Write tombstone (soft delete)
        self.db
            .storage()
            .delete_typed(&self.entity_type, &self.entity_id)
            .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_generates_id() {
        let db = Database::open_memory().unwrap();
        let id = db.insert("TestEntity").set("name", "test").execute().unwrap();
        assert_ne!(id, [0; 16]);
    }

    #[test]
    fn test_insert_with_specific_id() {
        let db = Database::open_memory().unwrap();
        let specific_id = [1u8; 16];
        let id = db
            .insert("TestEntity")
            .set("name", "test")
            .execute_with_id(specific_id)
            .unwrap();
        assert_eq!(id, specific_id);
    }
}
