use crate::error::{Result, StudioError};
use ormdb_core::{Catalog, StorageConfig, StorageEngine};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// A per-session embedded database
pub struct SessionDatabase {
    storage: Arc<StorageEngine>,
    catalog: Arc<Catalog>,
    _catalog_db: sled::Db,
    _temp_dir: Option<TempDir>,
}

impl SessionDatabase {
    /// Create a new temporary embedded database
    pub fn new_temporary() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().to_path_buf();
        Self::open_at(&path, Some(temp_dir))
    }

    /// Create a database at a specific path
    pub fn open_at(path: &Path, temp_dir: Option<TempDir>) -> Result<Self> {
        let storage_path = path.join("storage");
        let catalog_path = path.join("catalog");

        // Create directories
        std::fs::create_dir_all(&storage_path)?;
        std::fs::create_dir_all(&catalog_path)?;

        // Open storage engine
        let config = StorageConfig::new(&storage_path);
        let storage = StorageEngine::open(config)
            .map_err(|e| StudioError::Database(e.to_string()))?;

        // Open catalog (separate sled database)
        let catalog_db = sled::open(&catalog_path)
            .map_err(|e| StudioError::Database(format!("failed to open catalog db: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| StudioError::Database(e.to_string()))?;

        Ok(Self {
            storage: Arc::new(storage),
            catalog: Arc::new(catalog),
            _catalog_db: catalog_db,
            _temp_dir: temp_dir,
        })
    }

    /// Get the catalog for schema operations
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Get the storage engine
    pub fn storage(&self) -> &StorageEngine {
        &self.storage
    }
}
