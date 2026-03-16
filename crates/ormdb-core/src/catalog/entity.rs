//! Entity definitions.

use super::field::FieldDef;
use rkyv::{Archive, Deserialize, Serialize};

/// An entity definition (table schema).
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct EntityDef {
    /// Entity name (unique within schema).
    pub name: String,
    /// Name of the primary identity field.
    pub identity_field: String,
    /// Field definitions.
    pub fields: Vec<FieldDef>,
    /// Lifecycle rules.
    pub lifecycle: LifecycleRules,
}

/// Lifecycle rules for an entity.
#[derive(Debug, Clone, PartialEq, Default, Archive, Serialize, Deserialize)]
pub struct LifecycleRules {
    /// Enable soft delete (deleted records kept with tombstone).
    pub soft_delete: bool,
    /// Default ordering for queries without explicit order.
    pub default_order: Option<Vec<OrderBy>>,
}

/// Order specification for default ordering.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct OrderBy {
    /// Field name to order by.
    pub field: String,
    /// Sort direction.
    pub direction: OrderDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum OrderDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

impl EntityDef {
    /// Create a new entity definition.
    pub fn new(name: impl Into<String>, identity_field: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            identity_field: identity_field.into(),
            fields: Vec::new(),
            lifecycle: LifecycleRules::default(),
        }
    }

    /// Add a field to the entity.
    pub fn with_field(mut self, field: FieldDef) -> Self {
        self.fields.push(field);
        self
    }

    /// Add multiple fields.
    pub fn with_fields(mut self, fields: impl IntoIterator<Item = FieldDef>) -> Self {
        self.fields.extend(fields);
        self
    }

    /// Set lifecycle rules.
    pub fn with_lifecycle(mut self, lifecycle: LifecycleRules) -> Self {
        self.lifecycle = lifecycle;
        self
    }

    /// Enable soft delete.
    pub fn with_soft_delete(mut self) -> Self {
        self.lifecycle.soft_delete = true;
        self
    }

    /// Get a field by name.
    pub fn get_field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get the identity field definition.
    pub fn get_identity_field(&self) -> Option<&FieldDef> {
        self.get_field(&self.identity_field)
    }

    /// Get all indexed fields.
    pub fn indexed_fields(&self) -> impl Iterator<Item = &FieldDef> {
        self.fields.iter().filter(|f| f.indexed)
    }

    /// Check if this entity has soft delete enabled.
    pub fn has_soft_delete(&self) -> bool {
        self.lifecycle.soft_delete
    }
}

impl OrderBy {
    /// Create ascending order.
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: OrderDirection::Asc,
        }
    }

    /// Create descending order.
    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: OrderDirection::Desc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{FieldType, ScalarType};

    #[test]
    fn test_entity_builder() {
        let entity = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
            .with_field(FieldDef::optional(
                "email",
                FieldType::scalar(ScalarType::String),
            ))
            .with_soft_delete();

        assert_eq!(entity.name, "User");
        assert_eq!(entity.identity_field, "id");
        assert_eq!(entity.fields.len(), 3);
        assert!(entity.has_soft_delete());
    }

    #[test]
    fn test_get_field() {
        let entity = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)));

        assert!(entity.get_field("id").is_some());
        assert!(entity.get_field("name").is_some());
        assert!(entity.get_field("nonexistent").is_none());
        assert!(entity.get_identity_field().is_some());
    }
}
