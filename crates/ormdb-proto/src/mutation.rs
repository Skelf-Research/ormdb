//! Mutation IR types for write operations.

use crate::value::Value;
use rkyv::{Archive, Deserialize, Serialize};

/// A mutation operation (insert, update, or delete).
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum Mutation {
    /// Insert a new entity.
    Insert {
        /// Entity type to insert into.
        entity: String,
        /// Field values for the new entity.
        data: Vec<FieldValue>,
    },
    /// Update an existing entity.
    Update {
        /// Entity type to update.
        entity: String,
        /// ID of the entity to update.
        id: [u8; 16],
        /// Field values to update.
        data: Vec<FieldValue>,
    },
    /// Delete an entity.
    Delete {
        /// Entity type to delete from.
        entity: String,
        /// ID of the entity to delete.
        id: [u8; 16],
    },
    /// Upsert (insert or update) an entity.
    Upsert {
        /// Entity type to upsert.
        entity: String,
        /// ID of the entity (if updating).
        id: Option<[u8; 16]>,
        /// Field values for the entity.
        data: Vec<FieldValue>,
    },
}

/// A field name and value pair.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct FieldValue {
    /// Field name.
    pub field: String,
    /// Field value.
    pub value: Value,
}

impl FieldValue {
    /// Create a new field-value pair.
    pub fn new(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            field: field.into(),
            value: value.into(),
        }
    }
}

impl Mutation {
    /// Create an insert mutation.
    pub fn insert(entity: impl Into<String>, data: Vec<FieldValue>) -> Self {
        Mutation::Insert {
            entity: entity.into(),
            data,
        }
    }

    /// Create an update mutation.
    pub fn update(entity: impl Into<String>, id: [u8; 16], data: Vec<FieldValue>) -> Self {
        Mutation::Update {
            entity: entity.into(),
            id,
            data,
        }
    }

    /// Create a delete mutation.
    pub fn delete(entity: impl Into<String>, id: [u8; 16]) -> Self {
        Mutation::Delete {
            entity: entity.into(),
            id,
        }
    }

    /// Create an upsert mutation.
    pub fn upsert(entity: impl Into<String>, id: Option<[u8; 16]>, data: Vec<FieldValue>) -> Self {
        Mutation::Upsert {
            entity: entity.into(),
            id,
            data,
        }
    }

    /// Get the entity type this mutation operates on.
    pub fn entity(&self) -> &str {
        match self {
            Mutation::Insert { entity, .. } => entity,
            Mutation::Update { entity, .. } => entity,
            Mutation::Delete { entity, .. } => entity,
            Mutation::Upsert { entity, .. } => entity,
        }
    }
}

/// A batch of mutations to execute atomically.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct MutationBatch {
    /// Mutations to execute in order.
    pub mutations: Vec<Mutation>,
}

impl MutationBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self { mutations: vec![] }
    }

    /// Create a batch from mutations.
    pub fn from_mutations(mutations: Vec<Mutation>) -> Self {
        Self { mutations }
    }

    /// Add a mutation to the batch.
    pub fn push(&mut self, mutation: Mutation) {
        self.mutations.push(mutation);
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    /// Get the number of mutations in the batch.
    pub fn len(&self) -> usize {
        self.mutations.len()
    }
}

impl Default for MutationBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<Mutation> for MutationBatch {
    fn from_iter<T: IntoIterator<Item = Mutation>>(iter: T) -> Self {
        Self {
            mutations: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_mutation() {
        let mutation = Mutation::insert(
            "User",
            vec![
                FieldValue::new("name", "Alice"),
                FieldValue::new("email", "alice@example.com"),
                FieldValue::new("active", true),
            ],
        );

        if let Mutation::Insert { entity, data } = &mutation {
            assert_eq!(entity, "User");
            assert_eq!(data.len(), 3);
            assert_eq!(data[0].field, "name");
        } else {
            panic!("Expected Insert mutation");
        }
    }

    #[test]
    fn test_update_mutation() {
        let id = [1u8; 16];
        let mutation = Mutation::update(
            "User",
            id,
            vec![FieldValue::new("name", "Bob"), FieldValue::new("age", 30i32)],
        );

        if let Mutation::Update {
            entity,
            id: update_id,
            data,
        } = &mutation
        {
            assert_eq!(entity, "User");
            assert_eq!(*update_id, id);
            assert_eq!(data.len(), 2);
        } else {
            panic!("Expected Update mutation");
        }
    }

    #[test]
    fn test_delete_mutation() {
        let id = [2u8; 16];
        let mutation = Mutation::delete("User", id);

        if let Mutation::Delete {
            entity,
            id: delete_id,
        } = &mutation
        {
            assert_eq!(entity, "User");
            assert_eq!(*delete_id, id);
        } else {
            panic!("Expected Delete mutation");
        }
    }

    #[test]
    fn test_mutation_batch() {
        let mut batch = MutationBatch::new();
        assert!(batch.is_empty());

        batch.push(Mutation::insert(
            "User",
            vec![FieldValue::new("name", "Test")],
        ));
        batch.push(Mutation::delete("Session", [0u8; 16]));

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_mutation_serialization_roundtrip() {
        let mutations = vec![
            Mutation::insert(
                "Post",
                vec![
                    FieldValue::new("title", "Hello World"),
                    FieldValue::new("published", false),
                ],
            ),
            Mutation::update(
                "Post",
                [1u8; 16],
                vec![FieldValue::new("published", true)],
            ),
            Mutation::delete("Comment", [2u8; 16]),
        ];

        let batch = MutationBatch::from_mutations(mutations);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&batch).unwrap();
        let archived = rkyv::access::<ArchivedMutationBatch, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: MutationBatch =
            rkyv::deserialize::<MutationBatch, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(batch, deserialized);
    }
}
