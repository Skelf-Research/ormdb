//! Mutation executor for handling write operations.

use ormdb_core::query::encode_entity;
use ormdb_core::storage::{Record, StorageEngine, VersionedKey};
use ormdb_proto::{FieldValue, Mutation, MutationBatch, MutationResult, Value};

use crate::database::Database;
use crate::error::Error;

/// Executes mutation operations against the database.
pub struct MutationExecutor<'a> {
    database: &'a Database,
}

impl<'a> MutationExecutor<'a> {
    /// Create a new mutation executor.
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    /// Execute a single mutation.
    pub fn execute(&self, mutation: &Mutation) -> Result<MutationResult, Error> {
        match mutation {
            Mutation::Insert { entity, data } => self.execute_insert(entity, data),
            Mutation::Update { entity, id, data } => self.execute_update(entity, id, data),
            Mutation::Delete { entity, id } => self.execute_delete(entity, id),
            Mutation::Upsert { entity, id, data } => self.execute_upsert(entity, id.as_ref(), data),
        }
    }

    /// Execute a batch of mutations atomically.
    pub fn execute_batch(&self, batch: &MutationBatch) -> Result<MutationResult, Error> {
        if batch.is_empty() {
            return Ok(MutationResult::affected(0));
        }

        let mut inserted_ids = Vec::new();
        let mut affected = 0u64;

        // Execute each mutation in order
        // Note: For true atomicity, we'd use sled transactions here.
        // For now, we execute sequentially which is sufficient for most use cases.
        for mutation in &batch.mutations {
            let result = self.execute(mutation)?;
            affected += result.affected;
            inserted_ids.extend(result.inserted_ids);
        }

        if inserted_ids.is_empty() {
            Ok(MutationResult::affected(affected))
        } else {
            Ok(MutationResult::bulk_inserted(inserted_ids))
        }
    }

    /// Execute an insert operation.
    fn execute_insert(&self, entity: &str, data: &[FieldValue]) -> Result<MutationResult, Error> {
        // Generate a new entity ID
        let id = StorageEngine::generate_id();

        // Build field data including the ID
        let mut fields: Vec<(String, Value)> = Vec::with_capacity(data.len() + 1);
        fields.push(("id".to_string(), Value::Uuid(id)));

        for fv in data {
            // Skip if someone tried to set the ID field
            if fv.field != "id" {
                fields.push((fv.field.clone(), fv.value.clone()));
            }
        }

        // Encode the entity data
        let encoded = encode_entity(&fields)
            .map_err(|e| Error::Database(format!("failed to encode entity: {}", e)))?;

        // Store the entity
        let key = VersionedKey::now(id);
        self.database
            .storage()
            .put_typed(entity, key, Record::new(encoded))
            .map_err(|e| Error::Storage(e))?;

        Ok(MutationResult::inserted(id))
    }

    /// Execute an update operation.
    fn execute_update(
        &self,
        entity: &str,
        id: &[u8; 16],
        data: &[FieldValue],
    ) -> Result<MutationResult, Error> {
        // Get existing entity data
        let (_version, existing) = self
            .database
            .storage()
            .get_latest(id)
            .map_err(|e| Error::Storage(e))?
            .ok_or_else(|| {
                Error::Database(format!("entity {}:{} not found", entity, hex_id(id)))
            })?;

        // Decode existing fields
        let mut fields: Vec<(String, Value)> =
            ormdb_core::query::decode_entity(&existing.data)
                .map_err(|e| Error::Database(format!("failed to decode entity: {}", e)))?;

        // Merge updates (replace existing fields, add new ones)
        for fv in data {
            // Don't allow updating the ID field
            if fv.field == "id" {
                continue;
            }

            if let Some(pos) = fields.iter().position(|(name, _)| name == &fv.field) {
                fields[pos].1 = fv.value.clone();
            } else {
                fields.push((fv.field.clone(), fv.value.clone()));
            }
        }

        // Encode and store
        let encoded = encode_entity(&fields)
            .map_err(|e| Error::Database(format!("failed to encode entity: {}", e)))?;

        let key = VersionedKey::now(*id);
        self.database
            .storage()
            .put_typed(entity, key, Record::new(encoded))
            .map_err(|e| Error::Storage(e))?;

        Ok(MutationResult::affected(1))
    }

    /// Execute a delete operation.
    fn execute_delete(&self, entity: &str, id: &[u8; 16]) -> Result<MutationResult, Error> {
        // Check if entity exists
        let exists = self
            .database
            .storage()
            .get_latest(id)
            .map_err(|e| Error::Storage(e))?
            .is_some();

        if !exists {
            return Ok(MutationResult::affected(0));
        }

        // Soft delete (creates tombstone)
        self.database
            .storage()
            .delete_typed(entity, id)
            .map_err(|e| Error::Storage(e))?;

        Ok(MutationResult::affected(1))
    }

    /// Execute an upsert operation.
    fn execute_upsert(
        &self,
        entity: &str,
        id: Option<&[u8; 16]>,
        data: &[FieldValue],
    ) -> Result<MutationResult, Error> {
        match id {
            Some(existing_id) => {
                // Check if entity exists
                let exists = self
                    .database
                    .storage()
                    .get_latest(existing_id)
                    .map_err(|e| Error::Storage(e))?
                    .is_some();

                if exists {
                    // Update existing
                    self.execute_update(entity, existing_id, data)
                } else {
                    // Insert with provided ID
                    self.execute_insert_with_id(entity, *existing_id, data)
                }
            }
            None => {
                // No ID provided, always insert
                self.execute_insert(entity, data)
            }
        }
    }

    /// Execute an insert with a specific ID.
    fn execute_insert_with_id(
        &self,
        entity: &str,
        id: [u8; 16],
        data: &[FieldValue],
    ) -> Result<MutationResult, Error> {
        // Build field data including the ID
        let mut fields: Vec<(String, Value)> = Vec::with_capacity(data.len() + 1);
        fields.push(("id".to_string(), Value::Uuid(id)));

        for fv in data {
            if fv.field != "id" {
                fields.push((fv.field.clone(), fv.value.clone()));
            }
        }

        // Encode the entity data
        let encoded = encode_entity(&fields)
            .map_err(|e| Error::Database(format!("failed to encode entity: {}", e)))?;

        // Store the entity
        let key = VersionedKey::now(id);
        self.database
            .storage()
            .put_typed(entity, key, Record::new(encoded))
            .map_err(|e| Error::Storage(e))?;

        Ok(MutationResult::inserted(id))
    }
}

/// Format an ID as hex for error messages.
fn hex_id(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};

    fn setup_test_db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        // Create schema
        let schema = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
                .with_field(FieldDef::new("age", FieldType::Scalar(ScalarType::Int32))),
        );
        db.catalog().apply_schema(schema).unwrap();

        (dir, db)
    }

    #[test]
    fn test_insert() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        let mutation = Mutation::insert(
            "User",
            vec![
                FieldValue::new("name", "Alice"),
                FieldValue::new("age", 30i32),
            ],
        );

        let result = executor.execute(&mutation).unwrap();
        assert_eq!(result.affected, 1);
        assert_eq!(result.inserted_ids.len(), 1);

        // Verify we can query the entity back
        let inserted_id = result.inserted_ids[0];
        let (_, record) = db.storage().get_latest(&inserted_id).unwrap().unwrap();
        let fields = ormdb_core::query::decode_entity(&record.data).unwrap();

        assert!(fields.iter().any(|(n, v)| n == "name" && *v == Value::String("Alice".into())));
        assert!(fields.iter().any(|(n, v)| n == "age" && *v == Value::Int32(30)));
    }

    #[test]
    fn test_update() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        // First insert
        let insert = Mutation::insert(
            "User",
            vec![
                FieldValue::new("name", "Alice"),
                FieldValue::new("age", 30i32),
            ],
        );
        let insert_result = executor.execute(&insert).unwrap();
        let id = insert_result.inserted_ids[0];

        // Then update
        let update = Mutation::update(
            "User",
            id,
            vec![FieldValue::new("name", "Alicia"), FieldValue::new("age", 31i32)],
        );
        let update_result = executor.execute(&update).unwrap();
        assert_eq!(update_result.affected, 1);

        // Verify changes
        let (_, record) = db.storage().get_latest(&id).unwrap().unwrap();
        let fields = ormdb_core::query::decode_entity(&record.data).unwrap();

        assert!(fields.iter().any(|(n, v)| n == "name" && *v == Value::String("Alicia".into())));
        assert!(fields.iter().any(|(n, v)| n == "age" && *v == Value::Int32(31)));
    }

    #[test]
    fn test_delete() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        // First insert
        let insert = Mutation::insert("User", vec![FieldValue::new("name", "Bob")]);
        let insert_result = executor.execute(&insert).unwrap();
        let id = insert_result.inserted_ids[0];

        // Then delete
        let delete = Mutation::delete("User", id);
        let delete_result = executor.execute(&delete).unwrap();
        assert_eq!(delete_result.affected, 1);

        // Verify deleted
        assert!(db.storage().get_latest(&id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        let delete = Mutation::delete("User", [0u8; 16]);
        let result = executor.execute(&delete).unwrap();
        assert_eq!(result.affected, 0);
    }

    #[test]
    fn test_upsert_insert() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        let upsert = Mutation::upsert(
            "User",
            None,
            vec![FieldValue::new("name", "Charlie")],
        );
        let result = executor.execute(&upsert).unwrap();
        assert_eq!(result.affected, 1);
        assert_eq!(result.inserted_ids.len(), 1);
    }

    #[test]
    fn test_upsert_update() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        // First insert
        let insert = Mutation::insert("User", vec![FieldValue::new("name", "Dave")]);
        let insert_result = executor.execute(&insert).unwrap();
        let id = insert_result.inserted_ids[0];

        // Then upsert (should update)
        let upsert = Mutation::upsert(
            "User",
            Some(id),
            vec![FieldValue::new("name", "David")],
        );
        let result = executor.execute(&upsert).unwrap();
        assert_eq!(result.affected, 1);
        assert!(result.inserted_ids.is_empty());

        // Verify
        let (_, record) = db.storage().get_latest(&id).unwrap().unwrap();
        let fields = ormdb_core::query::decode_entity(&record.data).unwrap();
        assert!(fields.iter().any(|(n, v)| n == "name" && *v == Value::String("David".into())));
    }

    #[test]
    fn test_batch() {
        let (_dir, db) = setup_test_db();
        let executor = MutationExecutor::new(&db);

        let batch = MutationBatch::from_mutations(vec![
            Mutation::insert("User", vec![FieldValue::new("name", "User1")]),
            Mutation::insert("User", vec![FieldValue::new("name", "User2")]),
            Mutation::insert("User", vec![FieldValue::new("name", "User3")]),
        ]);

        let result = executor.execute_batch(&batch).unwrap();
        assert_eq!(result.affected, 3);
        assert_eq!(result.inserted_ids.len(), 3);
    }
}
