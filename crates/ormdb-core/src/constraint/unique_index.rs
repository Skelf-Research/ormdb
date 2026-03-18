//! Secondary index for enforcing unique constraints.
//!
//! The UniqueIndex maintains a separate sled tree that maps unique field values
//! to entity IDs, enabling efficient duplicate detection.

use sled::Tree;

use crate::error::{ConstraintError, Error};

/// Tree name for unique constraint index.
pub const UNIQUE_INDEX_TREE: &str = "index:unique";

/// Secondary index for enforcing unique constraints.
///
/// Key format: `entity:constraint_name:field_value1:field_value2...` -> `entity_id`
pub struct UniqueIndex {
    tree: Tree,
}

impl UniqueIndex {
    /// Create a new unique index backed by the given tree.
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    /// Open or create the unique index from a sled database.
    pub fn open(db: &sled::Db) -> Result<Self, Error> {
        let tree = db.open_tree(UNIQUE_INDEX_TREE)?;
        Ok(Self { tree })
    }

    /// Build the index key for a unique constraint.
    fn build_key(entity: &str, constraint: &str, values: &[&str]) -> Vec<u8> {
        // Format: entity\0constraint\0value1\0value2\0...
        let mut key = Vec::new();
        key.extend_from_slice(entity.as_bytes());
        key.push(0);
        key.extend_from_slice(constraint.as_bytes());
        for value in values {
            key.push(0);
            key.extend_from_slice(value.as_bytes());
        }
        key
    }

    /// Insert a unique index entry.
    ///
    /// Returns an error if the value already exists for a different entity.
    pub fn insert(
        &self,
        entity: &str,
        constraint: &str,
        fields: &[String],
        values: &[&str],
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let key = Self::build_key(entity, constraint, values);

        // Check if key already exists
        if let Some(existing) = self.tree.get(&key)? {
            let mut existing_id = [0u8; 16];
            if existing.len() == 16 {
                existing_id.copy_from_slice(&existing);
                if existing_id != entity_id {
                    // Different entity has this value - constraint violation
                    return Err(Error::ConstraintViolation(ConstraintError::UniqueViolation {
                        constraint: constraint.to_string(),
                        entity: entity.to_string(),
                        fields: fields.to_vec(),
                        value: values.join(", "),
                    }));
                }
            }
            // Same entity - this is an update, allow it
        }

        // Insert the mapping
        self.tree.insert(key, &entity_id)?;
        Ok(())
    }

    /// Remove a unique index entry.
    pub fn remove(&self, entity: &str, constraint: &str, values: &[&str]) -> Result<(), Error> {
        let key = Self::build_key(entity, constraint, values);
        self.tree.remove(key)?;
        Ok(())
    }

    /// Look up the entity ID for a unique value.
    pub fn lookup(
        &self,
        entity: &str,
        constraint: &str,
        values: &[&str],
    ) -> Result<Option<[u8; 16]>, Error> {
        let key = Self::build_key(entity, constraint, values);

        match self.tree.get(&key)? {
            Some(bytes) if bytes.len() == 16 => {
                let mut id = [0u8; 16];
                id.copy_from_slice(&bytes);
                Ok(Some(id))
            }
            _ => Ok(None),
        }
    }

    /// Check if a unique value is available (doesn't exist or belongs to exclude_id).
    pub fn check_unique(
        &self,
        entity: &str,
        constraint: &str,
        values: &[&str],
        exclude_id: Option<[u8; 16]>,
    ) -> Result<bool, Error> {
        match self.lookup(entity, constraint, values)? {
            Some(existing_id) => {
                // Check if this is the same entity (update case)
                match exclude_id {
                    Some(id) if id == existing_id => Ok(true), // Same entity, allowed
                    _ => Ok(false),                             // Different entity, not allowed
                }
            }
            None => Ok(true), // Value doesn't exist, allowed
        }
    }

    /// Validate and insert a unique constraint.
    ///
    /// This is a convenience method that checks uniqueness and inserts atomically.
    pub fn validate_and_insert(
        &self,
        entity: &str,
        constraint: &str,
        fields: &[String],
        values: &[&str],
        entity_id: [u8; 16],
        exclude_id: Option<[u8; 16]>,
    ) -> Result<(), Error> {
        if !self.check_unique(entity, constraint, values, exclude_id)? {
            return Err(Error::ConstraintViolation(ConstraintError::UniqueViolation {
                constraint: constraint.to_string(),
                entity: entity.to_string(),
                fields: fields.to_vec(),
                value: values.join(", "),
            }));
        }
        self.insert(entity, constraint, fields, values, entity_id)
    }

    /// Flush the index to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.tree.flush()?;
        Ok(())
    }

    /// Get the number of entries in the index.
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> UniqueIndex {
        let db = sled::Config::new().temporary(true).open().unwrap();
        UniqueIndex::open(&db).unwrap()
    }

    #[test]
    fn test_insert_and_lookup() {
        let index = test_index();
        let entity_id = [1u8; 16];

        index
            .insert(
                "User",
                "user_email_unique",
                &["email".to_string()],
                &["test@example.com"],
                entity_id,
            )
            .unwrap();

        let result = index
            .lookup("User", "user_email_unique", &["test@example.com"])
            .unwrap();
        assert_eq!(result, Some(entity_id));
    }

    #[test]
    fn test_lookup_not_found() {
        let index = test_index();

        let result = index
            .lookup("User", "user_email_unique", &["nonexistent@example.com"])
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_unique_violation() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        // Insert first entry
        index
            .insert(
                "User",
                "user_email_unique",
                &["email".to_string()],
                &["duplicate@example.com"],
                id1,
            )
            .unwrap();

        // Try to insert same value for different entity
        let result = index.insert(
            "User",
            "user_email_unique",
            &["email".to_string()],
            &["duplicate@example.com"],
            id2,
        );

        assert!(result.is_err());
        if let Err(Error::ConstraintViolation(ConstraintError::UniqueViolation {
            constraint,
            value,
            ..
        })) = result
        {
            assert_eq!(constraint, "user_email_unique");
            assert_eq!(value, "duplicate@example.com");
        } else {
            panic!("Expected UniqueViolation error");
        }
    }

    #[test]
    fn test_update_same_entity() {
        let index = test_index();
        let entity_id = [1u8; 16];

        // Insert initial entry
        index
            .insert(
                "User",
                "user_email_unique",
                &["email".to_string()],
                &["test@example.com"],
                entity_id,
            )
            .unwrap();

        // Update same entity with same value should succeed
        let result = index.insert(
            "User",
            "user_email_unique",
            &["email".to_string()],
            &["test@example.com"],
            entity_id,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove() {
        let index = test_index();
        let entity_id = [1u8; 16];

        index
            .insert(
                "User",
                "user_email_unique",
                &["email".to_string()],
                &["test@example.com"],
                entity_id,
            )
            .unwrap();

        // Remove the entry
        index
            .remove("User", "user_email_unique", &["test@example.com"])
            .unwrap();

        // Should no longer exist
        let result = index
            .lookup("User", "user_email_unique", &["test@example.com"])
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_check_unique_available() {
        let index = test_index();

        // Value doesn't exist - should be available
        let result = index
            .check_unique("User", "user_email_unique", &["new@example.com"], None)
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_check_unique_taken() {
        let index = test_index();
        let id1 = [1u8; 16];

        index
            .insert(
                "User",
                "user_email_unique",
                &["email".to_string()],
                &["taken@example.com"],
                id1,
            )
            .unwrap();

        // Without exclude_id - should be taken
        let result = index
            .check_unique("User", "user_email_unique", &["taken@example.com"], None)
            .unwrap();
        assert!(!result);

        // With different exclude_id - should still be taken
        let id2 = [2u8; 16];
        let result = index
            .check_unique(
                "User",
                "user_email_unique",
                &["taken@example.com"],
                Some(id2),
            )
            .unwrap();
        assert!(!result);

        // With same exclude_id - should be available (update case)
        let result = index
            .check_unique(
                "User",
                "user_email_unique",
                &["taken@example.com"],
                Some(id1),
            )
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_composite_unique() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        // Insert composite unique: (org_id, email)
        index
            .insert(
                "Member",
                "member_org_email_unique",
                &["org_id".to_string(), "email".to_string()],
                &["org1", "user@example.com"],
                id1,
            )
            .unwrap();

        // Same email, different org should work
        index
            .insert(
                "Member",
                "member_org_email_unique",
                &["org_id".to_string(), "email".to_string()],
                &["org2", "user@example.com"],
                id2,
            )
            .unwrap();

        // Same org and email should fail
        let id3 = [3u8; 16];
        let result = index.insert(
            "Member",
            "member_org_email_unique",
            &["org_id".to_string(), "email".to_string()],
            &["org1", "user@example.com"],
            id3,
        );
        assert!(result.is_err());
    }
}
