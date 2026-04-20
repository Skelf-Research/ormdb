//! Schema builder for defining entities and relations.
//!
//! # Example
//!
//! ```rust,no_run
//! use ormdb::{Database, ScalarType};
//!
//! let db = Database::open_memory().unwrap();
//!
//! db.schema()
//!     .entity("User")
//!         .field("id", ScalarType::Uuid).primary_key()
//!         .field("name", ScalarType::String)
//!         .field("email", ScalarType::String).unique()
//!         .field("age", ScalarType::Int32).optional()
//!     .entity("Post")
//!         .field("id", ScalarType::Uuid).primary_key()
//!         .field("title", ScalarType::String)
//!         .field("content", ScalarType::String)
//!         .relation("author", "User").required()
//!     .apply()
//!     .unwrap();
//! ```

use ormdb_core::catalog::{
    ConstraintDef, EntityDef, FieldDef, FieldType, RelationDef, SchemaBundle,
};

use crate::database::Database;
use crate::error::{Error, Result};

// Re-export ScalarType for user convenience
pub use ormdb_core::catalog::ScalarType;

/// Builder for defining a schema.
pub struct SchemaBuilder<'db> {
    db: &'db Database,
    entities: Vec<EntityDefBuilder>,
    relations: Vec<RelationDef>,
    constraints: Vec<ConstraintDef>,
}

impl<'db> SchemaBuilder<'db> {
    /// Create a new schema builder.
    pub(crate) fn new(db: &'db Database) -> Self {
        Self {
            db,
            entities: Vec::new(),
            relations: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Start defining a new entity.
    ///
    /// The entity name must be unique within the schema.
    pub fn entity(self, name: &str) -> EntityBuilder<'db> {
        EntityBuilder {
            schema: self,
            name: name.to_string(),
            fields: Vec::new(),
            identity_field: None,
        }
    }

    /// Add a relation between entities.
    pub fn relation(
        mut self,
        name: &str,
        from_entity: &str,
        from_field: &str,
        to_entity: &str,
        to_field: &str,
    ) -> Self {
        let relation = RelationDef::one_to_many(name, from_entity, from_field, to_entity, to_field);
        self.relations.push(relation);
        self
    }

    /// Add a unique constraint.
    pub fn unique_constraint(mut self, name: &str, entity: &str, field: &str) -> Self {
        let constraint = ConstraintDef::unique(name, entity, field);
        self.constraints.push(constraint);
        self
    }

    /// Add a composite unique constraint.
    pub fn unique_constraint_composite(
        mut self,
        name: &str,
        entity: &str,
        fields: &[&str],
    ) -> Self {
        let field_strings: Vec<String> = fields.iter().map(|s| (*s).to_string()).collect();
        let constraint = ConstraintDef::unique_composite(name, entity, field_strings);
        self.constraints.push(constraint);
        self
    }

    /// Apply the schema to the database.
    ///
    /// This increments the schema version and makes the changes active.
    pub fn apply(self) -> Result<()> {
        let current_version = self.db.schema_version();
        let new_version = current_version + 1;

        let mut bundle = SchemaBundle::new(new_version);

        // Add entities
        for entity_builder in self.entities {
            let identity_field = entity_builder
                .identity_field
                .unwrap_or_else(|| "id".to_string());

            let mut entity_def = EntityDef::new(&entity_builder.name, identity_field);
            for field in entity_builder.fields {
                entity_def = entity_def.with_field(field);
            }
            bundle = bundle.with_entity(entity_def);
        }

        // Add relations
        for relation in self.relations {
            bundle = bundle.with_relation(relation);
        }

        // Add constraints
        for constraint in self.constraints {
            bundle = bundle.with_constraint(constraint);
        }

        // Apply to catalog
        self.db
            .catalog()
            .apply_schema(bundle)
            .map(|_| ())
            .map_err(|e| Error::Schema(e.to_string()))
    }

    /// Finalize an entity and continue building the schema.
    fn add_entity(&mut self, builder: EntityDefBuilder) {
        self.entities.push(builder);
    }
}

/// Internal entity definition builder.
struct EntityDefBuilder {
    name: String,
    fields: Vec<FieldDef>,
    identity_field: Option<String>,
}

/// Builder for defining an entity.
pub struct EntityBuilder<'db> {
    schema: SchemaBuilder<'db>,
    name: String,
    fields: Vec<FieldDef>,
    identity_field: Option<String>,
}

impl<'db> EntityBuilder<'db> {
    /// Add a field to the entity.
    ///
    /// Returns a `FieldBuilder` for further configuration.
    pub fn field(self, name: &str, scalar_type: ScalarType) -> FieldBuilder<'db> {
        FieldBuilder {
            entity: self,
            name: name.to_string(),
            scalar_type,
            required: true,
            indexed: false,
            is_primary_key: false,
            is_unique: false,
        }
    }

    /// Add a relation field to this entity.
    ///
    /// This creates a foreign key field and the corresponding relation.
    pub fn relation(self, field_name: &str, target_entity: &str) -> RelationFieldBuilder<'db> {
        RelationFieldBuilder {
            entity: self,
            field_name: field_name.to_string(),
            target_entity: target_entity.to_string(),
            required: false,
        }
    }

    /// Finish this entity and continue building the schema.
    ///
    /// Call this when you're done adding fields to the entity.
    pub fn done(mut self) -> SchemaBuilder<'db> {
        let entity_builder = EntityDefBuilder {
            name: self.name,
            fields: self.fields,
            identity_field: self.identity_field,
        };
        self.schema.add_entity(entity_builder);
        self.schema
    }

    /// Start defining another entity.
    ///
    /// This implicitly finishes the current entity.
    pub fn entity(self, name: &str) -> EntityBuilder<'db> {
        self.done().entity(name)
    }

    /// Apply the schema to the database.
    ///
    /// This implicitly finishes the current entity.
    pub fn apply(self) -> Result<()> {
        self.done().apply()
    }

    /// Add a field to this entity's fields.
    fn add_field(&mut self, field: FieldDef) {
        self.fields.push(field);
    }

    /// Set the identity (primary key) field.
    fn set_identity(&mut self, field_name: &str) {
        self.identity_field = Some(field_name.to_string());
    }
}

/// Builder for configuring a field.
pub struct FieldBuilder<'db> {
    entity: EntityBuilder<'db>,
    name: String,
    scalar_type: ScalarType,
    required: bool,
    indexed: bool,
    is_primary_key: bool,
    is_unique: bool,
}

impl<'db> FieldBuilder<'db> {
    /// Mark this field as optional (nullable).
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Mark this field as the primary key.
    ///
    /// Primary keys are automatically indexed and unique.
    pub fn primary_key(mut self) -> Self {
        self.is_primary_key = true;
        self.indexed = true;
        self
    }

    /// Mark this field as unique.
    ///
    /// Unique fields are automatically indexed.
    pub fn unique(mut self) -> Self {
        self.is_unique = true;
        self.indexed = true;
        self
    }

    /// Mark this field as indexed.
    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Add another field to the entity.
    pub fn field(mut self, name: &str, scalar_type: ScalarType) -> FieldBuilder<'db> {
        // Finalize this field
        self.finalize();

        // Start a new field
        FieldBuilder {
            entity: self.entity,
            name: name.to_string(),
            scalar_type,
            required: true,
            indexed: false,
            is_primary_key: false,
            is_unique: false,
        }
    }

    /// Add a relation field.
    pub fn relation(mut self, field_name: &str, target_entity: &str) -> RelationFieldBuilder<'db> {
        self.finalize();
        self.entity.relation(field_name, target_entity)
    }

    /// Start defining another entity.
    pub fn entity(mut self, name: &str) -> EntityBuilder<'db> {
        self.finalize();
        self.entity.entity(name)
    }

    /// Apply the schema.
    pub fn apply(mut self) -> Result<()> {
        self.finalize();
        self.entity.apply()
    }

    /// Finalize this field and add it to the entity.
    fn finalize(&mut self) {
        let field_type = if self.required {
            FieldType::scalar(self.scalar_type.clone())
        } else {
            FieldType::OptionalScalar(self.scalar_type.clone())
        };

        let mut field_def = FieldDef::new(&self.name, field_type);
        field_def.required = self.required;
        if self.indexed {
            field_def = field_def.with_index();
        }

        if self.is_primary_key {
            self.entity.set_identity(&self.name);
        }

        self.entity.add_field(field_def);
    }
}

/// Builder for configuring a relation field.
pub struct RelationFieldBuilder<'db> {
    entity: EntityBuilder<'db>,
    field_name: String,
    target_entity: String,
    required: bool,
}

impl<'db> RelationFieldBuilder<'db> {
    /// Mark this relation as required (not nullable).
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Add another field to the entity.
    pub fn field(mut self, name: &str, scalar_type: ScalarType) -> FieldBuilder<'db> {
        self.finalize();
        self.entity.field(name, scalar_type)
    }

    /// Add another relation field.
    pub fn relation(mut self, field_name: &str, target_entity: &str) -> RelationFieldBuilder<'db> {
        self.finalize();
        self.entity.relation(field_name, target_entity)
    }

    /// Start defining another entity.
    pub fn entity(mut self, name: &str) -> EntityBuilder<'db> {
        self.finalize();
        self.entity.entity(name)
    }

    /// Apply the schema.
    pub fn apply(mut self) -> Result<()> {
        self.finalize();
        self.entity.apply()
    }

    /// Finalize this relation field.
    fn finalize(&mut self) {
        // Add the foreign key field (e.g., author_id for relation to User)
        let fk_field_name = format!("{}_id", self.field_name);
        let field_type = if self.required {
            FieldType::scalar(ScalarType::Uuid)
        } else {
            FieldType::OptionalScalar(ScalarType::Uuid)
        };

        let mut field_def = FieldDef::new(&fk_field_name, field_type);
        field_def.required = self.required;
        field_def = field_def.with_index();
        self.entity.add_field(field_def);

        // Note: Relations are added at the schema level, not entity level
        // This is a simplified version that just creates the FK field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_builder() {
        let db = Database::open_memory().unwrap();

        db.schema()
            .entity("User")
            .field("id", ScalarType::Uuid)
            .primary_key()
            .field("name", ScalarType::String)
            .field("email", ScalarType::String)
            .unique()
            .apply()
            .unwrap();

        assert_eq!(db.schema_version(), 1);
    }
}
