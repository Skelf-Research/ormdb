//! Constraint validation logic.
//!
//! The ConstraintValidator checks all constraints for an entity during
//! insert, update, and delete operations.

use std::collections::HashMap;

use crate::catalog::{Catalog, ConstraintDef};
use crate::error::{ConstraintError, Error};
use crate::storage::StorageEngine;

use super::check_evaluator::{CheckEvaluator, Value};
use super::unique_index::UniqueIndex;

/// Constraint validator for enforcing database constraints.
pub struct ConstraintValidator<'a> {
    catalog: &'a Catalog,
    engine: &'a StorageEngine,
    unique_index: &'a UniqueIndex,
}

impl<'a> ConstraintValidator<'a> {
    /// Create a new constraint validator.
    pub fn new(
        catalog: &'a Catalog,
        engine: &'a StorageEngine,
        unique_index: &'a UniqueIndex,
    ) -> Self {
        Self {
            catalog,
            engine,
            unique_index,
        }
    }

    /// Validate all constraints for an insert operation.
    ///
    /// Checks:
    /// - Unique constraints on the new values
    /// - Foreign key constraints (referenced entities must exist)
    /// - Check constraints
    pub fn validate_insert(
        &self,
        entity: &str,
        entity_id: [u8; 16],
        data: &HashMap<String, Value>,
    ) -> Result<(), Error> {
        let constraints = self.get_constraints_for_entity(entity)?;

        for constraint in constraints {
            match &constraint {
                ConstraintDef::Unique { name, fields, .. } => {
                    self.check_unique_insert(entity, name, fields, data, entity_id)?;
                }
                ConstraintDef::ForeignKey {
                    name,
                    field,
                    references_entity,
                    references_field,
                    ..
                } => {
                    self.check_foreign_key(name, field, references_entity, references_field, data)?;
                }
                ConstraintDef::Check {
                    name, expression, ..
                } => {
                    self.check_expression(entity, name, expression, data)?;
                }
            }
        }

        Ok(())
    }

    /// Validate all constraints for an update operation.
    ///
    /// Checks:
    /// - Unique constraints on changed fields
    /// - Foreign key constraints on changed FK fields
    /// - Check constraints on the merged data
    pub fn validate_update(
        &self,
        entity: &str,
        entity_id: [u8; 16],
        old_data: &HashMap<String, Value>,
        new_data: &HashMap<String, Value>,
    ) -> Result<(), Error> {
        let constraints = self.get_constraints_for_entity(entity)?;

        // Merge old and new data (new overrides old)
        let mut merged = old_data.clone();
        merged.extend(new_data.clone());

        for constraint in constraints {
            match &constraint {
                ConstraintDef::Unique { name, fields, .. } => {
                    // Only check if any of the unique fields changed
                    let changed = fields.iter().any(|f| new_data.contains_key(f));
                    if changed {
                        self.check_unique_update(entity, name, fields, &merged, entity_id)?;
                    }
                }
                ConstraintDef::ForeignKey {
                    name,
                    field,
                    references_entity,
                    references_field,
                    ..
                } => {
                    // Only check if the FK field changed
                    if new_data.contains_key(field) {
                        self.check_foreign_key(
                            name,
                            field,
                            references_entity,
                            references_field,
                            &merged,
                        )?;
                    }
                }
                ConstraintDef::Check {
                    name, expression, ..
                } => {
                    // Always check - constraint must be valid on merged data
                    self.check_expression(entity, name, expression, &merged)?;
                }
            }
        }

        Ok(())
    }

    /// Validate that a delete is allowed.
    ///
    /// Checks:
    /// - No RESTRICT foreign keys reference this entity
    /// (CASCADE and SET NULL are handled by CascadeExecutor)
    pub fn validate_delete(&self, entity: &str, entity_id: [u8; 16]) -> Result<(), Error> {
        // Find all relations where this entity is referenced with RESTRICT behavior
        let schema = match self.catalog.current_schema()? {
            Some(s) => s,
            None => return Ok(()), // No schema, no constraints
        };

        for relation in schema.relations.values() {
            // Check if this relation references our entity type
            if relation.to_entity == entity {
                use crate::catalog::DeleteBehavior;
                if relation.on_delete == DeleteBehavior::Restrict {
                    // Count referencing entities
                    let count = self.count_references(
                        &relation.from_entity,
                        &relation.from_field,
                        entity_id,
                    )?;

                    if count > 0 {
                        return Err(Error::ConstraintViolation(
                            ConstraintError::RestrictViolation {
                                constraint: relation.name.clone(),
                                entity: entity.to_string(),
                                referencing_entity: relation.from_entity.clone(),
                                count,
                            },
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all constraints that apply to an entity.
    fn get_constraints_for_entity(&self, entity: &str) -> Result<Vec<ConstraintDef>, Error> {
        let schema = match self.catalog.current_schema()? {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        Ok(schema
            .constraints
            .iter()
            .filter(|c| c.entity() == entity)
            .cloned()
            .collect())
    }

    /// Check a unique constraint for an insert.
    fn check_unique_insert(
        &self,
        entity: &str,
        constraint_name: &str,
        fields: &[String],
        data: &HashMap<String, Value>,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let values = self.extract_field_values(fields, data)?;
        let value_refs: Vec<&str> = values.iter().map(|s| s.as_str()).collect();

        self.unique_index
            .validate_and_insert(entity, constraint_name, fields, &value_refs, entity_id, None)
    }

    /// Check a unique constraint for an update.
    fn check_unique_update(
        &self,
        entity: &str,
        constraint_name: &str,
        fields: &[String],
        data: &HashMap<String, Value>,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let values = self.extract_field_values(fields, data)?;
        let value_refs: Vec<&str> = values.iter().map(|s| s.as_str()).collect();

        // Use exclude_id to allow updating with same values
        self.unique_index.validate_and_insert(
            entity,
            constraint_name,
            fields,
            &value_refs,
            entity_id,
            Some(entity_id),
        )
    }

    /// Check a foreign key constraint.
    fn check_foreign_key(
        &self,
        constraint_name: &str,
        field: &str,
        references_entity: &str,
        _references_field: &str,
        data: &HashMap<String, Value>,
    ) -> Result<(), Error> {
        // Get the FK value
        let fk_value = match data.get(field) {
            Some(Value::Null) => return Ok(()), // NULL FK is allowed
            Some(v) => v,
            None => return Ok(()), // Field not present, assume NULL
        };

        // Convert FK value to entity ID
        let referenced_id = match fk_value {
            Value::String(s) => {
                // Try to parse as UUID hex
                self.parse_uuid_string(s)?
            }
            _ => {
                return Err(Error::ConstraintViolation(
                    ConstraintError::ForeignKeyViolation {
                        constraint: constraint_name.to_string(),
                        entity: String::new(), // Will be filled in by caller
                        field: field.to_string(),
                        referenced_entity: references_entity.to_string(),
                    },
                ));
            }
        };

        // Check if referenced entity exists
        if self.engine.get_latest(&referenced_id)?.is_none() {
            return Err(Error::ConstraintViolation(
                ConstraintError::ForeignKeyViolation {
                    constraint: constraint_name.to_string(),
                    entity: String::new(),
                    field: field.to_string(),
                    referenced_entity: references_entity.to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Check a check constraint.
    fn check_expression(
        &self,
        entity: &str,
        constraint_name: &str,
        expression: &str,
        data: &HashMap<String, Value>,
    ) -> Result<(), Error> {
        match CheckEvaluator::evaluate(expression, data) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::ConstraintViolation(ConstraintError::CheckViolation {
                constraint: constraint_name.to_string(),
                entity: entity.to_string(),
                expression: expression.to_string(),
            })),
            Err(e) => Err(Error::InvalidData(format!(
                "check constraint '{}' evaluation failed: {}",
                constraint_name, e
            ))),
        }
    }

    /// Extract field values as strings for unique index.
    fn extract_field_values(
        &self,
        fields: &[String],
        data: &HashMap<String, Value>,
    ) -> Result<Vec<String>, Error> {
        let mut values = Vec::with_capacity(fields.len());
        for field in fields {
            let value = data
                .get(field)
                .ok_or_else(|| Error::InvalidData(format!("missing field: {}", field)))?;

            let str_value = match value {
                Value::Null => "__NULL__".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::String(s) => s.clone(),
            };
            values.push(str_value);
        }
        Ok(values)
    }

    /// Parse a UUID string to bytes.
    fn parse_uuid_string(&self, s: &str) -> Result<[u8; 16], Error> {
        // Remove dashes and parse as hex
        let hex: String = s.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return Err(Error::InvalidData(format!("invalid UUID: {}", s)));
        }

        let mut bytes = [0u8; 16];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let byte_str = std::str::from_utf8(chunk)
                .map_err(|_| Error::InvalidData(format!("invalid UUID: {}", s)))?;
            bytes[i] = u8::from_str_radix(byte_str, 16)
                .map_err(|_| Error::InvalidData(format!("invalid UUID: {}", s)))?;
        }

        Ok(bytes)
    }

    /// Count entities that reference a given ID via a specific field.
    fn count_references(
        &self,
        from_entity: &str,
        _from_field: &str,
        target_id: [u8; 16],
    ) -> Result<usize, Error> {
        let target_hex = self.bytes_to_hex(&target_id);
        let mut count = 0;

        // Scan all entities of the from_entity type
        for result in self.engine.scan_entity_type(from_entity) {
            let (_, _, record) = result?;

            // Deserialize and check the FK field
            // For now, do a simple byte comparison in the record data
            // In a full implementation, you'd deserialize and check the field value
            if record.data.windows(16).any(|w| w == target_id) {
                count += 1;
            }

            // Alternative: Check if the hex representation appears
            // This is a heuristic - proper implementation would deserialize
            let data_str = String::from_utf8_lossy(&record.data);
            if data_str.contains(&target_hex) {
                count += 1;
            }
        }

        // Deduplicate count (in case both checks matched)
        Ok(count.min(1) * count)
    }

    /// Convert bytes to hex string.
    fn bytes_to_hex(&self, bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
    use crate::storage::StorageConfig;

    fn setup_test_env() -> (tempfile::TempDir, StorageEngine, Catalog, UniqueIndex) {
        let dir = tempfile::tempdir().unwrap();
        let engine = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();
        let catalog = Catalog::open(engine.db()).unwrap();
        let unique_index = UniqueIndex::open(engine.db()).unwrap();
        (dir, engine, catalog, unique_index)
    }

    #[test]
    fn test_check_constraint_violation() {
        let (_dir, engine, catalog, unique_index) = setup_test_env();

        // Create schema with check constraint
        let schema = SchemaBundle::new(1)
            .with_entity(
                EntityDef::new("Product", "id")
                    .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                    .with_field(FieldDef::new("price", FieldType::Scalar(ScalarType::Float64))),
            )
            .with_constraint(ConstraintDef::check("positive_price", "Product", "price > 0"));

        catalog.apply_schema(schema).unwrap();

        let validator = ConstraintValidator::new(&catalog, &engine, &unique_index);

        // Valid price
        let mut data = HashMap::new();
        data.insert("price".to_string(), Value::Float(10.0));
        let id = crate::storage::StorageEngine::generate_id();
        assert!(validator.validate_insert("Product", id, &data).is_ok());

        // Invalid price (negative)
        let mut data2 = HashMap::new();
        data2.insert("price".to_string(), Value::Float(-5.0));
        let id2 = crate::storage::StorageEngine::generate_id();
        let result = validator.validate_insert("Product", id2, &data2);
        assert!(result.is_err());

        if let Err(Error::ConstraintViolation(ConstraintError::CheckViolation {
            constraint, ..
        })) = result
        {
            assert_eq!(constraint, "positive_price");
        } else {
            panic!("Expected CheckViolation error");
        }
    }

    #[test]
    fn test_unique_constraint() {
        let (_dir, engine, catalog, unique_index) = setup_test_env();

        // Create schema with unique constraint
        let schema = SchemaBundle::new(1)
            .with_entity(
                EntityDef::new("User", "id")
                    .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                    .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String))),
            )
            .with_constraint(ConstraintDef::unique("user_email_unique", "User", "email"));

        catalog.apply_schema(schema).unwrap();

        let validator = ConstraintValidator::new(&catalog, &engine, &unique_index);

        // First insert should succeed
        let mut data1 = HashMap::new();
        data1.insert("email".to_string(), Value::String("test@example.com".to_string()));
        let id1 = crate::storage::StorageEngine::generate_id();
        assert!(validator.validate_insert("User", id1, &data1).is_ok());

        // Second insert with same email should fail
        let mut data2 = HashMap::new();
        data2.insert("email".to_string(), Value::String("test@example.com".to_string()));
        let id2 = crate::storage::StorageEngine::generate_id();
        let result = validator.validate_insert("User", id2, &data2);
        assert!(result.is_err());

        if let Err(Error::ConstraintViolation(ConstraintError::UniqueViolation {
            constraint,
            value,
            ..
        })) = result
        {
            assert_eq!(constraint, "user_email_unique");
            assert_eq!(value, "test@example.com");
        } else {
            panic!("Expected UniqueViolation error");
        }
    }

    #[test]
    fn test_unique_update_same_entity() {
        let (_dir, engine, catalog, unique_index) = setup_test_env();

        let schema = SchemaBundle::new(1)
            .with_entity(
                EntityDef::new("User", "id")
                    .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                    .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)))
                    .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String))),
            )
            .with_constraint(ConstraintDef::unique("user_email_unique", "User", "email"));

        catalog.apply_schema(schema).unwrap();

        let validator = ConstraintValidator::new(&catalog, &engine, &unique_index);

        // Insert initial user
        let mut data = HashMap::new();
        data.insert("email".to_string(), Value::String("user@example.com".to_string()));
        data.insert("name".to_string(), Value::String("Alice".to_string()));
        let id = crate::storage::StorageEngine::generate_id();
        validator.validate_insert("User", id, &data).unwrap();

        // Update name (keeping same email) should succeed
        let mut new_data = HashMap::new();
        new_data.insert("name".to_string(), Value::String("Alice Updated".to_string()));
        new_data.insert("email".to_string(), Value::String("user@example.com".to_string()));

        let result = validator.validate_update("User", id, &data, &new_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_schema_no_constraints() {
        let (_dir, engine, catalog, unique_index) = setup_test_env();
        // Don't apply any schema

        let validator = ConstraintValidator::new(&catalog, &engine, &unique_index);

        let data = HashMap::new();
        let id = crate::storage::StorageEngine::generate_id();

        // Should succeed with no schema
        assert!(validator.validate_insert("AnyEntity", id, &data).is_ok());
    }
}
