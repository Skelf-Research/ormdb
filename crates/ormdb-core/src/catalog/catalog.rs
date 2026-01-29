//! Catalog manager for storing and retrieving schema metadata.

use super::{ConstraintDef, EntityDef, RelationDef, SchemaBundle};
use crate::error::Error;
use sled::{Db, Tree};
use std::sync::atomic::{AtomicU64, Ordering};

/// Tree name for schema bundles.
const SCHEMA_TREE: &str = "catalog:schemas";

/// Tree name for catalog metadata.
const META_TREE: &str = "catalog:meta";

/// Key for current schema version in meta tree.
const CURRENT_VERSION_KEY: &[u8] = b"current_version";

/// The catalog manager for schema metadata.
pub struct Catalog {
    /// Schema bundles tree.
    schema_tree: Tree,
    /// Metadata tree.
    meta_tree: Tree,
    /// Current schema version (cached).
    current_version: AtomicU64,
    /// Current schema (cached).
    current_schema: std::sync::RwLock<Option<SchemaBundle>>,
}

impl Catalog {
    /// Open or create a catalog using the given sled database.
    pub fn open(db: &Db) -> Result<Self, Error> {
        let schema_tree = db.open_tree(SCHEMA_TREE)?;
        let meta_tree = db.open_tree(META_TREE)?;

        // Load current version from metadata
        let current_version = match meta_tree.get(CURRENT_VERSION_KEY)? {
            Some(bytes) => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&bytes);
                u64::from_be_bytes(buf)
            }
            None => 0,
        };

        let catalog = Self {
            schema_tree,
            meta_tree,
            current_version: AtomicU64::new(current_version),
            current_schema: std::sync::RwLock::new(None),
        };

        // Pre-load current schema if exists
        if current_version > 0 {
            if let Some(schema) = catalog.schema_at_version(current_version)? {
                *catalog.current_schema.write().unwrap() = Some(schema);
            }
        }

        Ok(catalog)
    }

    /// Get the current schema version.
    pub fn current_version(&self) -> u64 {
        self.current_version.load(Ordering::SeqCst)
    }

    /// Get the current schema bundle.
    pub fn current_schema(&self) -> Result<Option<SchemaBundle>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard.clone())
    }

    /// Get a schema bundle at a specific version.
    pub fn schema_at_version(&self, version: u64) -> Result<Option<SchemaBundle>, Error> {
        let key = version.to_be_bytes();
        match self.schema_tree.get(key)? {
            Some(bytes) => {
                let schema = SchemaBundle::from_bytes(&bytes)?;
                Ok(Some(schema))
            }
            None => Ok(None),
        }
    }

    /// Apply a new schema bundle.
    ///
    /// The bundle's version must be greater than the current version.
    /// Returns the new version number.
    pub fn apply_schema(&self, mut bundle: SchemaBundle) -> Result<u64, Error> {
        let current = self.current_version();
        let new_version = current + 1;

        // Ensure version is set correctly
        bundle.version = new_version;

        // Serialize and store
        let key = new_version.to_be_bytes();
        let value = bundle.to_bytes()?;
        self.schema_tree.insert(key, value)?;

        // Update current version in metadata
        self.meta_tree
            .insert(CURRENT_VERSION_KEY, &new_version.to_be_bytes())?;

        // Update cached values
        self.current_version.store(new_version, Ordering::SeqCst);
        *self.current_schema.write().unwrap() = Some(bundle);

        Ok(new_version)
    }

    /// Get an entity definition by name from the current schema.
    pub fn get_entity(&self, name: &str) -> Result<Option<EntityDef>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard.as_ref().and_then(|s| s.get_entity(name).cloned()))
    }

    /// List all entity names in the current schema.
    pub fn list_entities(&self) -> Result<Vec<String>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard
            .as_ref()
            .map(|s| s.entity_names().into_iter().map(String::from).collect())
            .unwrap_or_default())
    }

    /// Get a relation definition by name from the current schema.
    pub fn get_relation(&self, name: &str) -> Result<Option<RelationDef>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard.as_ref().and_then(|s| s.get_relation(name).cloned()))
    }

    /// Get all relations where the given entity is the source.
    pub fn relations_from(&self, entity: &str) -> Result<Vec<RelationDef>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard
            .as_ref()
            .map(|s| s.relations_from(entity).into_iter().cloned().collect())
            .unwrap_or_default())
    }

    /// Get all relations where the given entity is the target.
    pub fn relations_to(&self, entity: &str) -> Result<Vec<RelationDef>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard
            .as_ref()
            .map(|s| s.relations_to(entity).into_iter().cloned().collect())
            .unwrap_or_default())
    }

    /// Get all constraints for an entity.
    pub fn constraints_for(&self, entity: &str) -> Result<Vec<ConstraintDef>, Error> {
        let guard = self.current_schema.read().unwrap();
        Ok(guard
            .as_ref()
            .map(|s| s.constraints_for(entity).into_iter().cloned().collect())
            .unwrap_or_default())
    }

    /// List all schema versions.
    pub fn list_versions(&self) -> Result<Vec<u64>, Error> {
        let mut versions = Vec::new();
        for result in self.schema_tree.iter() {
            let (key, _) = result?;
            if key.len() == 8 {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&key);
                versions.push(u64::from_be_bytes(buf));
            }
        }
        versions.sort();
        Ok(versions)
    }

    /// Flush pending writes to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.schema_tree.flush()?;
        self.meta_tree.flush()?;
        Ok(())
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

        SchemaBundle::new(0) // Version will be set by apply_schema
            .with_entity(user)
            .with_entity(post)
            .with_relation(relation)
            .with_constraint(unique)
    }

    fn test_db() -> sled::Db {
        sled::Config::new().temporary(true).open().unwrap()
    }

    #[test]
    fn test_catalog_open_empty() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();

        assert_eq!(catalog.current_version(), 0);
        assert!(catalog.current_schema().unwrap().is_none());
    }

    #[test]
    fn test_apply_schema() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();

        let schema = sample_schema();
        let version = catalog.apply_schema(schema).unwrap();

        assert_eq!(version, 1);
        assert_eq!(catalog.current_version(), 1);
        assert!(catalog.current_schema().unwrap().is_some());
    }

    #[test]
    fn test_get_entity() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();
        catalog.apply_schema(sample_schema()).unwrap();

        let user = catalog.get_entity("User").unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().name, "User");

        let nonexistent = catalog.get_entity("NonExistent").unwrap();
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_list_entities() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();
        catalog.apply_schema(sample_schema()).unwrap();

        let entities = catalog.list_entities().unwrap();
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&"User".to_string()));
        assert!(entities.contains(&"Post".to_string()));
    }

    #[test]
    fn test_get_relation() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();
        catalog.apply_schema(sample_schema()).unwrap();

        let relation = catalog.get_relation("user_posts").unwrap();
        assert!(relation.is_some());
    }

    #[test]
    fn test_relations_for_entity() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();
        catalog.apply_schema(sample_schema()).unwrap();

        let from_post = catalog.relations_from("Post").unwrap();
        assert_eq!(from_post.len(), 1);

        let to_user = catalog.relations_to("User").unwrap();
        assert_eq!(to_user.len(), 1);
    }

    #[test]
    fn test_constraints_for_entity() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();
        catalog.apply_schema(sample_schema()).unwrap();

        let constraints = catalog.constraints_for("User").unwrap();
        assert_eq!(constraints.len(), 1);
    }

    #[test]
    fn test_schema_versioning() {
        let db = test_db();
        let catalog = Catalog::open(&db).unwrap();

        // Apply first schema
        let schema1 = sample_schema();
        let v1 = catalog.apply_schema(schema1).unwrap();
        assert_eq!(v1, 1);

        // Apply second schema (with additional entity)
        let schema2 = sample_schema().with_entity(
            EntityDef::new("Comment", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid))),
        );
        let v2 = catalog.apply_schema(schema2).unwrap();
        assert_eq!(v2, 2);

        // Verify we can retrieve both versions
        let retrieved_v1 = catalog.schema_at_version(1).unwrap().unwrap();
        assert_eq!(retrieved_v1.entities.len(), 2);

        let retrieved_v2 = catalog.schema_at_version(2).unwrap().unwrap();
        assert_eq!(retrieved_v2.entities.len(), 3);

        // List versions
        let versions = catalog.list_versions().unwrap();
        assert_eq!(versions, vec![1, 2]);
    }

    #[test]
    fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let config = sled::Config::new().path(dir.path());

        // Create and populate catalog
        {
            let db = config.clone().open().unwrap();
            let catalog = Catalog::open(&db).unwrap();
            catalog.apply_schema(sample_schema()).unwrap();
            catalog.flush().unwrap();
        }

        // Reopen and verify
        {
            let db = config.open().unwrap();
            let catalog = Catalog::open(&db).unwrap();

            assert_eq!(catalog.current_version(), 1);
            let schema = catalog.current_schema().unwrap().unwrap();
            assert_eq!(schema.entities.len(), 2);
        }
    }
}
