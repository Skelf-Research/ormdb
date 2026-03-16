//! Statistics collection for query planning.
//!
//! This module tracks entity counts and other statistics used by the cost model
//! to make informed decisions about query execution strategies.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::catalog::Catalog;
use crate::error::Error;
use crate::storage::StorageEngine;

/// Statistics for query planning.
///
/// Tracks row counts per entity type, with real-time updates on mutations.
/// Thread-safe for concurrent access.
pub struct TableStatistics {
    /// Row count per entity type.
    entity_counts: RwLock<HashMap<String, AtomicU64>>,
    /// Last refresh timestamp (microseconds since Unix epoch).
    last_refresh: AtomicU64,
}

impl TableStatistics {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            entity_counts: RwLock::new(HashMap::new()),
            last_refresh: AtomicU64::new(0),
        }
    }

    /// Refresh statistics by scanning the storage engine.
    ///
    /// This should be called:
    /// - On startup to initialize counts
    /// - Periodically to correct any drift
    /// - After schema changes
    pub fn refresh(&self, storage: &StorageEngine, catalog: &Catalog) -> Result<(), Error> {
        let entity_names = catalog.list_entities()?;
        let mut counts = HashMap::new();

        for entity_name in entity_names {
            let mut count = 0u64;
            for result in storage.scan_entity_type(&entity_name) {
                result?; // Check for errors
                count += 1;
            }
            counts.insert(entity_name, AtomicU64::new(count));
        }

        // Update the counts
        let mut guard = self.entity_counts.write().map_err(|_| {
            Error::InvalidData("Failed to acquire statistics write lock".to_string())
        })?;
        *guard = counts;

        // Update refresh timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        self.last_refresh.store(now, AtomicOrdering::SeqCst);

        Ok(())
    }

    /// Get the estimated row count for an entity type.
    ///
    /// Returns 0 if the entity type is not tracked.
    pub fn entity_count(&self, entity_type: &str) -> u64 {
        let guard = match self.entity_counts.read() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        guard
            .get(entity_type)
            .map(|c| c.load(AtomicOrdering::Relaxed))
            .unwrap_or(0)
    }

    /// Increment the count for an entity type.
    ///
    /// Call this after inserting an entity.
    pub fn increment(&self, entity_type: &str) {
        // Try to increment existing counter
        if let Ok(guard) = self.entity_counts.read() {
            if let Some(counter) = guard.get(entity_type) {
                counter.fetch_add(1, AtomicOrdering::Relaxed);
                return;
            }
        }

        // Need to add new counter
        if let Ok(mut guard) = self.entity_counts.write() {
            guard
                .entry(entity_type.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    /// Decrement the count for an entity type.
    ///
    /// Call this after deleting an entity.
    pub fn decrement(&self, entity_type: &str) {
        if let Ok(guard) = self.entity_counts.read() {
            if let Some(counter) = guard.get(entity_type) {
                // Use saturating subtraction to avoid underflow
                let current = counter.load(AtomicOrdering::Relaxed);
                if current > 0 {
                    counter.fetch_sub(1, AtomicOrdering::Relaxed);
                }
            }
        }
    }

    /// Set the count for an entity type directly.
    ///
    /// Useful for bulk operations or corrections.
    pub fn set_count(&self, entity_type: &str, count: u64) {
        if let Ok(guard) = self.entity_counts.read() {
            if let Some(counter) = guard.get(entity_type) {
                counter.store(count, AtomicOrdering::Relaxed);
                return;
            }
        }

        if let Ok(mut guard) = self.entity_counts.write() {
            guard.insert(entity_type.to_string(), AtomicU64::new(count));
        }
    }

    /// Get the timestamp of the last refresh (microseconds since Unix epoch).
    pub fn last_refresh_time(&self) -> u64 {
        self.last_refresh.load(AtomicOrdering::SeqCst)
    }

    /// Check if statistics are stale (older than threshold).
    ///
    /// # Arguments
    /// * `threshold_ms` - Staleness threshold in milliseconds
    pub fn is_stale(&self, threshold_ms: u64) -> bool {
        let last = self.last_refresh.load(AtomicOrdering::SeqCst);
        if last == 0 {
            return true; // Never refreshed
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let elapsed_ms = (now - last) / 1000;
        elapsed_ms > threshold_ms
    }

    /// Get all entity counts as a snapshot.
    pub fn snapshot(&self) -> HashMap<String, u64> {
        let guard = match self.entity_counts.read() {
            Ok(g) => g,
            Err(_) => return HashMap::new(),
        };

        guard
            .iter()
            .map(|(k, v)| (k.clone(), v.load(AtomicOrdering::Relaxed)))
            .collect()
    }

    /// Get total entities across all types.
    pub fn total_entities(&self) -> u64 {
        let guard = match self.entity_counts.read() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        guard
            .values()
            .map(|v| v.load(AtomicOrdering::Relaxed))
            .sum()
    }
}

impl Default for TableStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
    use crate::storage::{Record, StorageConfig, VersionedKey};
    use crate::query::value_codec::encode_entity;
    use ormdb_proto::Value;

    fn setup_test_db() -> (StorageEngine, Catalog, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();

        let catalog_db = sled::Config::new().temporary(true).open().unwrap();
        let catalog = Catalog::open(&catalog_db).unwrap();

        // Create schema with User entity
        let user = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)));

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)));

        let schema = SchemaBundle::new(1)
            .with_entity(user)
            .with_entity(post);

        catalog.apply_schema(schema).unwrap();

        (storage, catalog, dir)
    }

    fn insert_entity(storage: &StorageEngine, entity_type: &str, id: [u8; 16]) {
        let fields = vec![
            ("id".to_string(), Value::Uuid(id)),
            ("name".to_string(), Value::String("test".to_string())),
        ];
        let data = encode_entity(&fields).unwrap();
        let key = VersionedKey::now(id);
        storage.put_typed(entity_type, key, Record::new(data)).unwrap();
    }

    #[test]
    fn test_new_statistics() {
        let stats = TableStatistics::new();
        assert_eq!(stats.entity_count("User"), 0);
        assert_eq!(stats.total_entities(), 0);
    }

    #[test]
    fn test_increment_decrement() {
        let stats = TableStatistics::new();

        stats.increment("User");
        assert_eq!(stats.entity_count("User"), 1);

        stats.increment("User");
        stats.increment("User");
        assert_eq!(stats.entity_count("User"), 3);

        stats.decrement("User");
        assert_eq!(stats.entity_count("User"), 2);
    }

    #[test]
    fn test_decrement_at_zero() {
        let stats = TableStatistics::new();

        // Decrement on non-existent type should not panic
        stats.decrement("User");
        assert_eq!(stats.entity_count("User"), 0);

        // Set to 1, decrement twice
        stats.set_count("User", 1);
        stats.decrement("User");
        stats.decrement("User"); // Should stay at 0
        assert_eq!(stats.entity_count("User"), 0);
    }

    #[test]
    fn test_set_count() {
        let stats = TableStatistics::new();

        stats.set_count("User", 100);
        assert_eq!(stats.entity_count("User"), 100);

        stats.set_count("User", 50);
        assert_eq!(stats.entity_count("User"), 50);
    }

    #[test]
    fn test_refresh_from_storage() {
        let (storage, catalog, _dir) = setup_test_db();

        // Insert some users
        for _ in 0..5 {
            insert_entity(&storage, "User", StorageEngine::generate_id());
        }
        // Insert some posts
        for _ in 0..3 {
            insert_entity(&storage, "Post", StorageEngine::generate_id());
        }

        storage.flush().unwrap();

        let stats = TableStatistics::new();
        stats.refresh(&storage, &catalog).unwrap();

        assert_eq!(stats.entity_count("User"), 5);
        assert_eq!(stats.entity_count("Post"), 3);
        assert_eq!(stats.total_entities(), 8);
    }

    #[test]
    fn test_snapshot() {
        let stats = TableStatistics::new();

        stats.set_count("User", 10);
        stats.set_count("Post", 20);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.get("User"), Some(&10));
        assert_eq!(snapshot.get("Post"), Some(&20));
    }

    #[test]
    fn test_is_stale() {
        let stats = TableStatistics::new();

        // Never refreshed, should be stale
        assert!(stats.is_stale(1000));

        // Simulate a refresh by setting last_refresh
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        stats.last_refresh.store(now, AtomicOrdering::SeqCst);

        // Should not be stale with long threshold
        assert!(!stats.is_stale(60_000)); // 60 seconds
    }

    #[test]
    fn test_multiple_entity_types() {
        let stats = TableStatistics::new();

        stats.increment("User");
        stats.increment("User");
        stats.increment("Post");
        stats.increment("Comment");
        stats.increment("Comment");
        stats.increment("Comment");

        assert_eq!(stats.entity_count("User"), 2);
        assert_eq!(stats.entity_count("Post"), 1);
        assert_eq!(stats.entity_count("Comment"), 3);
        assert_eq!(stats.total_entities(), 6);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let stats = Arc::new(TableStatistics::new());

        let mut handles = vec![];

        // Spawn multiple threads incrementing
        for _ in 0..10 {
            let stats_clone = Arc::clone(&stats);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    stats_clone.increment("User");
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(stats.entity_count("User"), 1000);
    }
}
