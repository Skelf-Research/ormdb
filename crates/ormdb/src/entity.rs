//! Entity result types for query results.
//!
//! The `Entity` type represents a single entity (row) from a query result.

use std::collections::HashMap;

use ormdb_proto::{EntityBlock, Value};

/// A single entity from query results.
///
/// Provides convenient access to field values.
#[derive(Debug, Clone)]
pub struct Entity {
    /// The entity type name.
    pub entity_type: String,
    /// The entity ID (UUID).
    id: [u8; 16],
    /// Field name -> value mapping.
    fields: HashMap<String, Value>,
    /// Related entities (relation name -> entities).
    relations: HashMap<String, Vec<Entity>>,
}

impl Entity {
    /// Create an entity from an EntityBlock at a specific index.
    pub(crate) fn from_block_index(
        block: &EntityBlock,
        index: usize,
        _all_blocks: &[EntityBlock],
    ) -> Self {
        let id = block.ids.get(index).copied().unwrap_or([0; 16]);

        let mut fields = HashMap::new();
        for col in &block.columns {
            if let Some(value) = col.values.get(index) {
                fields.insert(col.name.clone(), value.clone());
            }
        }

        // TODO: Handle relations by looking at edge blocks
        // For now, we just return the entity without relations
        let relations = HashMap::new();

        Self {
            entity_type: block.entity.clone(),
            id,
            fields,
            relations,
        }
    }

    /// Get the entity ID as bytes.
    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    /// Get the entity ID as a hex string.
    pub fn id_hex(&self) -> String {
        hex::encode(self.id)
    }

    /// Get a field value by name.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Get a string field value.
    ///
    /// Returns `None` if the field doesn't exist or is not a string.
    pub fn get_string(&self, field: &str) -> Option<&str> {
        match self.fields.get(field)? {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get an i32 field value.
    pub fn get_i32(&self, field: &str) -> Option<i32> {
        match self.fields.get(field)? {
            Value::Int32(n) => Some(*n),
            _ => None,
        }
    }

    /// Get an i64 field value.
    pub fn get_i64(&self, field: &str) -> Option<i64> {
        match self.fields.get(field)? {
            Value::Int64(n) => Some(*n),
            _ => None,
        }
    }

    /// Get an f64 field value.
    pub fn get_f64(&self, field: &str) -> Option<f64> {
        match self.fields.get(field)? {
            Value::Float64(n) => Some(*n),
            _ => None,
        }
    }

    /// Get a boolean field value.
    pub fn get_bool(&self, field: &str) -> Option<bool> {
        match self.fields.get(field)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get a UUID field value as bytes.
    pub fn get_uuid(&self, field: &str) -> Option<[u8; 16]> {
        match self.fields.get(field)? {
            Value::Uuid(id) => Some(*id),
            _ => None,
        }
    }

    /// Get related entities by relation name.
    ///
    /// Returns an empty slice if the relation doesn't exist or wasn't included.
    pub fn relation(&self, name: &str) -> &[Entity] {
        self.relations.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if a field exists and is not null.
    pub fn has(&self, field: &str) -> bool {
        matches!(self.fields.get(field), Some(v) if !matches!(v, Value::Null))
    }

    /// Get all field names.
    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.fields.keys().map(|s| s.as_str())
    }

    /// Get all field values as an iterator.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.fields.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.entity_type, self.id_hex())
    }
}
