//! Background worker for asynchronous index updates.
//!
//! This worker processes changelog entries and applies them to secondary indexes
//! (hash index, B-tree index, columnar store) in the background.
//!
//! ## Benefits
//!
//! - Single synchronous write to row store (fast!)
//! - Index updates happen asynchronously
//! - Reduces write amplification on hot path
//! - Better write throughput

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::Mutex;

use super::{BTreeIndex, Changelog, ChangelogEntry, ColumnarStore, HashIndex, MutationType};
use crate::error::Error;
use ormdb_proto::Value;

/// Configuration for the index worker.
#[derive(Debug, Clone)]
pub struct IndexWorkerConfig {
    /// How many entries to process per batch.
    pub batch_size: usize,
    /// How often to poll for new entries (milliseconds).
    pub poll_interval_ms: u64,
    /// Maximum time to wait for worker shutdown (milliseconds).
    pub shutdown_timeout_ms: u64,
}

impl Default for IndexWorkerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            poll_interval_ms: 10,
            shutdown_timeout_ms: 5000,
        }
    }
}

/// Background worker that processes changelog entries.
pub struct IndexWorker {
    /// Shutdown signal.
    shutdown: Arc<AtomicBool>,
    /// Worker thread handle.
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl IndexWorker {
    /// Start a new background index worker.
    ///
    /// The worker will continuously poll the changelog and apply entries to the indexes.
    pub fn start(
        changelog: Arc<Changelog>,
        hash_index: Arc<HashIndex>,
        btree_index: Option<Arc<BTreeIndex>>,
        columnar: Arc<ColumnarStore>,
        btree_columns: Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>,
        config: IndexWorkerConfig,
    ) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = thread::spawn(move || {
            Self::worker_loop(
                changelog,
                hash_index,
                btree_index,
                columnar,
                btree_columns,
                config,
                shutdown_clone,
            );
        });

        Self {
            shutdown,
            handle: Mutex::new(Some(handle)),
        }
    }

    /// Stop the worker and wait for it to finish.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.lock().take() {
            let _ = handle.join();
        }
    }

    /// Check if the worker is still running.
    pub fn is_running(&self) -> bool {
        self.handle.lock().as_ref().map(|h| !h.is_finished()).unwrap_or(false)
    }

    /// The main worker loop.
    fn worker_loop(
        changelog: Arc<Changelog>,
        hash_index: Arc<HashIndex>,
        btree_index: Option<Arc<BTreeIndex>>,
        columnar: Arc<ColumnarStore>,
        btree_columns: Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>,
        config: IndexWorkerConfig,
        shutdown: Arc<AtomicBool>,
    ) {
        let poll_interval = Duration::from_millis(config.poll_interval_ms);

        loop {
            if shutdown.load(Ordering::SeqCst) {
                // Process remaining entries before shutting down
                Self::drain_changelog(
                    &changelog,
                    &hash_index,
                    btree_index.as_deref(),
                    &columnar,
                    &btree_columns,
                    config.batch_size,
                );
                break;
            }

            let entries = changelog.poll(config.batch_size);
            if entries.is_empty() {
                // No work to do, sleep and try again
                thread::sleep(poll_interval);
                continue;
            }

            // Process batch
            let mut last_seq = 0;
            for entry in entries {
                if let Err(e) = Self::process_entry(
                    &entry,
                    &hash_index,
                    btree_index.as_deref(),
                    &columnar,
                    &btree_columns,
                ) {
                    tracing::error!(
                        seq = entry.seq,
                        entity_type = %entry.entity_type,
                        error = ?e,
                        "Failed to process changelog entry"
                    );
                    // Continue processing other entries
                }
                last_seq = entry.seq;
            }

            // Mark as processed
            if last_seq > 0 {
                changelog.mark_processed(last_seq);
            }
        }
    }

    /// Drain all remaining changelog entries (used during shutdown).
    fn drain_changelog(
        changelog: &Changelog,
        hash_index: &HashIndex,
        btree_index: Option<&BTreeIndex>,
        columnar: &ColumnarStore,
        btree_columns: &Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>,
        batch_size: usize,
    ) {
        loop {
            let entries = changelog.poll(batch_size);
            if entries.is_empty() {
                break;
            }

            let mut last_seq = 0;
            for entry in entries {
                let _ = Self::process_entry(&entry, hash_index, btree_index, columnar, btree_columns);
                last_seq = entry.seq;
            }

            if last_seq > 0 {
                changelog.mark_processed(last_seq);
            }
        }
    }

    /// Process a single changelog entry.
    fn process_entry(
        entry: &ChangelogEntry,
        hash_index: &HashIndex,
        btree_index: Option<&BTreeIndex>,
        columnar: &ColumnarStore,
        btree_columns: &Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>,
    ) -> Result<(), Error> {
        let entity_type = &entry.entity_type;
        let entity_id = entry.entity_id;

        // Get B-tree columns for this entity type
        let btree_cols = btree_columns(entity_type);

        match &entry.mutation {
            MutationType::Upsert { before, after } => {
                // Update hash index
                Self::update_hash_index(
                    hash_index,
                    entity_type,
                    entity_id,
                    before.as_deref(),
                    after,
                )?;

                // Update B-tree index
                if let Some(btree) = btree_index {
                    Self::update_btree_index(
                        btree,
                        entity_type,
                        entity_id,
                        before.as_deref(),
                        after,
                        &btree_cols,
                    )?;
                }

                // Update columnar store
                Self::update_columnar(columnar, entity_type, entity_id, after)?;
            }
            MutationType::Delete { before } => {
                // Remove from hash index
                Self::update_hash_index(
                    hash_index,
                    entity_type,
                    entity_id,
                    Some(before),
                    &[], // Empty after = delete
                )?;

                // Remove from B-tree index
                if let Some(btree) = btree_index {
                    Self::update_btree_index(
                        btree,
                        entity_type,
                        entity_id,
                        Some(before),
                        &[],
                        &btree_cols,
                    )?;
                }

                // Remove from columnar store
                Self::delete_columnar(columnar, entity_type, entity_id, before)?;
            }
        }

        Ok(())
    }

    /// Update hash index for a mutation.
    fn update_hash_index(
        hash_index: &HashIndex,
        entity_type: &str,
        entity_id: [u8; 16],
        before: Option<&[(String, Value)]>,
        after: &[(String, Value)],
    ) -> Result<(), Error> {
        use std::collections::{HashMap, HashSet};

        let to_map = |fields: &[(String, Value)]| -> HashMap<String, Value> {
            fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        let before_map = before.map(to_map).unwrap_or_default();
        let after_map = to_map(after);

        let mut names: HashSet<String> = HashSet::new();
        names.extend(before_map.keys().cloned());
        names.extend(after_map.keys().cloned());

        for name in names {
            let before_value = before_map.get(&name);
            let after_value = after_map.get(&name);

            if before_value == after_value {
                continue;
            }

            if let Some(value) = before_value {
                if !matches!(value, Value::Null) {
                    hash_index.remove(entity_type, &name, value, entity_id)?;
                }
            }

            if let Some(value) = after_value {
                if !matches!(value, Value::Null) {
                    hash_index.insert(entity_type, &name, value, entity_id)?;
                }
            }
        }

        Ok(())
    }

    /// Update B-tree index for a mutation.
    fn update_btree_index(
        btree: &BTreeIndex,
        entity_type: &str,
        entity_id: [u8; 16],
        before: Option<&[(String, Value)]>,
        after: &[(String, Value)],
        btree_columns: &[String],
    ) -> Result<(), Error> {
        use std::collections::{HashMap, HashSet};

        if btree_columns.is_empty() {
            return Ok(());
        }

        let to_map = |fields: &[(String, Value)]| -> HashMap<String, Value> {
            fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        let before_map = before.map(to_map).unwrap_or_default();
        let after_map = to_map(after);

        let columns: HashSet<&String> = btree_columns.iter().collect();

        for column in columns {
            let before_value = before_map.get(column);
            let after_value = after_map.get(column);

            if before_value == after_value {
                continue;
            }

            if let Some(value) = before_value {
                if !matches!(value, Value::Null) {
                    btree.remove(entity_type, column, value, entity_id)?;
                }
            }

            if let Some(value) = after_value {
                if !matches!(value, Value::Null) {
                    btree.insert(entity_type, column, value, entity_id)?;
                }
            }
        }

        Ok(())
    }

    /// Update columnar store for an upsert.
    fn update_columnar(
        columnar: &ColumnarStore,
        entity_type: &str,
        entity_id: [u8; 16],
        fields: &[(String, Value)],
    ) -> Result<(), Error> {
        let projection = columnar.projection(entity_type)?;
        projection.update_row(&entity_id, fields)?;
        Ok(())
    }

    /// Delete from columnar store.
    fn delete_columnar(
        columnar: &ColumnarStore,
        entity_type: &str,
        entity_id: [u8; 16],
        fields: &[(String, Value)],
    ) -> Result<(), Error> {
        let projection = columnar.projection(entity_type)?;
        let columns: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        projection.delete_row(&entity_id, &columns)?;
        Ok(())
    }
}

impl Drop for IndexWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use sled::Config as SledConfig;

    fn test_changelog() -> Arc<Changelog> {
        Arc::new(Changelog::new())
    }

    fn test_hash_index(db: &sled::Db) -> Arc<HashIndex> {
        Arc::new(HashIndex::open(db).unwrap())
    }

    fn test_columnar(db: &sled::Db) -> Arc<ColumnarStore> {
        Arc::new(ColumnarStore::open(db).unwrap())
    }

    fn no_btree_columns(_entity_type: &str) -> Vec<String> {
        Vec::new()
    }

    #[test]
    fn test_worker_processes_entries() {
        let dir = tempdir().unwrap();
        let db = SledConfig::new().path(dir.path()).open().unwrap();

        let changelog = test_changelog();
        let hash_index = test_hash_index(&db);
        let columnar = test_columnar(&db);

        let entity_id = [1u8; 16];

        // Append an entry
        changelog.append(
            "TestUser",
            entity_id,
            MutationType::Upsert {
                before: None,
                after: vec![("name".to_string(), Value::String("Alice".to_string()))],
            },
        );

        assert_eq!(changelog.pending_count(), 1);

        // Start worker with fast polling
        let config = IndexWorkerConfig {
            batch_size: 10,
            poll_interval_ms: 1,
            shutdown_timeout_ms: 1000,
        };

        let worker = IndexWorker::start(
            changelog.clone(),
            hash_index.clone(),
            None,
            columnar,
            Arc::new(no_btree_columns),
            config,
        );

        // Wait for processing
        std::thread::sleep(Duration::from_millis(50));

        // Stop worker
        worker.stop();

        // Verify entry was processed
        assert_eq!(changelog.pending_count(), 0);

        // Verify hash index was updated
        let ids = hash_index
            .lookup("TestUser", "name", &Value::String("Alice".to_string()))
            .unwrap();
        assert!(!ids.is_empty());
        assert!(ids.contains(&entity_id));
    }

    #[test]
    fn test_worker_handles_updates() {
        let dir = tempdir().unwrap();
        let db = SledConfig::new().path(dir.path()).open().unwrap();

        let changelog = test_changelog();
        let hash_index = test_hash_index(&db);
        let columnar = test_columnar(&db);

        let entity_id = [2u8; 16];

        // Insert then update
        changelog.append(
            "TestUser",
            entity_id,
            MutationType::Upsert {
                before: None,
                after: vec![("status".to_string(), Value::String("active".to_string()))],
            },
        );

        changelog.append(
            "TestUser",
            entity_id,
            MutationType::Upsert {
                before: Some(vec![("status".to_string(), Value::String("active".to_string()))]),
                after: vec![("status".to_string(), Value::String("inactive".to_string()))],
            },
        );

        let config = IndexWorkerConfig {
            batch_size: 10,
            poll_interval_ms: 1,
            shutdown_timeout_ms: 1000,
        };

        let worker = IndexWorker::start(
            changelog.clone(),
            hash_index.clone(),
            None,
            columnar,
            Arc::new(no_btree_columns),
            config,
        );

        std::thread::sleep(Duration::from_millis(50));
        worker.stop();

        // Verify old value removed, new value added
        let active_ids = hash_index
            .lookup("TestUser", "status", &Value::String("active".to_string()))
            .unwrap();
        assert!(!active_ids.contains(&entity_id));

        let inactive_ids = hash_index
            .lookup("TestUser", "status", &Value::String("inactive".to_string()))
            .unwrap();
        assert!(!inactive_ids.is_empty());
        assert!(inactive_ids.contains(&entity_id));
    }

    #[test]
    fn test_worker_handles_delete() {
        let dir = tempdir().unwrap();
        let db = SledConfig::new().path(dir.path()).open().unwrap();

        let changelog = test_changelog();
        let hash_index = test_hash_index(&db);
        let columnar = test_columnar(&db);

        let entity_id = [3u8; 16];

        // Insert then delete
        changelog.append(
            "TestUser",
            entity_id,
            MutationType::Upsert {
                before: None,
                after: vec![("email".to_string(), Value::String("test@example.com".to_string()))],
            },
        );

        changelog.append(
            "TestUser",
            entity_id,
            MutationType::Delete {
                before: vec![("email".to_string(), Value::String("test@example.com".to_string()))],
            },
        );

        let config = IndexWorkerConfig {
            batch_size: 10,
            poll_interval_ms: 1,
            shutdown_timeout_ms: 1000,
        };

        let worker = IndexWorker::start(
            changelog.clone(),
            hash_index.clone(),
            None,
            columnar,
            Arc::new(no_btree_columns),
            config,
        );

        std::thread::sleep(Duration::from_millis(50));
        worker.stop();

        // Verify entity removed from index
        let ids = hash_index
            .lookup("TestUser", "email", &Value::String("test@example.com".to_string()))
            .unwrap();
        assert!(!ids.contains(&entity_id));
    }
}
