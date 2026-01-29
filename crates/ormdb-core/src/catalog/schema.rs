//! Schema bundle - versioned snapshot of the entire schema.

use super::{ConstraintDef, EntityDef, RelationDef};
use crate::error::Error;
use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

/// A versioned snapshot of the entire schema.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct SchemaBundle {
    /// Schema version (monotonically increasing).
    pub version: u64,
    /// Creation timestamp (microseconds since Unix epoch).
    pub created_at: u64,
    /// Entity definitions keyed by name.
    pub entities: HashMap<String, EntityDef>,
    /// Relation definitions keyed by name.
    pub relations: HashMap<String, RelationDef>,
    /// Constraint definitions.
    pub constraints: Vec<ConstraintDef>,
}

impl SchemaBundle {
    /// Create an empty schema bundle.
    pub fn new(version: u64) -> Self {
        Self {
            version,
            created_at: crate::storage::key::current_timestamp(),
            entities: HashMap::new(),
            relations: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    /// Add an entity to the schema.
    pub fn with_entity(mut self, entity: EntityDef) -> Self {
        self.entities.insert(entity.name.clone(), entity);
        self
    }

    /// Add a relation to the schema.
    pub fn with_relation(mut self, relation: RelationDef) -> Self {
        self.relations.insert(relation.name.clone(), relation);
        self
    }

    /// Add a constraint to the schema.
    pub fn with_constraint(mut self, constraint: ConstraintDef) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Get an entity by name.
    pub fn get_entity(&self, name: &str) -> Option<&EntityDef> {
        self.entities.get(name)
    }

    /// Get a relation by name.
    pub fn get_relation(&self, name: &str) -> Option<&RelationDef> {
        self.relations.get(name)
    }

    /// Get all relations for an entity (as source).
    pub fn relations_from(&self, entity: &str) -> Vec<&RelationDef> {
        self.relations
            .values()
            .filter(|r| r.from_entity == entity)
            .collect()
    }

    /// Get all relations to an entity (as target).
    pub fn relations_to(&self, entity: &str) -> Vec<&RelationDef> {
        self.relations
            .values()
            .filter(|r| r.to_entity == entity)
            .collect()
    }

    /// Get all constraints for an entity.
    pub fn constraints_for(&self, entity: &str) -> Vec<&ConstraintDef> {
        self.constraints
            .iter()
            .filter(|c| c.entity() == entity)
            .collect()
    }

    /// List all entity names.
    pub fn entity_names(&self) -> Vec<&str> {
        self.entities.keys().map(|s| s.as_str()).collect()
    }

    /// Serialize the schema bundle to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(|e| Error::Serialization(e.to_string()))
    }

    /// Deserialize a schema bundle from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| Error::Deserialization(e.to_string()))
    }
}

impl Default for SchemaBundle {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{FieldDef, FieldType, ScalarType};

    fn sample_schema() -> SchemaBundle {
        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
            .with_field(FieldDef::new("email", FieldType::scalar(ScalarType::String)));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::scalar(ScalarType::String)))
            .with_field(FieldDef::new(
                "author_id",
                FieldType::scalar(ScalarType::Uuid),
            ));

        let relation = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");

        let unique = ConstraintDef::unique("user_email_unique", "User", "email");
        let fk = ConstraintDef::foreign_key("post_author_fk", "Post", "author_id", "User", "id");

        SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post)
            .with_relation(relation)
            .with_constraint(unique)
            .with_constraint(fk)
    }

    #[test]
    fn test_schema_bundle_builder() {
        let schema = sample_schema();

        assert_eq!(schema.version, 1);
        assert_eq!(schema.entities.len(), 2);
        assert_eq!(schema.relations.len(), 1);
        assert_eq!(schema.constraints.len(), 2);
    }

    #[test]
    fn test_get_entity() {
        let schema = sample_schema();

        assert!(schema.get_entity("User").is_some());
        assert!(schema.get_entity("Post").is_some());
        assert!(schema.get_entity("NonExistent").is_none());
    }

    #[test]
    fn test_relations_for_entity() {
        let schema = sample_schema();

        let from_post = schema.relations_from("Post");
        assert_eq!(from_post.len(), 1);

        let to_user = schema.relations_to("User");
        assert_eq!(to_user.len(), 1);
    }

    #[test]
    fn test_constraints_for_entity() {
        let schema = sample_schema();

        let user_constraints = schema.constraints_for("User");
        assert_eq!(user_constraints.len(), 1);

        let post_constraints = schema.constraints_for("Post");
        assert_eq!(post_constraints.len(), 1);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let schema = sample_schema();
        let bytes = schema.to_bytes().unwrap();
        let decoded = SchemaBundle::from_bytes(&bytes).unwrap();

        assert_eq!(schema.version, decoded.version);
        assert_eq!(schema.entities.len(), decoded.entities.len());
        assert_eq!(schema.relations.len(), decoded.relations.len());
        assert_eq!(schema.constraints.len(), decoded.constraints.len());
    }
}
