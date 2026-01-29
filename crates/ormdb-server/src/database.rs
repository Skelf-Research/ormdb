//! Database wrapper combining StorageEngine and Catalog.

use std::path::Path;
use std::sync::Arc;

use ormdb_core::catalog::Catalog;
use ormdb_core::query::QueryExecutor;
use ormdb_core::storage::{StorageConfig, StorageEngine};

use crate::error::Error;

/// Database wrapper that provides access to storage and catalog.
pub struct Database {
    storage: StorageEngine,
    catalog: Catalog,
    /// Keep the sled::Db handle alive for the catalog.
    _catalog_db: sled::Db,
}

impl Database {
    /// Open a database at the given path.
    pub fn open(data_path: &Path) -> Result<Self, Error> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(data_path).map_err(|e| {
            Error::Database(format!("failed to create data directory: {}", e))
        })?;

        // Open storage engine
        let storage_path = data_path.join("storage");
        let storage = StorageEngine::open(StorageConfig::new(&storage_path))
            .map_err(|e| Error::Database(format!("failed to open storage: {}", e)))?;

        // Open catalog database (separate sled instance)
        let catalog_path = data_path.join("catalog");
        let catalog_db = sled::open(&catalog_path)
            .map_err(|e| Error::Database(format!("failed to open catalog db: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| Error::Database(format!("failed to open catalog: {}", e)))?;

        Ok(Self {
            storage,
            catalog,
            _catalog_db: catalog_db,
        })
    }

    /// Get a reference to the storage engine.
    pub fn storage(&self) -> &StorageEngine {
        &self.storage
    }

    /// Get a reference to the catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Create a query executor for this database.
    pub fn executor(&self) -> QueryExecutor<'_> {
        QueryExecutor::new(&self.storage, &self.catalog)
    }

    /// Get the current schema version.
    pub fn schema_version(&self) -> u64 {
        self.catalog.current_version()
    }

    /// Flush all pending writes to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.storage
            .flush()
            .map_err(|e| Error::Database(format!("failed to flush storage: {}", e)))
    }
}

/// Thread-safe database handle.
pub type SharedDatabase = Arc<Database>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_open() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        // Should have version 0 (no schema applied yet)
        assert_eq!(db.schema_version(), 0);
    }

    #[test]
    fn test_database_with_schema() {
        use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};

        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        // Apply a schema
        let schema = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String))),
        );

        db.catalog().apply_schema(schema).unwrap();

        // Version should now be 1
        assert_eq!(db.schema_version(), 1);

        // Should be able to get entity definition
        let user_def = db.catalog().get_entity("User").unwrap();
        assert!(user_def.is_some());
    }

    #[test]
    fn test_database_persistence() {
        use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};

        let dir = tempfile::tempdir().unwrap();

        // Open and create schema
        {
            let db = Database::open(dir.path()).unwrap();
            let schema = SchemaBundle::new(1).with_entity(
                EntityDef::new("Item", "id")
                    .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid))),
            );
            db.catalog().apply_schema(schema).unwrap();
            db.flush().unwrap();
        }

        // Reopen and verify
        {
            let db = Database::open(dir.path()).unwrap();
            assert_eq!(db.schema_version(), 1);
            let item_def = db.catalog().get_entity("Item").unwrap();
            assert!(item_def.is_some());
        }
    }
}
