//! Database wrapper combining StorageEngine and Catalog.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::{info, warn};

use ormdb_core::catalog::Catalog;
use ormdb_core::metrics::SharedMetricsRegistry;
use ormdb_core::query::{PlanCache, QueryExecutor, TableStatistics};
use ormdb_core::replication::ChangeLog;
use ormdb_core::storage::{
    ColumnarStore, CompactionEngine, CompactionResult, RetentionPolicy, StorageConfig,
    StorageEngine,
};

use crate::error::Error;

/// Database wrapper that provides access to storage and catalog.
pub struct Database {
    storage: Arc<StorageEngine>,
    catalog: Catalog,
    statistics: TableStatistics,
    plan_cache: PlanCache,
    columnar: ColumnarStore,
    changelog: ChangeLog,
    /// Keep the sled::Db handle alive for the catalog.
    _catalog_db: sled::Db,
    /// Retention policy for compaction.
    retention_policy: RetentionPolicy,
}

impl Database {
    /// Open a database at the given path.
    pub fn open(data_path: &Path) -> Result<Self, Error> {
        Self::open_with_retention(data_path, RetentionPolicy::default())
    }

    /// Open a database with a specific retention policy.
    pub fn open_with_retention(data_path: &Path, retention_policy: RetentionPolicy) -> Result<Self, Error> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(data_path).map_err(|e| {
            Error::Database(format!("failed to create data directory: {}", e))
        })?;

        // Open storage engine
        let storage_path = data_path.join("storage");
        let storage = Arc::new(StorageEngine::open(StorageConfig::new(&storage_path))
            .map_err(|e| Error::Database(format!("failed to open storage: {}", e)))?);

        // Open catalog database (separate sled instance)
        let catalog_path = data_path.join("catalog");
        let catalog_db = sled::open(&catalog_path)
            .map_err(|e| Error::Database(format!("failed to open catalog db: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| Error::Database(format!("failed to open catalog: {}", e)))?;

        // Open columnar store (uses storage sled Db)
        let columnar = ColumnarStore::open(storage.db())
            .map_err(|e| Error::Database(format!("failed to open columnar store: {}", e)))?;

        // Open changelog (uses storage sled Db)
        let changelog = ChangeLog::open(storage.db())
            .map_err(|e| Error::Database(format!("failed to open changelog: {}", e)))?;

        Ok(Self {
            storage,
            catalog,
            statistics: TableStatistics::new(),
            plan_cache: PlanCache::new(1000), // 1000 entry cache
            columnar,
            changelog,
            _catalog_db: catalog_db,
            retention_policy,
        })
    }

    /// Get a reference to the storage engine.
    pub fn storage(&self) -> &StorageEngine {
        &self.storage
    }

    /// Get an Arc reference to the storage engine.
    pub fn storage_arc(&self) -> Arc<StorageEngine> {
        self.storage.clone()
    }

    /// Get a reference to the catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Get a reference to the table statistics.
    pub fn statistics(&self) -> &TableStatistics {
        &self.statistics
    }

    /// Get a reference to the plan cache.
    pub fn plan_cache(&self) -> &PlanCache {
        &self.plan_cache
    }

    /// Get a reference to the columnar store.
    pub fn columnar(&self) -> &ColumnarStore {
        &self.columnar
    }

    /// Get a reference to the changelog.
    pub fn changelog(&self) -> &ChangeLog {
        &self.changelog
    }

    /// Create a query executor for this database.
    pub fn executor(&self) -> QueryExecutor<'_> {
        QueryExecutor::new(&self.storage, &self.catalog)
    }

    /// Create a query executor with metrics tracking.
    pub fn executor_with_metrics(&self, metrics: SharedMetricsRegistry) -> QueryExecutor<'_> {
        QueryExecutor::with_metrics(&self.storage, &self.catalog, metrics)
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

    /// Run a single compaction cycle manually.
    pub fn compact(&self) -> CompactionResult {
        let engine = CompactionEngine::new(self.storage.clone(), self.retention_policy.clone());
        engine.compact()
    }

    /// Create a compaction engine for this database.
    pub fn compaction_engine(&self) -> CompactionEngine {
        CompactionEngine::new(self.storage.clone(), self.retention_policy.clone())
    }

    /// Get the retention policy.
    pub fn retention_policy(&self) -> &RetentionPolicy {
        &self.retention_policy
    }
}

/// Handle for a background compaction task.
pub struct CompactionTask {
    handle: JoinHandle<()>,
    stop_flag: Arc<AtomicBool>,
}

impl CompactionTask {
    /// Start a background compaction task.
    pub fn start(database: Arc<Database>, interval: Duration) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let handle = tokio::spawn(async move {
            info!(interval_secs = interval.as_secs(), "Background compaction task started");

            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await; // Skip first immediate tick

            loop {
                ticker.tick().await;

                if stop_flag_clone.load(Ordering::SeqCst) {
                    info!("Background compaction task stopping");
                    break;
                }

                // Run compaction
                let result = database.compact();

                if result.did_cleanup() {
                    info!(
                        versions_removed = result.versions_removed,
                        tombstones_removed = result.tombstones_removed,
                        bytes_reclaimed = result.bytes_reclaimed,
                        duration_ms = result.duration.as_millis() as u64,
                        "Background compaction completed"
                    );
                }
            }
        });

        Self { handle, stop_flag }
    }

    /// Signal the compaction task to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Wait for the compaction task to finish.
    pub async fn join(self) {
        self.stop();
        if let Err(e) = self.handle.await {
            warn!(error = %e, "Compaction task panicked");
        }
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
