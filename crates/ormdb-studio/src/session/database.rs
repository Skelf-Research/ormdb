use crate::error::{Result, StudioError};
use ormdb_core::storage::{CompactionEngine, RetentionPolicy};
use ormdb_core::{Catalog, SharedMetricsRegistry, StorageConfig, StorageEngine, new_shared_registry};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::task::JoinHandle;

/// A per-session embedded database with automatic compaction
pub struct SessionDatabase {
    storage: Arc<StorageEngine>,
    catalog: Arc<Catalog>,
    metrics: SharedMetricsRegistry,
    _catalog_db: sled::Db,
    _temp_dir: Option<TempDir>,
    /// Handle to the compaction background task
    compaction_task: Option<JoinHandle<()>>,
    /// Flag to signal compaction task to stop
    stop_compaction: Arc<AtomicBool>,
}

/// Studio session retention policy configuration
const SESSION_TTL_SECS: u64 = 3600;           // Keep data for 1 hour max
const SESSION_MAX_VERSIONS: usize = 10;        // Keep at most 10 versions per entity
const SESSION_MIN_AGE_SECS: u64 = 300;         // Minimum 5 minutes before cleanup
const COMPACTION_INTERVAL_SECS: u64 = 300;     // Run compaction every 5 minutes

impl SessionDatabase {
    /// Create a new temporary embedded database with automatic compaction
    pub fn new_temporary() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().to_path_buf();
        Self::open_at(&path, Some(temp_dir))
    }

    /// Create a database at a specific path with studio-optimized settings
    pub fn open_at(path: &Path, temp_dir: Option<TempDir>) -> Result<Self> {
        let storage_path = path.join("storage");
        let catalog_path = path.join("catalog");

        // Create directories
        std::fs::create_dir_all(&storage_path)?;
        std::fs::create_dir_all(&catalog_path)?;

        // Configure retention policy for studio sessions:
        // - Keep data for at most 1 hour (TTL)
        // - Keep at most 10 versions per entity
        // - Don't clean versions younger than 5 minutes
        // - Clean up tombstones after retention period
        let retention = RetentionPolicy::default()
            .ttl(Duration::from_secs(SESSION_TTL_SECS))
            .max_versions(SESSION_MAX_VERSIONS)
            .min_age(Duration::from_secs(SESSION_MIN_AGE_SECS))
            .cleanup_tombstones(true);

        // Open storage engine with studio-optimized configuration
        let config = StorageConfig::new(&storage_path)
            .with_retention(retention.clone())
            .with_compaction_interval(Duration::from_secs(COMPACTION_INTERVAL_SECS));

        let storage = StorageEngine::open(config)
            .map_err(|e| StudioError::Database(e.to_string()))?;

        let storage = Arc::new(storage);

        // Open catalog (separate sled database)
        let catalog_db = sled::open(&catalog_path)
            .map_err(|e| StudioError::Database(format!("failed to open catalog db: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| StudioError::Database(e.to_string()))?;

        // Set up automatic compaction background task
        let stop_flag = Arc::new(AtomicBool::new(false));
        let compaction_task = Self::start_compaction_task(
            storage.clone(),
            retention,
            stop_flag.clone(),
        );

        Ok(Self {
            storage,
            catalog: Arc::new(catalog),
            metrics: new_shared_registry(),
            _catalog_db: catalog_db,
            _temp_dir: temp_dir,
            compaction_task: Some(compaction_task),
            stop_compaction: stop_flag,
        })
    }

    /// Start the background compaction task
    fn start_compaction_task(
        storage: Arc<StorageEngine>,
        retention: RetentionPolicy,
        stop_flag: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let compactor = CompactionEngine::new(storage, retention);
            let interval = Duration::from_secs(COMPACTION_INTERVAL_SECS);

            loop {
                tokio::time::sleep(interval).await;

                if stop_flag.load(Ordering::Relaxed) {
                    tracing::debug!("Compaction task stopping");
                    break;
                }

                // Run compaction
                let result = compactor.compact();

                if result.did_cleanup() {
                    tracing::info!(
                        versions_removed = result.versions_removed,
                        tombstones_removed = result.tombstones_removed,
                        bytes_reclaimed = result.bytes_reclaimed,
                        duration_ms = result.duration.as_millis() as u64,
                        "Session compaction completed"
                    );
                }

                // Trigger sled's internal maintenance
                if let Err(e) = compactor.compact_sled() {
                    tracing::warn!(error = %e, "Failed to flush storage after compaction");
                }
            }
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

    /// Get the metrics registry
    pub fn metrics(&self) -> &SharedMetricsRegistry {
        &self.metrics
    }

    /// Manually trigger compaction
    pub fn compact(&self) -> ormdb_core::storage::CompactionResult {
        let retention = RetentionPolicy::default()
            .ttl(Duration::from_secs(SESSION_TTL_SECS))
            .max_versions(SESSION_MAX_VERSIONS)
            .min_age(Duration::from_secs(SESSION_MIN_AGE_SECS))
            .cleanup_tombstones(true);

        let compactor = CompactionEngine::new(self.storage.clone(), retention);
        let result = compactor.compact();

        // Also flush storage
        let _ = compactor.compact_sled();

        result
    }
}

impl Drop for SessionDatabase {
    fn drop(&mut self) {
        // Signal the compaction task to stop
        self.stop_compaction.store(true, Ordering::Relaxed);

        // Note: We don't wait for the task to finish here since Drop can't be async.
        // The task will stop on its next iteration.
        tracing::debug!("SessionDatabase dropped, compaction task signaled to stop");
    }
}
