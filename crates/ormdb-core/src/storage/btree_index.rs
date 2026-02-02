//! B-tree index for efficient range queries.
//!
//! This module provides a B-tree secondary index using Microsoft's bf-tree
//! that maps field values to entity IDs, enabling O(log N) lookups for range filters.

use std::path::Path;
use std::sync::Arc;

use bf_tree::{BfTree, Config, ScanReturnField};

use crate::error::Error;
use ormdb_proto::Value;

/// Default cache size for bf-tree (64MB).
const DEFAULT_CACHE_SIZE: usize = 64 * 1024 * 1024;

/// Max key length for bf-tree keys (includes entity/column/value/id).
const DEFAULT_MAX_KEY_LEN: usize = 256;

/// Max record size for bf-tree leaf pages.
const DEFAULT_MAX_RECORD_SIZE: usize = 1536;

/// Buffer size for scan operations.
const SCAN_BUFFER_SIZE: usize = 1024;

/// B-tree index for efficient range lookups.
///
/// Stores a mapping from (column_name, value) -> entity_id for each entity type.
/// This enables O(log N) lookups for WHERE column > value, BETWEEN, etc.
///
/// Key format: `[entity_type:var][0x00][column_name:var][0x00][encoded_value:var][0x00][entity_id:16]`
/// Value format: `[entity_id:16]`
///
/// Unlike HashIndex which stores multiple entity IDs per key, BTreeIndex stores
/// one entity ID per key to support efficient range scans.
pub struct BTreeIndex {
    tree: Arc<BfTree>,
}

impl BTreeIndex {
    /// Open or create a B-tree index at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut config = Config::new(path.as_ref(), DEFAULT_CACHE_SIZE);
        config.cb_max_key_len(DEFAULT_MAX_KEY_LEN);
        config.cb_max_record_size(DEFAULT_MAX_RECORD_SIZE);
        let tree = BfTree::with_config(config, None)
            .map_err(|e| Error::InvalidData(format!("failed to open bf-tree: {:?}", e)))?;
        Ok(Self {
            tree: Arc::new(tree),
        })
    }

    /// Open with custom configuration.
    pub fn open_with_config(config: Config) -> Result<Self, Error> {
        let tree = BfTree::with_config(config, None)
            .map_err(|e| Error::InvalidData(format!("failed to open bf-tree: {:?}", e)))?;
        Ok(Self {
            tree: Arc::new(tree),
        })
    }

    /// Build the index key for an entity type, column, value, and entity ID.
    ///
    /// Format: `[entity_type][0x00][column_name][0x00][encoded_value][0x00][entity_id]`
    fn build_key(
        entity_type: &str,
        column_name: &str,
        value: &Value,
        entity_id: [u8; 16],
    ) -> Vec<u8> {
        let mut key = Self::build_value_prefix(entity_type, column_name, value);
        key.extend_from_slice(&entity_id);
        key
    }

    /// Build a prefix key for scanning all values of a column.
    fn build_prefix(entity_type: &str, column_name: &str) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(entity_type.as_bytes());
        key.push(0x00);
        key.extend_from_slice(column_name.as_bytes());
        key.push(0x00);
        key
    }

    /// Build a prefix key for a specific column value (excluding entity ID).
    fn build_value_prefix(entity_type: &str, column_name: &str, value: &Value) -> Vec<u8> {
        let mut key = Self::build_prefix(entity_type, column_name);
        Self::encode_value_sortable_into(value, &mut key);
        key.push(0x00);
        key
    }

    /// Encode a value in a sortable format for B-tree keys, reusing the provided buffer.
    ///
    /// Uses a format that preserves sort order for byte comparison:
    /// - Integers: Big-endian with sign bit flipped for correct ordering
    /// - Floats: IEEE 754 with sign handling for correct ordering
    /// - Strings: UTF-8 bytes directly (lexicographic order)
    fn encode_value_sortable_into(value: &Value, buf: &mut Vec<u8>) {
        match value {
            Value::Null => {
                buf.push(0x00); // Null sorts first
            }
            Value::Bool(b) => {
                buf.push(0x01);
                buf.push(if *b { 1 } else { 0 });
            }
            Value::Int32(n) => {
                buf.push(0x02);
                // Flip sign bit for correct sort order (negative before positive)
                let sortable = (*n as u32) ^ 0x8000_0000;
                buf.extend_from_slice(&sortable.to_be_bytes());
            }
            Value::Int64(n) => {
                buf.push(0x03);
                // Flip sign bit for correct sort order
                let sortable = (*n as u64) ^ 0x8000_0000_0000_0000;
                buf.extend_from_slice(&sortable.to_be_bytes());
            }
            Value::Float32(n) => {
                buf.push(0x04);
                // Convert to sortable integer representation
                let bits = n.to_bits();
                let sortable = if (bits & 0x8000_0000) != 0 {
                    !bits // Negative: flip all bits
                } else {
                    bits ^ 0x8000_0000 // Positive: flip sign bit
                };
                buf.extend_from_slice(&sortable.to_be_bytes());
            }
            Value::Float64(n) => {
                buf.push(0x05);
                let bits = n.to_bits();
                let sortable = if (bits & 0x8000_0000_0000_0000) != 0 {
                    !bits
                } else {
                    bits ^ 0x8000_0000_0000_0000
                };
                buf.extend_from_slice(&sortable.to_be_bytes());
            }
            Value::String(s) => {
                buf.push(0x06);
                buf.extend_from_slice(s.as_bytes());
            }
            Value::Uuid(id) => {
                buf.push(0x07);
                buf.extend_from_slice(id);
            }
            Value::Timestamp(ts) => {
                buf.push(0x08);
                // Timestamps are already unsigned, use big-endian
                buf.extend_from_slice(&ts.to_be_bytes());
            }
            Value::Bytes(b) => {
                buf.push(0x09);
                buf.extend_from_slice(b);
            }
            _ => {
                buf.push(0xFF); // Unsupported types sort last
            }
        }
    }

    /// Encode a value in a sortable format for B-tree keys.
    /// Convenience wrapper that allocates a new buffer.
    /// Used by tests and as a fallback for simple cases.
    #[cfg_attr(not(test), allow(dead_code))]
    fn encode_value_sortable(value: &Value) -> Vec<u8> {
        let mut buf = Vec::new();
        Self::encode_value_sortable_into(value, &mut buf);
        buf
    }

    /// Build end key for "greater than" range scan.
    /// Returns a key that is guaranteed to be greater than any value.
    fn build_end_key_max(entity_type: &str, column_name: &str) -> Vec<u8> {
        let mut key = Self::build_prefix(entity_type, column_name);
        // Add max bytes to ensure we capture all values and entity IDs
        key.extend_from_slice(&[0xFF; 64]);
        key
    }

    /// Build start key for "less than" range scan.
    /// Returns the column prefix which is the minimum key.
    fn build_start_key_min(entity_type: &str, column_name: &str) -> Vec<u8> {
        let mut key = Self::build_prefix(entity_type, column_name);
        // Add minimum value marker
        key.push(0x00);
        key
    }

    fn build_value_min_key(entity_type: &str, column_name: &str, value: &Value) -> Vec<u8> {
        let mut key = Self::build_value_prefix(entity_type, column_name, value);
        key.extend_from_slice(&[0x00; 16]);
        key
    }

    fn build_value_max_key(entity_type: &str, column_name: &str, value: &Value) -> Vec<u8> {
        let mut key = Self::build_value_prefix(entity_type, column_name, value);
        key.extend_from_slice(&[0xFF; 16]);
        key
    }

    /// Insert an entity ID into the index for a value.
    pub fn insert(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let key = Self::build_key(entity_type, column_name, value, entity_id);
        // Store entity_id as the value
        self.tree.insert(&key, &entity_id);
        Ok(())
    }

    /// Remove an entity ID from the index for a value.
    pub fn remove(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
        entity_id: [u8; 16],
    ) -> Result<(), Error> {
        let key = Self::build_key(entity_type, column_name, value, entity_id);
        self.tree.delete(&key);
        Ok(())
    }

    /// Lookup all entity IDs where value > threshold. O(log N + K) operation.
    pub fn scan_greater_than(
        &self,
        entity_type: &str,
        column_name: &str,
        threshold: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_value_max_key(entity_type, column_name, threshold);
        let end_key = Self::build_end_key_max(entity_type, column_name);

        self.scan_range(&start_key, &end_key, true)
    }

    /// Lookup all entity IDs where value >= threshold. O(log N + K) operation.
    pub fn scan_greater_equal(
        &self,
        entity_type: &str,
        column_name: &str,
        threshold: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_value_min_key(entity_type, column_name, threshold);
        let end_key = Self::build_end_key_max(entity_type, column_name);

        self.scan_range(&start_key, &end_key, false)
    }

    /// Lookup all entity IDs where value < threshold. O(log N + K) operation.
    pub fn scan_less_than(
        &self,
        entity_type: &str,
        column_name: &str,
        threshold: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_start_key_min(entity_type, column_name);
        let end_key = Self::build_value_min_key(entity_type, column_name, threshold);

        // Exclude end (threshold) for strict less-than
        self.scan_range_exclude_end(&start_key, &end_key, true)
    }

    /// Lookup all entity IDs where value <= threshold. O(log N + K) operation.
    pub fn scan_less_equal(
        &self,
        entity_type: &str,
        column_name: &str,
        threshold: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_start_key_min(entity_type, column_name);
        let end_key = Self::build_value_max_key(entity_type, column_name, threshold);

        // Include end (threshold) for less-than-or-equal
        self.scan_range_exclude_end(&start_key, &end_key, false)
    }

    /// Lookup all entity IDs where low <= value <= high. O(log N + K) operation.
    pub fn scan_between(
        &self,
        entity_type: &str,
        column_name: &str,
        low: &Value,
        high: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_value_min_key(entity_type, column_name, low);
        let end_key = Self::build_value_max_key(entity_type, column_name, high);

        // Include both endpoints
        self.scan_range(&start_key, &end_key, false)
    }

    /// Scan all entity IDs for a column in value order.
    pub fn scan_all(
        &self,
        entity_type: &str,
        column_name: &str,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_start_key_min(entity_type, column_name);
        let end_key = Self::build_end_key_max(entity_type, column_name);

        self.scan_range(&start_key, &end_key, false)
    }

    /// Lookup all entity IDs where value == target. O(log N + K) operation.
    pub fn scan_equal(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let start_key = Self::build_value_min_key(entity_type, column_name, value);
        let end_key = Self::build_value_max_key(entity_type, column_name, value);

        self.scan_range(&start_key, &end_key, false)
    }

    /// Internal range scan implementation.
    ///
    /// - If exclude_start is true, skip results that match start_key exactly.
    /// - If exclude_end is true, skip results that match end_key exactly.
    fn scan_range_inner(
        &self,
        start_key: &[u8],
        end_key: &[u8],
        exclude_start: bool,
        exclude_end: bool,
    ) -> Result<Vec<[u8; 16]>, Error> {
        let mut results = Vec::new();
        let mut buffer = vec![0u8; SCAN_BUFFER_SIZE];

        // Use bf-tree's range scan
        let mut iter = self
            .tree
            .scan_with_end_key(start_key, end_key, ScanReturnField::KeyAndValue)
            .map_err(|e| Error::InvalidData(format!("scan error: {:?}", e)))?;

        while let Some((key_len, value_len)) = iter.next(&mut buffer) {
            let key = &buffer[..key_len];

            // Skip if it matches start_key exactly and we're excluding start
            if exclude_start && key == start_key {
                continue;
            }

            // Skip if it matches end_key exactly and we're excluding end
            if exclude_end && key == end_key {
                continue;
            }

            // Extract entity_id from value (after key in buffer)
            let value_start = key_len;
            let value_end = key_len + value_len;
            if value_len >= 16 && value_end <= buffer.len() {
                let mut id = [0u8; 16];
                id.copy_from_slice(&buffer[value_start..value_start + 16]);
                results.push(id);
            }
        }

        Ok(results)
    }

    /// Internal range scan - exclude start only.
    fn scan_range(
        &self,
        start_key: &[u8],
        end_key: &[u8],
        exclude_start: bool,
    ) -> Result<Vec<[u8; 16]>, Error> {
        self.scan_range_inner(start_key, end_key, exclude_start, false)
    }

    /// Internal range scan - exclude end only.
    fn scan_range_exclude_end(
        &self,
        start_key: &[u8],
        end_key: &[u8],
        exclude_end: bool,
    ) -> Result<Vec<[u8; 16]>, Error> {
        self.scan_range_inner(start_key, end_key, false, exclude_end)
    }

    /// Check if a B-tree index exists for a column.
    ///
    /// Unlike hash index, we don't have a quick way to check, so we try a minimal scan.
    pub fn has_index(&self, entity_type: &str, column_name: &str) -> Result<bool, Error> {
        // Build end key directly without cloning start_key
        let mut end_key = Vec::new();
        end_key.extend_from_slice(entity_type.as_bytes());
        end_key.push(0x00);
        end_key.extend_from_slice(column_name.as_bytes());
        end_key.push(0x00);
        let start_key = end_key.clone();
        end_key.push(0xFF);

        // Try to scan just one item
        match self
            .tree
            .scan_with_end_key(&start_key, &end_key, ScanReturnField::Key)
        {
            Ok(mut iter) => {
                let mut buffer = [0u8; 256];
                Ok(iter.next(&mut buffer).is_some())
            }
            Err(_) => Ok(false),
        }
    }

    /// Build index for a column from data iterator.
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

    /// Drop all index entries for a specific column.
    ///
    /// Collects all keys first, then deletes them. For very large indexes,
    /// this may use significant memory, but it's simpler and more reliable
    /// than incremental approaches that can have issues with bf-tree's
    /// buffered delete behavior.
    pub fn drop_column_index(&self, entity_type: &str, column_name: &str) -> Result<(), Error> {
        let start_key = Self::build_start_key_min(entity_type, column_name);
        let end_key = Self::build_end_key_max(entity_type, column_name);

        let mut buffer = vec![0u8; SCAN_BUFFER_SIZE];
        let mut keys = Vec::new();

        let mut iter = self
            .tree
            .scan_with_end_key(&start_key, &end_key, ScanReturnField::Key)
            .map_err(|e| Error::InvalidData(format!("scan error: {:?}", e)))?;

        while let Some((key_len, _value_len)) = iter.next(&mut buffer) {
            keys.push(buffer[..key_len].to_vec());
        }

        // Drop iterator before deleting to avoid holding locks
        drop(iter);

        for key in keys {
            self.tree.delete(&key);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> BTreeIndex {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_btree");
        BTreeIndex::open(&path).unwrap()
    }

    #[test]
    fn test_insert_and_scan_gt() {
        let index = test_index();

        // Insert some ages
        for age in [20, 25, 30, 35, 40, 45, 50] {
            let id = [age as u8; 16];
            index
                .insert("User", "age", &Value::Int32(age), id)
                .unwrap();
        }

        // Scan for age > 30
        let results = index
            .scan_greater_than("User", "age", &Value::Int32(30))
            .unwrap();

        // Should find 35, 40, 45, 50
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_scan_between() {
        let index = test_index();

        for age in [20, 25, 30, 35, 40, 45, 50] {
            let id = [age as u8; 16];
            index
                .insert("User", "age", &Value::Int32(age), id)
                .unwrap();
        }

        // Scan for 25 <= age <= 40
        let results = index
            .scan_between("User", "age", &Value::Int32(25), &Value::Int32(40))
            .unwrap();

        // Should find 25, 30, 35, 40
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_scan_lt() {
        let index = test_index();

        for age in [20, 25, 30, 35, 40] {
            let id = [age as u8; 16];
            index
                .insert("User", "age", &Value::Int32(age), id)
                .unwrap();
        }

        // Scan for age < 30
        let results = index
            .scan_less_than("User", "age", &Value::Int32(30))
            .unwrap();

        // Should find 20, 25
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_scan_equal_with_duplicates() {
        let index = test_index();

        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        let id3 = [3u8; 16];

        index.insert("User", "status", &Value::String("active".to_string()), id1).unwrap();
        index.insert("User", "status", &Value::String("active".to_string()), id2).unwrap();
        index.insert("User", "status", &Value::String("active".to_string()), id3).unwrap();

        let results = index
            .scan_equal("User", "status", &Value::String("active".to_string()))
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.contains(&id1));
        assert!(results.contains(&id2));
        assert!(results.contains(&id3));
    }

    #[test]
    fn test_sortable_encoding() {
        // Verify that negative numbers sort before positive
        let neg = BTreeIndex::encode_value_sortable(&Value::Int32(-10));
        let zero = BTreeIndex::encode_value_sortable(&Value::Int32(0));
        let pos = BTreeIndex::encode_value_sortable(&Value::Int32(10));

        assert!(neg < zero);
        assert!(zero < pos);
    }
}
