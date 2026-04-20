//! Database handle for embedded ORMDB.
//!
//! The `Database` struct is the main entry point for interacting with an embedded
//! ORMDB database. It is thread-safe and can be cheaply cloned (using `Arc` internally).
//!
//! # Example
//!
//! ```rust,no_run
//! use ormdb::Database;
//!
//! // Open a database at a path
//! let db = Database::open("./my_data").unwrap();
//!
//! // Or open an in-memory database
//! let db = Database::open_memory().unwrap();
//!
//! // Insert data
//! let user_id = db.insert("User")
//!     .set("name", "Alice")
//!     .execute()
//!     .unwrap();
//!
//! // Query data
//! let users = db.query("User")
//!     .filter("name", "Alice")
//!     .execute()
//!     .unwrap();
//! ```

use std::path::Path;
use std::sync::Arc;

use tracing::warn;

use ormdb_core::catalog::Catalog;
use ormdb_core::query::{PlanCache, QueryExecutor, TableStatistics};
use ormdb_core::storage::{
    ColumnarStore, CompactionEngine, CompactionResult, RetentionPolicy, StorageConfig,
    StorageEngine,
};

use crate::error::{Error, Result};
use crate::mutation::{Delete, Insert, Update};
use crate::query::Query;
use crate::schema::SchemaBuilder;
use crate::transaction::Transaction;

/// Configuration for opening a database.
#[derive(Debug, Clone)]
pub struct Config {
    /// Cache capacity in bytes.
    pub cache_capacity: u64,
    /// Flush interval in milliseconds. None means flush on every write.
    pub flush_every_ms: Option<u64>,
    /// Enable compression.
    pub compression: bool,
    /// Retention policy for MVCC garbage collection.
    pub retention: RetentionPolicy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_capacity: 1024 * 1024 * 1024, // 1GB
            flush_every_ms: Some(1000),
            compression: true,
            retention: RetentionPolicy::default(),
        }
    }
}

/// Internal database state.
struct DatabaseInner {
    storage: Arc<StorageEngine>,
    catalog: Catalog,
    statistics: TableStatistics,
    plan_cache: PlanCache,
    columnar: ColumnarStore,
    retention_policy: RetentionPolicy,
    _catalog_db: sled::Db,
}

/// A thread-safe handle to an embedded ORMDB database.
///
/// This is the main entry point for interacting with the database. The handle
/// can be cheaply cloned and shared across threads.
///
/// # Multi-Writer Support
///
/// Unlike SQLite, ORMDB supports concurrent writes from multiple threads.
/// Writes use optimistic concurrency control - if two transactions modify
/// the same entity, one will fail with `Error::TransactionConflict`.
///
/// # Example
///
/// ```rust,no_run
/// use ormdb::Database;
/// use std::thread;
///
/// let db = Database::open("./data").unwrap();
///
/// // Clone is cheap - uses Arc internally
/// let db1 = db.clone();
/// let db2 = db.clone();
///
/// // Multiple threads can write concurrently
/// let h1 = thread::spawn(move || {
///     db1.insert("User").set("name", "Alice").execute()
/// });
/// let h2 = thread::spawn(move || {
///     db2.insert("User").set("name", "Bob").execute()
/// });
///
/// h1.join().unwrap().unwrap();
/// h2.join().unwrap().unwrap();
/// ```
#[derive(Clone)]
pub struct Database {
    inner: Arc<DatabaseInner>,
}

impl Database {
    /// Open a database at the given path.
    ///
    /// Creates the database directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or created.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_config(path, Config::default())
    }

    /// Open a temporary in-memory database.
    ///
    /// The database is destroyed when the `Database` handle is dropped.
    /// Useful for testing and temporary workloads.
    pub fn open_memory() -> Result<Self> {
        let storage_config = StorageConfig::temporary();
        let storage = Arc::new(StorageEngine::open(storage_config)?);

        let catalog_db = sled::Config::new()
            .temporary(true)
            .open()
            .map_err(|e| Error::Database(format!("failed to open catalog: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| Error::Database(format!("failed to open catalog: {}", e)))?;

        let columnar = ColumnarStore::open(storage.db())
            .map_err(|e| Error::Database(format!("failed to open columnar store: {}", e)))?;

        Ok(Self {
            inner: Arc::new(DatabaseInner {
                storage,
                catalog,
                statistics: TableStatistics::new(),
                plan_cache: PlanCache::new(1000),
                columnar,
                retention_policy: RetentionPolicy::default(),
                _catalog_db: catalog_db,
            }),
        })
    }

    /// Open a database with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    /// * `config` - Configuration options
    pub fn open_with_config(path: impl AsRef<Path>, config: Config) -> Result<Self> {
        let data_path = path.as_ref();

        // Create data directory if it doesn't exist
        std::fs::create_dir_all(data_path)
            .map_err(|e| Error::Database(format!("failed to create data directory: {}", e)))?;

        // Open storage engine
        let storage_path = data_path.join("storage");
        let mut storage_config = StorageConfig::new(&storage_path);
        storage_config.cache_capacity = config.cache_capacity;
        storage_config.flush_every_ms = config.flush_every_ms;
        storage_config.compression = config.compression;

        let storage = Arc::new(
            StorageEngine::open(storage_config)
                .map_err(|e| Error::Database(format!("failed to open storage: {}", e)))?,
        );

        // Open catalog database (separate sled instance)
        let catalog_path = data_path.join("catalog");
        let catalog_db = sled::open(&catalog_path)
            .map_err(|e| Error::Database(format!("failed to open catalog db: {}", e)))?;

        let catalog = Catalog::open(&catalog_db)
            .map_err(|e| Error::Database(format!("failed to open catalog: {}", e)))?;

        // Open columnar store
        let columnar = ColumnarStore::open(storage.db())
            .map_err(|e| Error::Database(format!("failed to open columnar store: {}", e)))?;

        let db = Self {
            inner: Arc::new(DatabaseInner {
                storage,
                catalog,
                statistics: TableStatistics::new(),
                plan_cache: PlanCache::new(1000),
                columnar,
                retention_policy: config.retention,
                _catalog_db: catalog_db,
            }),
        };

        // Refresh statistics on startup
        if let Err(e) = db.inner.statistics.refresh(&db.inner.storage, &db.inner.catalog) {
            warn!(error = %e, "Failed to refresh statistics on startup");
        }

        Ok(db)
    }

    // =========================================================================
    // Schema Operations
    // =========================================================================

    /// Get a schema builder for defining entities and relations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// db.schema()
    ///     .entity("User")
    ///         .field("id", ormdb::ScalarType::Uuid).primary_key()
    ///         .field("name", ormdb::ScalarType::String)
    ///         .field("email", ormdb::ScalarType::String).unique()
    ///     .entity("Post")
    ///         .field("id", ormdb::ScalarType::Uuid).primary_key()
    ///         .field("title", ormdb::ScalarType::String)
    ///         .relation("author", "User").required()
    ///     .apply()
    ///     .unwrap();
    /// ```
    pub fn schema(&self) -> SchemaBuilder<'_> {
        SchemaBuilder::new(self)
    }

    /// Get the current schema version.
    pub fn schema_version(&self) -> u64 {
        self.inner.catalog.current_version()
    }

    // =========================================================================
    // Query Operations
    // =========================================================================

    /// Start building a query for an entity type.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity type name to query
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// let users = db.query("User")
    ///     .filter("status", "active")
    ///     .order_by("name")
    ///     .limit(10)
    ///     .execute()
    ///     .unwrap();
    /// ```
    pub fn query(&self, entity: &str) -> Query<'_> {
        Query::new(self, entity)
    }

    /// Execute a raw GraphQuery directly.
    ///
    /// This is a lower-level API for advanced use cases.
    pub fn execute_raw(&self, query: &ormdb_proto::GraphQuery) -> Result<ormdb_proto::QueryResult> {
        let executor = QueryExecutor::new(&self.inner.storage, &self.inner.catalog);
        executor.execute(query).map_err(Into::into)
    }

    // =========================================================================
    // Mutation Operations
    // =========================================================================

    /// Start building an insert operation for an entity type.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity type name to insert into
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// let user_id = db.insert("User")
    ///     .set("name", "Alice")
    ///     .set("email", "alice@example.com")
    ///     .execute()
    ///     .unwrap();
    /// ```
    pub fn insert(&self, entity: &str) -> Insert<'_> {
        Insert::new(self, entity)
    }

    /// Start building an update operation for an entity.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity type name
    /// * `id` - The entity ID to update
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// db.update("User", user_id)
    ///     .set("name", "Bob")
    ///     .execute()
    ///     .unwrap();
    /// ```
    pub fn update(&self, entity: &str, id: [u8; 16]) -> Update<'_> {
        Update::new(self, entity, id)
    }

    /// Start building a delete operation for an entity.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity type name
    /// * `id` - The entity ID to delete
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// db.delete("User", user_id)
    ///     .execute()
    ///     .unwrap();
    /// ```
    pub fn delete(&self, entity: &str, id: [u8; 16]) -> Delete<'_> {
        Delete::new(self, entity, id)
    }

    // =========================================================================
    // Transaction Operations
    // =========================================================================

    /// Begin a new transaction.
    ///
    /// Transactions provide atomic operations with optimistic concurrency control.
    /// Multiple transactions can run concurrently, but if they modify the same
    /// entity, one will fail with `Error::TransactionConflict` at commit time.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// let mut tx = db.transaction();
    /// tx.insert("User").set("name", "Alice").execute();
    /// tx.insert("User").set("name", "Bob").execute();
    /// tx.commit().unwrap();
    /// ```
    pub fn transaction(&self) -> Transaction<'_> {
        Transaction::new(self)
    }

    /// Execute a closure within a transaction.
    ///
    /// The transaction is automatically committed if the closure returns `Ok`,
    /// or rolled back if it returns `Err`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ormdb::Database;
    ///
    /// let db = Database::open_memory().unwrap();
    ///
    /// db.with_transaction(|tx| {
    ///     tx.insert("User").set("name", "Alice").execute()?;
    ///     tx.insert("User").set("name", "Bob").execute()?;
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Transaction) -> Result<T>,
    {
        let mut tx = self.transaction();
        let result = f(&mut tx)?;
        tx.commit()?;
        Ok(result)
    }

    // =========================================================================
    // Maintenance Operations
    // =========================================================================

    /// Flush all pending writes to disk.
    ///
    /// This ensures data durability. Normally, the database flushes automatically
    /// based on the configured flush interval.
    pub fn flush(&self) -> Result<()> {
        self.inner
            .storage
            .flush()
            .map_err(|e| Error::Database(format!("failed to flush: {}", e)))
    }

    /// Run compaction to reclaim space from old MVCC versions.
    ///
    /// This removes old versions based on the retention policy and returns
    /// statistics about the compaction.
    pub fn compact(&self) -> CompactionResult {
        let engine =
            CompactionEngine::new(self.inner.storage.clone(), self.inner.retention_policy.clone());
        let result = engine.compact();
        if result.did_cleanup() {
            if let Err(e) = engine.compact_sled() {
                warn!(error = %e, "Failed to run sled compaction");
            }
        }
        result
    }

    /// Refresh table statistics.
    ///
    /// This updates the query planner's statistics for better query optimization.
    pub fn refresh_statistics(&self) -> Result<()> {
        self.inner
            .statistics
            .refresh(&self.inner.storage, &self.inner.catalog)
            .map_err(|e| Error::Database(format!("failed to refresh statistics: {}", e)))
    }

    // =========================================================================
    // Internal Accessors (for other modules)
    // =========================================================================

    pub(crate) fn storage(&self) -> &StorageEngine {
        &self.inner.storage
    }

    pub(crate) fn catalog(&self) -> &Catalog {
        &self.inner.catalog
    }

    pub(crate) fn statistics(&self) -> &TableStatistics {
        &self.inner.statistics
    }

    pub(crate) fn plan_cache(&self) -> &PlanCache {
        &self.inner.plan_cache
    }

    pub(crate) fn columnar(&self) -> &ColumnarStore {
        &self.inner.columnar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.schema_version(), 0);
    }

    #[test]
    fn test_open_path() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        assert_eq!(db.schema_version(), 0);
        db.flush().unwrap();
    }

    #[test]
    fn test_clone_is_cheap() {
        let db = Database::open_memory().unwrap();
        let db2 = db.clone();

        // Both point to the same underlying data
        assert_eq!(db.schema_version(), db2.schema_version());
    }
}
