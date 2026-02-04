//! Changelog for async index updates.
//!
//! The changelog tracks pending mutations that need to be applied to secondary indexes.
//! This enables decoupling the write path (fast, single write to row store) from
//! index updates (async, background processing).
//!
//! ## Architecture
//!
//! ```text
//! Write Path:
//!   put_typed() → row_store.put() → changelog.append() → return (fast!)
//!
//! Background Worker:
//!   changelog.poll() → update indexes → changelog.mark_processed()
//!
//! Read Path:
//!   index.lookup() ∪ changelog.pending_for_key()
//! ```

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};

use ormdb_proto::Value;

/// A mutation entry in the changelog.
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    /// Unique sequence number for ordering
    pub seq: u64,
    /// Entity type (e.g., "User")
    pub entity_type: String,
    /// Entity ID
    pub entity_id: [u8; 16],
    /// The mutation type
    pub mutation: MutationType,
    /// Timestamp when the entry was created
    pub created_at: u64,
}

/// Type of mutation recorded in the changelog.
#[derive(Debug, Clone)]
pub enum MutationType {
    /// Entity was inserted or updated with these field values
    Upsert {
        /// Previous field values (None for insert)
        before: Option<Vec<(String, Value)>>,
        /// New field values
        after: Vec<(String, Value)>,
    },
    /// Entity was deleted
    Delete {
        /// Field values before deletion
        before: Vec<(String, Value)>,
    },
}

/// Changelog for tracking pending index updates.
///
/// Thread-safe and optimized for:
/// - Fast appends (O(1))
/// - Fast lookups by entity type + column (for read path merging)
/// - Batch processing by background worker
pub struct Changelog {
    /// Sequence counter for ordering entries
    next_seq: AtomicU64,
    /// Pending entries not yet processed (ordered by seq)
    pending: Mutex<VecDeque<ChangelogEntry>>,
    /// Index for fast lookup: (entity_type, column_name, value) -> pending entity IDs
    /// Used by read path to merge pending changes with index results
    pending_index: DashMap<(String, String, Vec<u8>), Vec<[u8; 16]>>,
    /// Maximum pending entries before forcing sync processing
    max_pending: usize,
    /// Callback for when max_pending is reached (optional backpressure)
    backpressure_callback: RwLock<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Changelog {
    /// Create a new changelog with default settings.
    pub fn new() -> Self {
        Self::with_max_pending(10_000)
    }

    /// Create a changelog with custom max pending entries.
    pub fn with_max_pending(max_pending: usize) -> Self {
        Self {
            next_seq: AtomicU64::new(1),
            pending: Mutex::new(VecDeque::new()),
            pending_index: DashMap::new(),
            max_pending,
            backpressure_callback: RwLock::new(None),
        }
    }

    /// Set a callback for backpressure when max_pending is reached.
    pub fn set_backpressure_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        *self.backpressure_callback.write() = Some(Arc::new(callback));
    }

    /// Append a mutation to the changelog.
    ///
    /// Returns the sequence number assigned to this entry.
    pub fn append(&self, entity_type: &str, entity_id: [u8; 16], mutation: MutationType) -> u64 {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let entry = ChangelogEntry {
            seq,
            entity_type: entity_type.to_string(),
            entity_id,
            mutation: mutation.clone(),
            created_at,
        };

        // Update pending index for read path merging
        self.update_pending_index(entity_type, entity_id, &mutation);

        // Append to pending queue
        let mut pending = self.pending.lock();
        pending.push_back(entry);

        // Check backpressure
        if pending.len() >= self.max_pending {
            drop(pending); // Release lock before callback
            if let Some(callback) = self.backpressure_callback.read().as_ref() {
                callback();
            }
        }

        seq
    }

    /// Update the pending index for fast read path lookups.
    fn update_pending_index(
        &self,
        entity_type: &str,
        entity_id: [u8; 16],
        mutation: &MutationType,
    ) {
        match mutation {
            MutationType::Upsert { before, after } => {
                // Remove from old value indexes
                if let Some(before_fields) = before {
                    for (col, val) in before_fields {
                        let key = (
                            entity_type.to_string(),
                            col.clone(),
                            encode_value_for_index(val),
                        );
                        if let Some(mut ids) = self.pending_index.get_mut(&key) {
                            ids.retain(|id| id != &entity_id);
                        }
                    }
                }
                // Add to new value indexes
                for (col, val) in after {
                    let key = (
                        entity_type.to_string(),
                        col.clone(),
                        encode_value_for_index(val),
                    );
                    self.pending_index
                        .entry(key)
                        .or_insert_with(Vec::new)
                        .push(entity_id);
                }
            }
            MutationType::Delete { before } => {
                // Remove from all value indexes
                for (col, val) in before {
                    let key = (
                        entity_type.to_string(),
                        col.clone(),
                        encode_value_for_index(val),
                    );
                    if let Some(mut ids) = self.pending_index.get_mut(&key) {
                        ids.retain(|id| id != &entity_id);
                    }
                }
            }
        }
    }

    /// Get pending entity IDs for a column value (for read path merging).
    ///
    /// This allows the read path to include entities that have been written
    /// but not yet indexed.
    pub fn pending_ids_for_value(
        &self,
        entity_type: &str,
        column_name: &str,
        value: &Value,
    ) -> Vec<[u8; 16]> {
        let key = (
            entity_type.to_string(),
            column_name.to_string(),
            encode_value_for_index(value),
        );
        self.pending_index
            .get(&key)
            .map(|ids| ids.clone())
            .unwrap_or_default()
    }

    /// Check if there are any pending changes for an entity.
    pub fn has_pending_for_entity(&self, entity_type: &str, entity_id: [u8; 16]) -> bool {
        let pending = self.pending.lock();
        pending
            .iter()
            .any(|e| e.entity_type == entity_type && e.entity_id == entity_id)
    }

    /// Poll for pending entries to process.
    ///
    /// Returns up to `max_entries` oldest pending entries without removing them.
    /// Use `mark_processed()` to remove entries after successful processing.
    pub fn poll(&self, max_entries: usize) -> Vec<ChangelogEntry> {
        let pending = self.pending.lock();
        pending.iter().take(max_entries).cloned().collect()
    }

    /// Mark entries as processed (remove from pending).
    ///
    /// `up_to_seq` removes all entries with seq <= up_to_seq.
    pub fn mark_processed(&self, up_to_seq: u64) {
        let mut pending = self.pending.lock();

        // Remove entries from front of queue (oldest first)
        while let Some(front) = pending.front() {
            if front.seq <= up_to_seq {
                let entry = pending.pop_front().unwrap();
                // Also remove from pending index
                self.remove_from_pending_index(&entry);
            } else {
                break;
            }
        }
    }

    /// Remove an entry from the pending index.
    fn remove_from_pending_index(&self, entry: &ChangelogEntry) {
        match &entry.mutation {
            MutationType::Upsert { after, .. } => {
                for (col, val) in after {
                    let key = (
                        entry.entity_type.clone(),
                        col.clone(),
                        encode_value_for_index(val),
                    );
                    if let Some(mut ids) = self.pending_index.get_mut(&key) {
                        ids.retain(|id| id != &entry.entity_id);
                        if ids.is_empty() {
                            drop(ids);
                            self.pending_index.remove(&key);
                        }
                    }
                }
            }
            MutationType::Delete { .. } => {
                // Delete entries don't add to pending index, so nothing to remove
            }
        }
    }

    /// Get the number of pending entries.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    /// Check if the changelog is empty.
    pub fn is_empty(&self) -> bool {
        self.pending.lock().is_empty()
    }

    /// Clear all pending entries (use with caution).
    pub fn clear(&self) {
        self.pending.lock().clear();
        self.pending_index.clear();
    }
}

impl Default for Changelog {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a value for use as an index key.
fn encode_value_for_index(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    match value {
        Value::Null => buf.push(0x00),
        Value::Bool(b) => {
            buf.push(0x01);
            buf.push(if *b { 1 } else { 0 });
        }
        Value::Int32(n) => {
            buf.push(0x02);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Int64(n) => {
            buf.push(0x03);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Float32(n) => {
            buf.push(0x04);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Float64(n) => {
            buf.push(0x05);
            buf.extend_from_slice(&n.to_le_bytes());
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
            buf.extend_from_slice(&ts.to_le_bytes());
        }
        Value::Bytes(b) => {
            buf.push(0x09);
            buf.extend_from_slice(b);
        }
        _ => buf.push(0xFF),
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changelog_append_and_poll() {
        let changelog = Changelog::new();

        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        // Append some entries
        let seq1 = changelog.append(
            "User",
            id1,
            MutationType::Upsert {
                before: None,
                after: vec![("name".to_string(), Value::String("Alice".to_string()))],
            },
        );

        let seq2 = changelog.append(
            "User",
            id2,
            MutationType::Upsert {
                before: None,
                after: vec![("name".to_string(), Value::String("Bob".to_string()))],
            },
        );

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(changelog.pending_count(), 2);

        // Poll entries
        let entries = changelog.poll(10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[1].seq, 2);

        // Mark first as processed
        changelog.mark_processed(1);
        assert_eq!(changelog.pending_count(), 1);

        // Mark second as processed
        changelog.mark_processed(2);
        assert!(changelog.is_empty());
    }

    #[test]
    fn test_changelog_pending_ids_for_value() {
        let changelog = Changelog::new();

        let id1 = [1u8; 16];
        let id2 = [2u8; 16];

        // Insert two users with status "active"
        changelog.append(
            "User",
            id1,
            MutationType::Upsert {
                before: None,
                after: vec![("status".to_string(), Value::String("active".to_string()))],
            },
        );

        changelog.append(
            "User",
            id2,
            MutationType::Upsert {
                before: None,
                after: vec![("status".to_string(), Value::String("active".to_string()))],
            },
        );

        // Should find both pending IDs
        let pending = changelog.pending_ids_for_value(
            "User",
            "status",
            &Value::String("active".to_string()),
        );
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&id1));
        assert!(pending.contains(&id2));

        // Different value should return empty
        let pending = changelog.pending_ids_for_value(
            "User",
            "status",
            &Value::String("inactive".to_string()),
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn test_changelog_update_removes_old_value() {
        let changelog = Changelog::new();

        let id1 = [1u8; 16];

        // Insert user with status "active"
        changelog.append(
            "User",
            id1,
            MutationType::Upsert {
                before: None,
                after: vec![("status".to_string(), Value::String("active".to_string()))],
            },
        );

        // Update to "inactive"
        changelog.append(
            "User",
            id1,
            MutationType::Upsert {
                before: Some(vec![(
                    "status".to_string(),
                    Value::String("active".to_string()),
                )]),
                after: vec![("status".to_string(), Value::String("inactive".to_string()))],
            },
        );

        // Should NOT be in "active" pending
        let pending = changelog.pending_ids_for_value(
            "User",
            "status",
            &Value::String("active".to_string()),
        );
        assert!(pending.is_empty());

        // Should be in "inactive" pending
        let pending = changelog.pending_ids_for_value(
            "User",
            "status",
            &Value::String("inactive".to_string()),
        );
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&id1));
    }
}
