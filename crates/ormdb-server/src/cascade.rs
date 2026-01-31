//! Cascade executor for handling referential integrity on deletes.
//!
//! This module implements the cascade behavior for delete operations:
//! - CASCADE: Delete related entities recursively
//! - RESTRICT: Prevent deletion if related entities exist
//! - SET NULL: Set foreign key fields to null on related entities

use std::collections::HashSet;

use ormdb_core::catalog::{Catalog, DeleteBehavior};
use ormdb_core::error::CascadeError;
use ormdb_core::storage::{Record, StorageEngine, Transaction};

use crate::error::Error;

/// Maximum cascade depth to prevent infinite recursion.
const MAX_CASCADE_DEPTH: usize = 100;

/// Result of a cascade operation.
#[derive(Debug, Default)]
pub struct CascadeResult {
    /// Entities that were deleted.
    pub deleted_entities: Vec<(String, [u8; 16])>,
    /// Fields that were set to null.
    pub nullified_fields: Vec<(String, [u8; 16], String)>,
}

impl CascadeResult {
    /// Create an empty cascade result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the total number of affected entities.
    pub fn affected_count(&self) -> usize {
        self.deleted_entities.len() + self.nullified_fields.len()
    }
}

/// Executes cascade operations for delete.
pub struct CascadeExecutor<'a> {
    catalog: &'a Catalog,
    engine: &'a StorageEngine,
}

impl<'a> CascadeExecutor<'a> {
    /// Create a new cascade executor.
    pub fn new(catalog: &'a Catalog, engine: &'a StorageEngine) -> Self {
        Self { catalog, engine }
    }

    /// Process cascades for a delete operation.
    ///
    /// This method checks all relations that reference the entity being deleted
    /// and applies the appropriate cascade behavior.
    pub fn process_delete(
        &self,
        entity: &str,
        entity_id: [u8; 16],
        tx: &mut Transaction<'_>,
    ) -> Result<CascadeResult, Error> {
        let mut result = CascadeResult::new();
        let mut visited = HashSet::new();

        self.process_delete_recursive(entity, entity_id, tx, &mut result, &mut visited, 0)?;

        Ok(result)
    }

    /// Recursively process cascades.
    fn process_delete_recursive(
        &self,
        entity: &str,
        entity_id: [u8; 16],
        tx: &mut Transaction<'_>,
        result: &mut CascadeResult,
        visited: &mut HashSet<[u8; 16]>,
        depth: usize,
    ) -> Result<(), Error> {
        // Check for maximum depth
        if depth > MAX_CASCADE_DEPTH {
            return Err(Error::Storage(ormdb_core::Error::CascadeError(
                CascadeError::MaxDepthExceeded { depth },
            )));
        }

        // Prevent cycles
        if visited.contains(&entity_id) {
            return Ok(());
        }
        visited.insert(entity_id);

        // Get schema to find relations
        let schema = match self.catalog.current_schema()? {
            Some(s) => s,
            None => return Ok(()), // No schema, no relations to check
        };

        // Find all relations that reference this entity type
        for relation in schema.relations.values() {
            if relation.to_entity != entity {
                continue;
            }

            // Find all entities of the referencing type that point to this entity
            let referencing_ids = self.find_referencing_entities(
                &relation.from_entity,
                &relation.from_field,
                entity_id,
            )?;

            if referencing_ids.is_empty() {
                continue;
            }

            match relation.on_delete {
                DeleteBehavior::Restrict => {
                    // Cannot delete - there are references
                    return Err(Error::Storage(ormdb_core::Error::CascadeError(
                        CascadeError::RestrictViolation {
                            entity: entity.to_string(),
                            referencing_entity: relation.from_entity.clone(),
                            count: referencing_ids.len(),
                        },
                    )));
                }
                DeleteBehavior::Cascade => {
                    // Delete all referencing entities
                    for ref_id in referencing_ids {
                        // First, recursively cascade from the referencing entity
                        self.process_delete_recursive(
                            &relation.from_entity,
                            ref_id,
                            tx,
                            result,
                            visited,
                            depth + 1,
                        )?;

                        // Then delete the referencing entity
                        tx.delete_typed(&relation.from_entity, ref_id);
                        result
                            .deleted_entities
                            .push((relation.from_entity.clone(), ref_id));
                    }
                }
                DeleteBehavior::SetNull => {
                    // Set the FK field to null on all referencing entities
                    for ref_id in referencing_ids {
                        self.set_field_null(
                            tx,
                            &relation.from_entity,
                            ref_id,
                            &relation.from_field,
                        )?;
                        result.nullified_fields.push((
                            relation.from_entity.clone(),
                            ref_id,
                            relation.from_field.clone(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Find all entities of a type that reference a given ID.
    fn find_referencing_entities(
        &self,
        from_entity: &str,
        from_field: &str,
        target_id: [u8; 16],
    ) -> Result<Vec<[u8; 16]>, Error> {
        let mut referencing = Vec::new();

        // Scan all entities of the from_entity type
        for scan_result in self.engine.scan_entity_type(from_entity) {
            let (entity_id, _, record) = scan_result?;

            // Check if this entity references the target
            if self.entity_references_target(&record.data, from_field, target_id) {
                referencing.push(entity_id);
            }
        }

        Ok(referencing)
    }

    /// Check if an entity's data references a target ID in a specific field.
    fn entity_references_target(&self, data: &[u8], field: &str, target_id: [u8; 16]) -> bool {
        // Decode the entity data
        if let Ok(fields) = ormdb_core::query::decode_entity(data) {
            for (name, value) in fields {
                if name == field {
                    // Check if the field value matches the target ID
                    match value {
                        ormdb_proto::Value::Uuid(id) => return id == target_id,
                        ormdb_proto::Value::String(s) => {
                            // Try to parse as UUID hex string
                            if let Ok(id) = parse_uuid_string(&s) {
                                return id == target_id;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        false
    }

    /// Set a field to null on an entity.
    fn set_field_null(
        &self,
        tx: &mut Transaction<'_>,
        entity_type: &str,
        entity_id: [u8; 16],
        field: &str,
    ) -> Result<(), Error> {
        // Get the current entity data
        let (_version, record) = match self.engine.get_latest(&entity_id)? {
            Some(r) => r,
            None => return Ok(()), // Entity doesn't exist
        };

        // Decode fields
        let mut fields = ormdb_core::query::decode_entity(&record.data)?;

        // Find and update the field
        let mut found = false;
        for (name, value) in &mut fields {
            if name == field {
                *value = ormdb_proto::Value::Null;
                found = true;
                break;
            }
        }

        if !found {
            // Field doesn't exist, add it as null
            fields.push((field.to_string(), ormdb_proto::Value::Null));
        }

        // Re-encode and queue update
        let encoded = ormdb_core::query::encode_entity(&fields)?;
        tx.update(entity_type, entity_id, Record::new(encoded));

        Ok(())
    }

    /// Check if deleting an entity is allowed (without actually performing cascades).
    ///
    /// This is useful for validation before committing.
    pub fn can_delete(&self, entity: &str, entity_id: [u8; 16]) -> Result<(), Error> {
        let schema = match self.catalog.current_schema()? {
            Some(s) => s,
            None => return Ok(()),
        };

        for relation in schema.relations.values() {
            if relation.to_entity != entity {
                continue;
            }

            if relation.on_delete == DeleteBehavior::Restrict {
                let count = self.count_references(
                    &relation.from_entity,
                    &relation.from_field,
                    entity_id,
                )?;

                if count > 0 {
                    return Err(Error::Storage(ormdb_core::Error::CascadeError(
                        CascadeError::RestrictViolation {
                            entity: entity.to_string(),
                            referencing_entity: relation.from_entity.clone(),
                            count,
                        },
                    )));
                }
            }
        }

        Ok(())
    }

    /// Count how many entities reference a given ID.
    fn count_references(
        &self,
        from_entity: &str,
        from_field: &str,
        target_id: [u8; 16],
    ) -> Result<usize, Error> {
        let refs = self.find_referencing_entities(from_entity, from_field, target_id)?;
        Ok(refs.len())
    }
}

/// Parse a UUID string to bytes.
fn parse_uuid_string(s: &str) -> Result<[u8; 16], ()> {
    // Remove dashes and parse as hex
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return Err(());
    }

    let mut bytes = [0u8; 16];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let byte_str = std::str::from_utf8(chunk).map_err(|_| ())?;
        bytes[i] = u8::from_str_radix(byte_str, 16).map_err(|_| ())?;
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::catalog::{
        EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle,
    };
    use ormdb_core::storage::{StorageConfig, VersionedKey};

    fn setup_test_env() -> (tempfile::TempDir, StorageEngine, Catalog) {
        let dir = tempfile::tempdir().unwrap();
        let engine = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();
        let catalog = Catalog::open(engine.db()).unwrap();
        (dir, engine, catalog)
    }

    fn create_test_schema() -> SchemaBundle {
        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

        SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post)
    }

    #[test]
    fn test_no_cascades_without_relations() {
        let (_dir, engine, catalog) = setup_test_env();

        // Schema without relations
        let schema = create_test_schema();
        catalog.apply_schema(schema).unwrap();

        let cascade = CascadeExecutor::new(&catalog, &engine);

        // Create a user
        let user_id = StorageEngine::generate_id();
        let user_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(user_id)),
            ("name".to_string(), ormdb_proto::Value::String("Alice".to_string())),
        ])
        .unwrap();
        engine
            .put_typed("User", VersionedKey::now(user_id), Record::new(user_data))
            .unwrap();

        // Process delete - should succeed with no cascades
        let mut tx = engine.transaction();
        let result = cascade.process_delete("User", user_id, &mut tx).unwrap();

        assert_eq!(result.deleted_entities.len(), 0);
        assert_eq!(result.nullified_fields.len(), 0);
    }

    #[test]
    fn test_restrict_prevents_delete() {
        let (_dir, engine, catalog) = setup_test_env();

        // Schema with RESTRICT relation
        let schema = create_test_schema().with_relation(
            RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id")
                .with_on_delete(DeleteBehavior::Restrict),
        );
        catalog.apply_schema(schema).unwrap();

        let cascade = CascadeExecutor::new(&catalog, &engine);

        // Create a user
        let user_id = StorageEngine::generate_id();
        let user_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(user_id)),
            ("name".to_string(), ormdb_proto::Value::String("Alice".to_string())),
        ])
        .unwrap();
        engine
            .put_typed("User", VersionedKey::now(user_id), Record::new(user_data))
            .unwrap();

        // Create a post referencing the user
        let post_id = StorageEngine::generate_id();
        let post_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(post_id)),
            ("title".to_string(), ormdb_proto::Value::String("Hello".to_string())),
            ("author_id".to_string(), ormdb_proto::Value::Uuid(user_id)),
        ])
        .unwrap();
        engine
            .put_typed("Post", VersionedKey::now(post_id), Record::new(post_data))
            .unwrap();

        // Flush to ensure data is persisted
        engine.flush().unwrap();

        // Try to delete user - should fail with RestrictViolation
        let mut tx = engine.transaction();
        let result = cascade.process_delete("User", user_id, &mut tx);

        assert!(result.is_err());
        if let Err(Error::Storage(ormdb_core::Error::CascadeError(CascadeError::RestrictViolation {
            referencing_entity,
            count,
            ..
        }))) = result
        {
            assert_eq!(referencing_entity, "Post");
            assert_eq!(count, 1);
        } else {
            panic!("Expected RestrictViolation error");
        }
    }

    #[test]
    fn test_cascade_delete() {
        let (_dir, engine, catalog) = setup_test_env();

        // Schema with CASCADE relation
        let schema = create_test_schema().with_relation(
            RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id")
                .with_on_delete(DeleteBehavior::Cascade),
        );
        catalog.apply_schema(schema).unwrap();

        let cascade = CascadeExecutor::new(&catalog, &engine);

        // Create a user
        let user_id = StorageEngine::generate_id();
        let user_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(user_id)),
            ("name".to_string(), ormdb_proto::Value::String("Alice".to_string())),
        ])
        .unwrap();
        engine
            .put_typed("User", VersionedKey::now(user_id), Record::new(user_data))
            .unwrap();

        // Create two posts referencing the user
        for i in 0..2 {
            let post_id = StorageEngine::generate_id();
            let post_data = ormdb_core::query::encode_entity(&[
                ("id".to_string(), ormdb_proto::Value::Uuid(post_id)),
                (
                    "title".to_string(),
                    ormdb_proto::Value::String(format!("Post {}", i)),
                ),
                ("author_id".to_string(), ormdb_proto::Value::Uuid(user_id)),
            ])
            .unwrap();
            engine
                .put_typed("Post", VersionedKey::now(post_id), Record::new(post_data))
                .unwrap();
        }

        engine.flush().unwrap();

        // Delete user - should cascade to posts
        let mut tx = engine.transaction();
        let result = cascade.process_delete("User", user_id, &mut tx).unwrap();

        // Two posts should be marked for deletion
        assert_eq!(result.deleted_entities.len(), 2);
        assert!(result.deleted_entities.iter().all(|(e, _)| e == "Post"));
    }

    #[test]
    fn test_can_delete() {
        let (_dir, engine, catalog) = setup_test_env();

        // Schema with RESTRICT relation
        let schema = create_test_schema().with_relation(
            RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id")
                .with_on_delete(DeleteBehavior::Restrict),
        );
        catalog.apply_schema(schema).unwrap();

        let cascade = CascadeExecutor::new(&catalog, &engine);

        // Create a user without posts
        let user_id = StorageEngine::generate_id();
        let user_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(user_id)),
            ("name".to_string(), ormdb_proto::Value::String("Bob".to_string())),
        ])
        .unwrap();
        engine
            .put_typed("User", VersionedKey::now(user_id), Record::new(user_data))
            .unwrap();

        // can_delete should succeed (no posts)
        assert!(cascade.can_delete("User", user_id).is_ok());

        // Create a post
        let post_id = StorageEngine::generate_id();
        let post_data = ormdb_core::query::encode_entity(&[
            ("id".to_string(), ormdb_proto::Value::Uuid(post_id)),
            ("title".to_string(), ormdb_proto::Value::String("Hello".to_string())),
            ("author_id".to_string(), ormdb_proto::Value::Uuid(user_id)),
        ])
        .unwrap();
        engine
            .put_typed("Post", VersionedKey::now(post_id), Record::new(post_data))
            .unwrap();
        engine.flush().unwrap();

        // Now can_delete should fail
        assert!(cascade.can_delete("User", user_id).is_err());
    }
}
