//! Version compaction and garbage collection.
//!
//! This module provides the CompactionEngine for cleaning up old MVCC versions
//! and tombstones according to a configured retention policy.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, info, instrument, warn};

use super::{RetentionPolicy, StorageEngine, VersionedKey};
use crate::error::Error;

/// Result of a compaction run.
#[derive(Debug, Clone, Default)]
pub struct CompactionResult {
    /// Number of old versions removed.
    pub versions_removed: u64,

    /// Number of tombstones removed.
    pub tombstones_removed: u64,

    /// Estimated bytes reclaimed (before sled compaction).
    pub bytes_reclaimed: u64,

    /// Duration of the compaction run.
    pub duration: Duration,

    /// Number of entities processed.
    pub entities_processed: u64,

    /// Number of errors encountered (non-fatal).
    pub errors: u64,
}

impl CompactionResult {
    /// Check if any cleanup was performed.
    pub fn did_cleanup(&self) -> bool {
        self.versions_removed > 0 || self.tombstones_removed > 0
    }
}

/// Engine for compacting storage by removing old versions and tombstones.
pub struct CompactionEngine {
    storage: Arc<StorageEngine>,
    policy: RetentionPolicy,
}

impl CompactionEngine {
    /// Create a new compaction engine with the given storage and policy.
    pub fn new(storage: Arc<StorageEngine>, policy: RetentionPolicy) -> Self {
        Self { storage, policy }
    }

    /// Run one compaction cycle.
    ///
    /// This scans all versioned data and removes versions that exceed the
    /// retention policy limits.
    #[instrument(skip(self))]
    pub fn compact(&self) -> CompactionResult {
        let start = Instant::now();
        let mut result = CompactionResult::default();

        let now_ts = current_timestamp();
        let min_age_ts = now_ts.saturating_sub(self.policy.min_age.as_micros() as u64);
        let ttl_cutoff = self.policy.ttl.map(|ttl| {
            now_ts.saturating_sub(ttl.as_micros() as u64)
        });

        // Scan the data tree to find all entity IDs with multiple versions
        match self.collect_entities_to_compact(min_age_ts) {
            Ok(entities) => {
                result.entities_processed = entities.len() as u64;

                for (entity_id, versions) in entities {
                    match self.compact_entity(&entity_id, versions, ttl_cutoff, min_age_ts) {
                        Ok((versions_removed, tombstones_removed, bytes)) => {
                            result.versions_removed += versions_removed;
                            result.tombstones_removed += tombstones_removed;
                            result.bytes_reclaimed += bytes;
                        }
                        Err(e) => {
                            warn!(
                                entity_id = ?entity_id,
                                error = %e,
                                "Failed to compact entity"
                            );
                            result.errors += 1;
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to collect entities for compaction");
                result.errors += 1;
            }
        }

        result.duration = start.elapsed();

        if result.did_cleanup() {
            info!(
                versions_removed = result.versions_removed,
                tombstones_removed = result.tombstones_removed,
                bytes_reclaimed = result.bytes_reclaimed,
                duration_ms = result.duration.as_millis() as u64,
                "Compaction completed"
            );
        } else {
            debug!(
                entities_processed = result.entities_processed,
                duration_ms = result.duration.as_millis() as u64,
                "Compaction completed with no cleanup needed"
            );
        }

        result
    }

    /// Compact storage by triggering sled's internal compaction.
    ///
    /// This should be called after removing old versions to reclaim disk space.
    pub fn compact_sled(&self) -> Result<(), Error> {
        // sled doesn't expose a direct compact() method, but we can flush
        // which triggers internal maintenance
        self.storage.flush()?;
        Ok(())
    }

    /// Collect all entities with their version timestamps for compaction analysis.
    fn collect_entities_to_compact(
        &self,
        min_age_ts: u64,
    ) -> Result<Vec<([u8; 16], Vec<u64>)>, Error> {
        let mut entities: std::collections::HashMap<[u8; 16], Vec<u64>> =
            std::collections::HashMap::new();

        // Scan the data tree to find all versioned keys
        let data_tree = self.storage.data_tree();

        for result in data_tree.iter() {
            let (key_bytes, _) = result?;

            if let Some(key) = VersionedKey::decode(&key_bytes) {
                // Only consider versions older than min_age
                if key.version_ts < min_age_ts {
                    entities
                        .entry(key.entity_id)
                        .or_default()
                        .push(key.version_ts);
                }
            }
        }

        // Sort versions for each entity (oldest first)
        let mut result: Vec<_> = entities.into_iter().collect();
        for (_, versions) in &mut result {
            versions.sort();
        }

        // Only return entities with more than 1 version (candidates for cleanup)
        Ok(result
            .into_iter()
            .filter(|(_, versions)| versions.len() > 1)
            .collect())
    }

    /// Compact a single entity's versions.
    ///
    /// Returns (versions_removed, tombstones_removed, bytes_reclaimed).
    fn compact_entity(
        &self,
        entity_id: &[u8; 16],
        mut versions: Vec<u64>,
        ttl_cutoff: Option<u64>,
        min_age_ts: u64,
    ) -> Result<(u64, u64, u64), Error> {
        let mut versions_removed = 0u64;
        let mut tombstones_removed = 0u64;
        let mut bytes_reclaimed = 0u64;

        // Always keep the latest version
        let latest_version = versions.pop();

        // Check if the latest is a tombstone that should be cleaned
        let mut should_remove_tombstone = false;
        if let Some(latest) = latest_version {
            if self.policy.cleanup_tombstones {
                // Check if it's a tombstone older than min_age
                if latest < min_age_ts {
                    if let Ok(Some(record)) = self.storage.get(entity_id, latest) {
                        if record.deleted {
                            // Check TTL if configured
                            let should_clean = match ttl_cutoff {
                                Some(cutoff) => latest < cutoff,
                                None => true, // No TTL, clean based on min_age only
                            };
                            if should_clean {
                                should_remove_tombstone = true;
                            }
                        }
                    }
                }
            }
        }

        // Determine which versions to remove
        let versions_to_keep = self.policy.max_versions.unwrap_or(usize::MAX);

        // versions is already sorted oldest-first, and we've removed the latest
        // We want to keep the most recent N-1 versions (since latest is already kept)
        let keep_count = versions_to_keep.saturating_sub(1);
        let remove_count = versions.len().saturating_sub(keep_count);

        // Remove the oldest versions that exceed max_versions
        for &version_ts in versions.iter().take(remove_count) {
            // Check TTL: if there's a TTL and this version is newer than cutoff, keep it
            if let Some(cutoff) = ttl_cutoff {
                if version_ts >= cutoff {
                    continue;
                }
            }

            // Remove this version
            match self.remove_version(entity_id, version_ts) {
                Ok(bytes) => {
                    versions_removed += 1;
                    bytes_reclaimed += bytes;
                }
                Err(e) => {
                    debug!(
                        entity_id = ?entity_id,
                        version = version_ts,
                        error = %e,
                        "Failed to remove version"
                    );
                }
            }
        }

        // Remove tombstone if applicable
        if should_remove_tombstone {
            if let Some(latest) = latest_version {
                match self.remove_version(entity_id, latest) {
                    Ok(bytes) => {
                        tombstones_removed += 1;
                        bytes_reclaimed += bytes;

                        // Also remove the latest pointer and type index entry
                        self.cleanup_entity_metadata(entity_id)?;
                    }
                    Err(e) => {
                        debug!(
                            entity_id = ?entity_id,
                            error = %e,
                            "Failed to remove tombstone"
                        );
                    }
                }
            }
        }

        Ok((versions_removed, tombstones_removed, bytes_reclaimed))
    }

    /// Remove a specific version of an entity.
    ///
    /// Returns the approximate size of the removed data.
    fn remove_version(&self, entity_id: &[u8; 16], version_ts: u64) -> Result<u64, Error> {
        let key = VersionedKey::new(*entity_id, version_ts);
        let key_bytes = key.encode();

        let data_tree = self.storage.data_tree();

        // Get the size before removing
        let size = match data_tree.get(&key_bytes)? {
            Some(value) => value.len() as u64 + key_bytes.len() as u64,
            None => 0,
        };

        // Remove the version
        data_tree.remove(&key_bytes)?;

        Ok(size)
    }

    /// Clean up metadata for a fully deleted entity (tombstone removed).
    fn cleanup_entity_metadata(&self, entity_id: &[u8; 16]) -> Result<(), Error> {
        // Remove the latest version pointer
        let latest_key = self.latest_key(entity_id);
        self.storage.meta_tree().remove(&latest_key)?;

        // Note: We don't remove from the type index here because we don't know
        // the entity type. The type index entry will be orphaned but harmless.
        // A full GC could scan for orphaned entries periodically.

        Ok(())
    }

    /// Get the metadata key for the latest version pointer.
    fn latest_key(&self, entity_id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(7 + 16);
        key.extend_from_slice(b"latest:");
        key.extend_from_slice(entity_id);
        key
    }
}

/// Get the current timestamp in microseconds since UNIX epoch.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Record, StorageConfig};
    use std::time::Duration;

    struct TestDb {
        engine: Arc<StorageEngine>,
        _dir: tempfile::TempDir,
    }

    fn test_storage() -> TestDb {
        let dir = tempfile::tempdir().unwrap();
        let config = StorageConfig::new(dir.path());
        let engine = Arc::new(StorageEngine::open(config).unwrap());
        TestDb { engine, _dir: dir }
    }

    #[test]
    fn test_compaction_empty_db() {
        let db = test_storage();
        let policy = RetentionPolicy::default();
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();
        assert_eq!(result.versions_removed, 0);
        assert_eq!(result.tombstones_removed, 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_compaction_single_version_kept() {
        let db = test_storage();
        let entity_id = StorageEngine::generate_id();

        // Insert a single version
        let key = VersionedKey::new(entity_id, 1);
        db.engine.put(key, Record::new(vec![1, 2, 3])).unwrap();

        let policy = RetentionPolicy::with_max_versions(5).min_age(Duration::from_secs(0));
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();

        // Single version should be kept
        assert_eq!(result.versions_removed, 0);
        assert!(db.engine.get(&entity_id, 1).unwrap().is_some());
    }

    #[test]
    fn test_compaction_removes_old_versions() {
        let db = test_storage();
        let entity_id = StorageEngine::generate_id();

        // Insert 5 versions with very old timestamps (so they pass min_age)
        for i in 1..=5 {
            let key = VersionedKey::new(entity_id, i);
            db.engine.put(key, Record::new(vec![i as u8])).unwrap();
        }

        // Policy: keep only 2 versions, no min_age
        let policy = RetentionPolicy::with_max_versions(2).min_age(Duration::from_secs(0));
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();

        // Should remove 3 oldest versions (1, 2, 3), keep 4 and 5
        assert_eq!(result.versions_removed, 3);

        // Verify remaining versions
        assert!(db.engine.get(&entity_id, 1).unwrap().is_none());
        assert!(db.engine.get(&entity_id, 2).unwrap().is_none());
        assert!(db.engine.get(&entity_id, 3).unwrap().is_none());
        assert!(db.engine.get(&entity_id, 4).unwrap().is_some());
        assert!(db.engine.get(&entity_id, 5).unwrap().is_some());
    }

    #[test]
    fn test_compaction_respects_min_age() {
        let db = test_storage();
        let entity_id = StorageEngine::generate_id();

        // Insert versions: some old, some recent
        let now = current_timestamp();
        let old_ts = 1000; // Very old
        let recent_ts = now.saturating_sub(1000); // Just 1ms ago

        db.engine
            .put(VersionedKey::new(entity_id, old_ts), Record::new(vec![1]))
            .unwrap();
        db.engine
            .put(VersionedKey::new(entity_id, recent_ts), Record::new(vec![2]))
            .unwrap();

        // Policy: keep 1 version but require 1 hour min_age
        let policy = RetentionPolicy::with_max_versions(1).min_age(Duration::from_secs(3600));
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();

        // Recent version is protected by min_age, so nothing should be removed
        // (only old versions are even considered)
        assert!(db.engine.get(&entity_id, old_ts).unwrap().is_some());
        assert!(db.engine.get(&entity_id, recent_ts).unwrap().is_some());

        // The result might show 0 or entities_processed depending on timing
        assert!(result.errors == 0);
    }

    #[test]
    fn test_compaction_result_metrics() {
        let db = test_storage();
        let entity_id = StorageEngine::generate_id();

        // Insert multiple versions
        for i in 1..=10 {
            let key = VersionedKey::new(entity_id, i);
            db.engine.put(key, Record::new(vec![i as u8; 100])).unwrap();
        }

        let policy = RetentionPolicy::with_max_versions(3).min_age(Duration::from_secs(0));
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();

        // Should have processed 1 entity and removed 7 versions
        assert_eq!(result.versions_removed, 7);
        assert!(result.bytes_reclaimed > 0);
        assert!(result.duration.as_nanos() > 0);
        assert!(result.did_cleanup());
    }

    #[test]
    fn test_keep_all_policy() {
        let db = test_storage();
        let entity_id = StorageEngine::generate_id();

        // Insert many versions
        for i in 1..=100 {
            let key = VersionedKey::new(entity_id, i);
            db.engine.put(key, Record::new(vec![i as u8])).unwrap();
        }

        let policy = RetentionPolicy::keep_all();
        let compactor = CompactionEngine::new(db.engine.clone(), policy);

        let result = compactor.compact();

        // Nothing should be removed
        assert_eq!(result.versions_removed, 0);
        assert!(!result.did_cleanup());
    }
}
