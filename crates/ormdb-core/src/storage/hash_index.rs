//! Hash index for efficient equality lookups.
//!
//! This module provides a hash-based secondary index that maps field values
//! to entity IDs, enabling O(1) lookups for equality filters.
//!
//! ## Optimizations
//!
//! - **has_index cache**: Caches results of `has_index()` checks to avoid
//!   expensive prefix scans on every query.
//! - **lookup cache**: LRU-style cache for hot lookup results to reduce
//!   disk I/O for frequently accessed values.

use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use sled::Db;

use crate::error::Error;
use ormdb_proto::Value;

/// Tree name prefix for hash indexes.
pub const HASH_INDEX_TREE_PREFIX: &str = "index:hash:";

/// Maximum number of lookup results to cache.
const LOOKUP_CACHE_MAX_ENTRIES: usize = 10_000;

/// Hash index for efficient equality lookups.
///
/// Stores a mapping from (column_name, value) -> [entity_ids] for each entity type.
/// This enables O(1) lookups for WHERE column = value queries instead of O(N) scans.
///
/// Key format: `[column_name_len:1][column_name][0x00][encoded_value]`
/// Value format: `[entity_id_1:16][entity_id_2:16]...` (packed 16-byte IDs)
pub struct HashIndex {
    db: Db,
    /// Cache for has_index checks: (entity_type, column_name) -> exists
    /// Avoids expensive prefix scans on every query.
    has_index_cache: DashMap<(String, String), bool>,
    /// Cache for lookup results: full_key -> entity_ids
    /// Provides O(1) lookups for hot values without disk I/O.
    lookup_cache: DashMap<Vec<u8>, Vec<[u8; 16]>>,
    /// Counter for cache eviction (simple clock-based eviction)
    lookup_cache_ops: AtomicU64,
}

impl HashIndex {
    /// Open or create a hash index.
    pub fn open(db: &Db) -> Result<Self, Error> {
        Ok(Self {
            db: db.clone(),
            has_index_cache: DashMap::new(),
            lookup_cache: DashMap::new(),
            lookup_cache_ops: AtomicU64::new(0),
        })
    }

    /// Get the sled tree for a specific entity type.
    fn tree_for_entity(&self, entity_type: &str) -> Result<sled::Tree, Error> {
        let tree_name = format!("{}{}", HASH_INDEX_TREE_PREFIX, entity_type);
        Ok(self.db.open_tree(tree_name)?)
    }

    /// Build the index key for a column value.
    ///
    /// Format: `[column_name_len:1][column_name][0x00][encoded_value]`
    fn build_key(column_name: &str, value: &Value) -> Vec<u8> {
        let mut key = Vec::new();

        // Column name with length prefix
        let name_bytes = column_name.as_bytes();
        key.push(name_bytes.len() as u8);
        key.extend_from_slice(name_bytes);

        // Separator
        key.push(0x00);

        // Encoded value
        key.extend(Self::encode_value_for_key(value));

        key
    }

    /// Encode a value for use in an index key.
    ///
    /// Uses a simple format that preserves type information and is
    /// suitable for byte comparison.
    fn encode_value_for_key(value: &Value) -> Vec<u8> {
        let mut buf = Vec::new();

        match value {
            Value::Null => {
                buf.push(0x00); // Type tag for Null
            }
            Value::Bool(b) => {
                buf.push(0x01); // Type tag for Bool
                buf.push(if *b { 1 } else { 0 });
            }
            Value::Int32(n) => {
                buf.push(0x02); // Type tag for Int32
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::Int64(n) => {
                buf.push(0x03); // Type tag for Int64
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::Float32(n) => {
                buf.push(0x04); // Type tag for Float32
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::Float64(n) => {
                buf.push(0x05); // Type tag for Float64
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::String(s) => {
                buf.push(0x06); // Type tag for String
                // Store string directly (no dictionary encoding for simplicity)
                let s_bytes = s.as_bytes();
                buf.extend_from_slice(&(s_bytes.len() as u32).to_le_bytes());
                buf.extend_from_slice(s_bytes);
            }
            Value::Uuid(id) => {
                buf.push(0x07); // Type tag for Uuid
                buf.extend_from_slice(id);
            }
            Value::Timestamp(ts) => {
                buf.push(0x08); // Type tag for Timestamp
                buf.extend_from_slice(&ts.to_le_bytes());
            }
            Value::Bytes(b) => {
                buf.push(0x09); // Type tag for Bytes
                buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
                buf.extend_from_slice(b);
            }
            // Arrays are not typically indexed
            _ => {
                buf.push(0xFF); // Unsupported type marker
            }
        }

        buf
    }

    /// Insert an entity ID into the index for a value.
    ///
    /// If the value already has entity IDs associated, the new ID is appended.
    pub fn insert(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let tree = self.tree_for_entity(entity_type)?;
        let key = Self::build_key(column_name, value);

        // Read-modify-write: get existing IDs, add new one, write back
        let mut ids = match tree.get(&key)? {
            Some(bytes) => Self::decode_id_list(&bytes),
            None => Vec::new(),
        };

        // Check if already present (avoid duplicates)
        if !ids.contains(&entity_id) {
            ids.push(entity_id);
            tree.insert(&key, Self::encode_id_list(&ids))?;

            // Update caches
            self.has_index_cache
                .insert((entity_type.to_string(), column_name.to_string()), true);
            // Invalidate lookup cache for this key (data changed)
            let full_key = self.build_full_key(entity_type, &key);
            self.lookup_cache.remove(&full_key);
        }

        Ok(())
    }

    /// Build a full cache key including entity type.
    fn build_full_key(&self, entity_type: &str, key: &[u8]) -> Vec<u8> {
        let mut full_key = Vec::with_capacity(entity_type.len() + 1 + key.len());
        full_key.extend_from_slice(entity_type.as_bytes());
        full_key.push(0x00);
        full_key.extend_from_slice(key);
        full_key
    }

    /// Batch insert multiple entity IDs for efficient bulk loading.
    ///
    /// Groups insertions by value and performs one read-modify-write per unique value,
    /// reducing O(n^2) to O(n) complexity for bulk operations.
    pub fn insert_batch(
        &self,
        entity_type: &str,
        column_name: &str,
        values_and_ids: impl IntoIterator<Item = (Value, [u8; 16])>,
    ) -> Result<usize, Error> {
        use std::collections::{HashMap, HashSet};

        // Group IDs by value key
        let mut groups: HashMap<Vec<u8>, Vec<[u8; 16]>> = HashMap::new();
        for (value, id) in values_and_ids {
            let key = Self::build_key(column_name, &value);
            groups.entry(key).or_default().push(id);
        }

        if groups.is_empty() {
            return Ok(0);
        }

        let tree = self.tree_for_entity(entity_type)?;
        let mut total_inserted = 0;

        // For each unique value, read once, merge, write once
        for (key, new_ids) in &groups {
            // Read existing IDs into HashSet for O(1) dedup
            let mut id_set: HashSet<[u8; 16]> = match tree.get(key)? {
                Some(bytes) => Self::decode_id_list(&bytes).into_iter().collect(),
                None => HashSet::new(),
            };

            // Merge new IDs (HashSet handles dedup automatically)
            let before_len = id_set.len();
            id_set.extend(new_ids.iter().copied());
            total_inserted += id_set.len() - before_len;

            // Write back as Vec (sorted for determinism)
            let mut ids: Vec<_> = id_set.into_iter().collect();
            ids.sort_unstable();
            tree.insert(key, Self::encode_id_list(&ids))?;

            // Invalidate lookup cache for this key
            let full_key = self.build_full_key(entity_type, key);
            self.lookup_cache.remove(&full_key);
        }

        // Update has_index cache (we definitely have an index now)
        self.has_index_cache
            .insert((entity_type.to_string(), column_name.to_string()), true);

        Ok(total_inserted)
    }

    /// Remove an entity ID from the index for a value.
    ///
    /// If this was the last ID for the value, the index entry is removed.
    pub fn remove(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let tree = self.tree_for_entity(entity_type)?;
        let key = Self::build_key(column_name, value);

        if let Some(bytes) = tree.get(&key)? {
            let mut ids = Self::decode_id_list(&bytes);
            ids.retain(|id| id != &entity_id);

            if ids.is_empty() {
                tree.remove(&key)?;
            } else {
                tree.insert(&key, Self::encode_id_list(&ids))?;
            }

            // Invalidate lookup cache for this key
            let full_key = self.build_full_key(entity_type, &key);
            self.lookup_cache.remove(&full_key);
        }

        Ok(())
    }

    /// Lookup all entity IDs for a value. O(1) operation.
    ///
    /// Returns an empty vector if no entities have this value.
    /// Uses an in-memory cache for hot values to avoid disk I/O.
    pub fn lookup(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let key = Self::build_key(column_name, value);
        let full_key = self.build_full_key(entity_type, &key);

        // Check cache first (O(1) in-memory lookup)
        if let Some(cached) = self.lookup_cache.get(&full_key) {
            return Ok(cached.clone());
        }

        // Cache miss - fetch from disk
        let tree = self.tree_for_entity(entity_type)?;
        let ids = match tree.get(&key)? {
            Some(bytes) => Self::decode_id_list(&bytes),
            None => vec![],
        };

        // Update cache (with simple eviction when too large)
        self.maybe_evict_lookup_cache();
        self.lookup_cache.insert(full_key, ids.clone());

        Ok(ids)
    }

    /// Simple cache eviction: clear half the cache when it gets too large.
    fn maybe_evict_lookup_cache(&self) {
        let ops = self.lookup_cache_ops.fetch_add(1, Ordering::Relaxed);

        // Check every 1000 operations
        if ops % 1000 == 0 && self.lookup_cache.len() > LOOKUP_CACHE_MAX_ENTRIES {
            // Simple eviction: remove ~half the entries
            let to_remove: Vec<_> = self
                .lookup_cache
                .iter()
                .take(self.lookup_cache.len() / 2)
                .map(|r| r.key().clone())
                .collect();

            for key in to_remove {
                self.lookup_cache.remove(&key);
            }
        }
    }

    /// Check if a hash index exists for a column.
    ///
    /// Uses an in-memory cache to avoid expensive prefix scans on every query.
    /// The cache is populated on index creation and checked first.
    pub fn has_index(&self, entity_type: &str, column_name: &str) -> Result<bool, Error> {
        let cache_key = (entity_type.to_string(), column_name.to_string());

        // Check cache first (O(1) lookup)
        if let Some(exists) = self.has_index_cache.get(&cache_key) {
            return Ok(*exists);
        }

        // Cache miss - perform prefix scan
        let tree = self.tree_for_entity(entity_type)?;

        // Build prefix for this column
        let mut prefix = Vec::new();
        let name_bytes = column_name.as_bytes();
        prefix.push(name_bytes.len() as u8);
        prefix.extend_from_slice(name_bytes);
        prefix.push(0x00);

        // Check if any entries exist with this prefix
        let exists = tree.scan_prefix(&prefix).next().is_some();

        // Cache the result
        self.has_index_cache.insert(cache_key, exists);

        Ok(exists)
    }

    /// Build index for a column from columnar data.
    ///
    /// This is used to backfill an index for existing data.
    pub fn build_for_column<I>(
        &self,
        entity_type: &str,
        column_name: &str,
        data: I,
    ) -> Result<usize, Error>
    where
        I: IntoIterator<Item = Result<([u8; 16], Value), Error>>,
    {
        let mut count = 0;
        for item in data {
            let (entity_id, value) = item?;
            self.insert(entity_type, column_name, &value, entity_id)?;
            count += 1;
        }
        Ok(count)
    }

    /// Drop the index for an entity type.
    pub fn drop_index(&self, entity_type: &str) -> Result<(), Error> {
        let tree_name = format!("{}{}", HASH_INDEX_TREE_PREFIX, entity_type);
        self.db.drop_tree(tree_name)?;

        // Clear caches for this entity type
        self.has_index_cache
            .retain(|k, _| k.0 != entity_type);
        self.lookup_cache
            .retain(|k, _| !k.starts_with(entity_type.as_bytes()));

        Ok(())
    }

    /// Drop the index for a specific column.
    pub fn drop_column_index(&self, entity_type: &str, column_name: &str) -> Result<(), Error> {
        let tree = self.tree_for_entity(entity_type)?;

        // Build prefix for this column
        let mut prefix = Vec::new();
        let name_bytes = column_name.as_bytes();
        prefix.push(name_bytes.len() as u8);
        prefix.extend_from_slice(name_bytes);
        prefix.push(0x00);

        // Remove all entries with this prefix
        let keys_to_remove: Vec<_> = tree
            .scan_prefix(&prefix)
            .filter_map(|r| r.ok().map(|(k, _)| k))
            .collect();

        for key in keys_to_remove {
            tree.remove(key)?;
        }

        // Update caches
        self.has_index_cache
            .insert((entity_type.to_string(), column_name.to_string()), false);
        // Clear lookup cache entries for this column (starts with entity_type + separator + column prefix)
        let lookup_prefix = self.build_full_key(entity_type, &prefix[..prefix.len() - 1]); // exclude trailing 0x00
        self.lookup_cache
            .retain(|k, _| !k.starts_with(&lookup_prefix));

        Ok(())
    }

    /// Flush the index to disk.
    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush()?;
        Ok(())
    }

    /// Encode a list of entity IDs into bytes.
    fn encode_id_list(ids: &[[u8; 16]]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(ids.len() * 16);
        for id in ids {
            buf.extend_from_slice(id);
        }
        buf
    }

    /// Decode a list of entity IDs from bytes.
    fn decode_id_list(bytes: &[u8]) -> Vec<[u8; 16]> {
        let count = bytes.len() / 16;
        let mut ids = Vec::with_capacity(count);
        for i in 0..count {
            let offset = i * 16;
            if offset + 16 <= bytes.len() {
                let mut id = [0u8; 16];
                id.copy_from_slice(&bytes[offset..offset + 16]);
                ids.push(id);
            }
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> HashIndex {
        let db = sled::Config::new().temporary(true).open().unwrap();
        HashIndex::open(&db).unwrap()
    }

    #[test]
    fn test_insert_and_lookup() {
        let index = test_index();
        let entity_id = [1u8; 16];

        index
            .insert("User", "status", &Value::String("active".to_string()), entity_id)
            .unwrap();

        let result = index
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();
        assert_eq!(result, vec![entity_id]);
    }

    #[test]
    fn test_multiple_entities_same_value() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let id3 = [3u8; 16];

        // Insert multiple entities with same status
        let value = Value::String("active".to_string());
        index.insert("User", "status", &value, id1).unwrap();
        index.insert("User", "status", &value, id2).unwrap();
        index.insert("User", "status", &value, id3).unwrap();

        let result = index.lookup("User", "status", &value).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&id1));
        assert!(result.contains(&id2));
        assert!(result.contains(&id3));
    }

    #[test]
    fn test_lookup_not_found() {
        let index = test_index();

        let result = index
            .lookup("User", "status", &Value::String("nonexistent".to_string()))
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_remove() {
        let index = test_index();
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        let value = Value::String("active".to_string());
        index.insert("User", "status", &value, id1).unwrap();
        index.insert("User", "status", &value, id2).unwrap();

        // Remove one entity
        index.remove("User", "status", &value, id1).unwrap();

        let result = index.lookup("User", "status", &value).unwrap();
        assert_eq!(result, vec![id2]);

        // Remove the last entity
        index.remove("User", "status", &value, id2).unwrap();

        let result = index.lookup("User", "status", &value).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_different_value_types() {
        let index = test_index();
        let entity_id = [42u8; 16];

        // Test Int32
        index
            .insert("User", "age", &Value::Int32(25), entity_id)
            .unwrap();
        let result = index.lookup("User", "age", &Value::Int32(25)).unwrap();
        assert_eq!(result, vec![entity_id]);

        // Test Int64
        index
            .insert("User", "score", &Value::Int64(100_000), entity_id)
            .unwrap();
        let result = index.lookup("User", "score", &Value::Int64(100_000)).unwrap();
        assert_eq!(result, vec![entity_id]);

        // Test Bool
        index
            .insert("User", "active", &Value::Bool(true), entity_id)
            .unwrap();
        let result = index.lookup("User", "active", &Value::Bool(true)).unwrap();
        assert_eq!(result, vec![entity_id]);

        // Test Uuid
        let uuid = [99u8; 16];
        index
            .insert("User", "org_id", &Value::Uuid(uuid), entity_id)
            .unwrap();
        let result = index.lookup("User", "org_id", &Value::Uuid(uuid)).unwrap();
        assert_eq!(result, vec![entity_id]);
    }

    #[test]
    fn test_no_duplicate_ids() {
        let index = test_index();
        let entity_id = [1u8; 16];

        let value = Value::String("active".to_string());

        // Insert same entity twice
        index.insert("User", "status", &value, entity_id).unwrap();
        index.insert("User", "status", &value, entity_id).unwrap();

        let result = index.lookup("User", "status", &value).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_has_index() {
        let index = test_index();
        let entity_id = [1u8; 16];

        // Initially no index
        assert!(!index.has_index("User", "status").unwrap());

        // After insert, index exists
        index
            .insert("User", "status", &Value::String("active".to_string()), entity_id)
            .unwrap();
        assert!(index.has_index("User", "status").unwrap());

        // Different column has no index
        assert!(!index.has_index("User", "age").unwrap());
    }

    #[test]
    fn test_drop_column_index() {
        let index = test_index();
        let entity_id = [1u8; 16];

        index
            .insert("User", "status", &Value::String("active".to_string()), entity_id)
            .unwrap();
        index
            .insert("User", "age", &Value::Int32(25), entity_id)
            .unwrap();

        // Drop status index
        index.drop_column_index("User", "status").unwrap();

        // Status index gone
        assert!(!index.has_index("User", "status").unwrap());

        // Age index still exists
        assert!(index.has_index("User", "age").unwrap());
    }

    #[test]
    fn test_build_for_column() {
        let index = test_index();

        // Simulate columnar data
        let data: Vec<Result<([u8; 16], Value), Error>> = vec![
            Ok(([1u8; 16], Value::String("active".to_string()))),
            Ok(([2u8; 16], Value::String("inactive".to_string()))),
            Ok(([3u8; 16], Value::String("active".to_string()))),
        ];

        let count = index.build_for_column("User", "status", data).unwrap();
        assert_eq!(count, 3);

        // Verify index contents
        let active = index
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();
        assert_eq!(active.len(), 2);

        let inactive = index
            .lookup("User", "status", &Value::String("inactive".to_string()))
            .unwrap();
        assert_eq!(inactive.len(), 1);
    }

    #[test]
    fn test_insert_batch() {
        let index = test_index();

        // Batch insert 1000 entities with 4 status values
        let entries: Vec<(Value, [u8; 16])> = (0..1000)
            .map(|i| {
                let status = match i % 4 {
                    0 => "active",
                    1 => "inactive",
                    2 => "pending",
                    _ => "admin",
                };
                let mut id = [0u8; 16];
                id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                (Value::String(status.to_string()), id)
            })
            .collect();

        let count = index.insert_batch("User", "status", entries).unwrap();
        assert_eq!(count, 1000);

        // Verify distribution
        let active = index
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();
        assert_eq!(active.len(), 250);

        let inactive = index
            .lookup("User", "status", &Value::String("inactive".to_string()))
            .unwrap();
        assert_eq!(inactive.len(), 250);
    }

    #[test]
    fn test_insert_batch_with_existing() {
        let index = test_index();

        // Insert some existing entries
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        index
            .insert("User", "status", &Value::String("active".to_string()), id1)
            .unwrap();
        index
            .insert("User", "status", &Value::String("active".to_string()), id2)
            .unwrap();

        // Batch insert including duplicates and new entries
        let entries: Vec<(Value, [u8; 16])> = vec![
            (Value::String("active".to_string()), id1), // duplicate
            (Value::String("active".to_string()), [3u8; 16]), // new
            (Value::String("inactive".to_string()), [4u8; 16]), // new value
        ];

        let count = index.insert_batch("User", "status", entries).unwrap();
        assert_eq!(count, 2); // Only 2 new entries, not the duplicate

        let active = index
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();
        assert_eq!(active.len(), 3); // id1, id2, [3u8; 16]

        let inactive = index
            .lookup("User", "status", &Value::String("inactive".to_string()))
            .unwrap();
        assert_eq!(inactive.len(), 1);
    }
}
