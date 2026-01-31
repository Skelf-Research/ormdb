//! Replication and CDC protocol types.

use crate::ChangeType;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Persistent change log entry for CDC/replication.
///
/// Each mutation generates a changelog entry that is persisted with
/// a monotonically increasing LSN (Log Sequence Number).
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct ChangeLogEntry {
    /// Log Sequence Number - monotonically increasing identifier.
    pub lsn: u64,
    /// Timestamp in microseconds since epoch.
    pub timestamp: u64,
    /// The entity type that was modified.
    pub entity_type: String,
    /// The ID of the modified entity.
    pub entity_id: [u8; 16],
    /// Type of change (Insert, Update, Delete).
    pub change_type: ChangeType,
    /// List of fields that changed.
    pub changed_fields: Vec<String>,
    /// Full before-state (rkyv serialized bytes), None for inserts.
    pub before_data: Option<Vec<u8>>,
    /// Full after-state (rkyv serialized bytes), None for deletes.
    pub after_data: Option<Vec<u8>>,
    /// Schema version when the change occurred.
    pub schema_version: u64,
}

impl ChangeLogEntry {
    /// Create a new changelog entry for an insert operation.
    pub fn insert(
        entity_type: impl Into<String>,
        entity_id: [u8; 16],
        after_data: Vec<u8>,
        changed_fields: Vec<String>,
        schema_version: u64,
    ) -> Self {
        Self {
            lsn: 0, // Assigned by ChangeLog
            timestamp: Self::current_timestamp(),
            entity_type: entity_type.into(),
            entity_id,
            change_type: ChangeType::Insert,
            changed_fields,
            before_data: None,
            after_data: Some(after_data),
            schema_version,
        }
    }

    /// Create a new changelog entry for an update operation.
    pub fn update(
        entity_type: impl Into<String>,
        entity_id: [u8; 16],
        before_data: Vec<u8>,
        after_data: Vec<u8>,
        changed_fields: Vec<String>,
        schema_version: u64,
    ) -> Self {
        Self {
            lsn: 0,
            timestamp: Self::current_timestamp(),
            entity_type: entity_type.into(),
            entity_id,
            change_type: ChangeType::Update,
            changed_fields,
            before_data: Some(before_data),
            after_data: Some(after_data),
            schema_version,
        }
    }

    /// Create a new changelog entry for a delete operation.
    pub fn delete(
        entity_type: impl Into<String>,
        entity_id: [u8; 16],
        before_data: Vec<u8>,
        schema_version: u64,
    ) -> Self {
        Self {
            lsn: 0,
            timestamp: Self::current_timestamp(),
            entity_type: entity_type.into(),
            entity_id,
            change_type: ChangeType::Delete,
            changed_fields: vec![],
            before_data: Some(before_data),
            after_data: None,
            schema_version,
        }
    }

    /// Get current timestamp in microseconds.
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0)
    }
}

/// Request to stream changes from the changelog.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct StreamChangesRequest {
    /// Starting LSN (inclusive).
    pub from_lsn: u64,
    /// Maximum number of entries to return.
    pub batch_size: u32,
    /// Optional filter by entity type.
    pub entity_filter: Option<Vec<String>>,
}

impl StreamChangesRequest {
    /// Create a new stream changes request.
    pub fn new(from_lsn: u64, batch_size: u32) -> Self {
        Self {
            from_lsn,
            batch_size,
            entity_filter: None,
        }
    }

    /// Add an entity type filter.
    pub fn with_entity_filter(mut self, entities: Vec<String>) -> Self {
        self.entity_filter = Some(entities);
        self
    }
}

/// Response containing a batch of changelog entries.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct StreamChangesResponse {
    /// The changelog entries.
    pub entries: Vec<ChangeLogEntry>,
    /// The next LSN to request (for pagination).
    pub next_lsn: u64,
    /// Whether there are more entries available.
    pub has_more: bool,
}

impl StreamChangesResponse {
    /// Create a new stream changes response.
    pub fn new(entries: Vec<ChangeLogEntry>, next_lsn: u64, has_more: bool) -> Self {
        Self {
            entries,
            next_lsn,
            has_more,
        }
    }

    /// Create an empty response.
    pub fn empty(from_lsn: u64) -> Self {
        Self {
            entries: vec![],
            next_lsn: from_lsn,
            has_more: false,
        }
    }
}

/// Replication role of a server instance.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum ReplicationRole {
    /// Primary server - accepts writes.
    Primary,
    /// Replica server - read-only, replicates from primary.
    Replica {
        /// Address of the primary server.
        primary_addr: String,
    },
    /// Standalone server - not participating in replication.
    Standalone,
}

impl ReplicationRole {
    /// Check if this role allows writes.
    pub fn can_write(&self) -> bool {
        !matches!(self, ReplicationRole::Replica { .. })
    }

    /// Check if this is a replica.
    pub fn is_replica(&self) -> bool {
        matches!(self, ReplicationRole::Replica { .. })
    }

    /// Check if this is the primary.
    pub fn is_primary(&self) -> bool {
        matches!(self, ReplicationRole::Primary)
    }
}

impl Default for ReplicationRole {
    fn default() -> Self {
        ReplicationRole::Standalone
    }
}

/// Current replication status of a server.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct ReplicationStatus {
    /// Current replication role.
    pub role: ReplicationRole,
    /// Current LSN (highest written for primary, highest applied for replica).
    pub current_lsn: u64,
    /// Replication lag in number of entries (replica only).
    pub lag_entries: u64,
    /// Replication lag in milliseconds (replica only).
    pub lag_ms: u64,
}

impl ReplicationStatus {
    /// Create a new replication status.
    pub fn new(role: ReplicationRole, current_lsn: u64) -> Self {
        Self {
            role,
            current_lsn,
            lag_entries: 0,
            lag_ms: 0,
        }
    }

    /// Create status for a primary server.
    pub fn primary(current_lsn: u64) -> Self {
        Self::new(ReplicationRole::Primary, current_lsn)
    }

    /// Create status for a standalone server.
    pub fn standalone(current_lsn: u64) -> Self {
        Self::new(ReplicationRole::Standalone, current_lsn)
    }

    /// Create status for a replica with lag information.
    pub fn replica(primary_addr: String, applied_lsn: u64, lag_entries: u64, lag_ms: u64) -> Self {
        Self {
            role: ReplicationRole::Replica { primary_addr },
            current_lsn: applied_lsn,
            lag_entries,
            lag_ms,
        }
    }
}

impl Default for ReplicationStatus {
    fn default() -> Self {
        Self::standalone(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changelog_entry_insert() {
        let entry = ChangeLogEntry::insert(
            "User",
            [1u8; 16],
            vec![1, 2, 3],
            vec!["name".to_string(), "email".to_string()],
            1,
        );

        assert_eq!(entry.lsn, 0); // Not assigned yet
        assert_eq!(entry.entity_type, "User");
        assert_eq!(entry.change_type, ChangeType::Insert);
        assert!(entry.before_data.is_none());
        assert!(entry.after_data.is_some());
    }

    #[test]
    fn test_changelog_entry_update() {
        let entry = ChangeLogEntry::update(
            "User",
            [1u8; 16],
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec!["name".to_string()],
            1,
        );

        assert_eq!(entry.change_type, ChangeType::Update);
        assert!(entry.before_data.is_some());
        assert!(entry.after_data.is_some());
        assert_eq!(entry.changed_fields, vec!["name"]);
    }

    #[test]
    fn test_changelog_entry_delete() {
        let entry = ChangeLogEntry::delete("User", [1u8; 16], vec![1, 2, 3], 1);

        assert_eq!(entry.change_type, ChangeType::Delete);
        assert!(entry.before_data.is_some());
        assert!(entry.after_data.is_none());
    }

    #[test]
    fn test_stream_changes_request() {
        let request = StreamChangesRequest::new(100, 50)
            .with_entity_filter(vec!["User".to_string(), "Post".to_string()]);

        assert_eq!(request.from_lsn, 100);
        assert_eq!(request.batch_size, 50);
        assert_eq!(
            request.entity_filter,
            Some(vec!["User".to_string(), "Post".to_string()])
        );
    }

    #[test]
    fn test_replication_role() {
        assert!(ReplicationRole::Primary.can_write());
        assert!(ReplicationRole::Standalone.can_write());
        assert!(!ReplicationRole::Replica {
            primary_addr: "localhost:5432".to_string()
        }
        .can_write());
    }

    #[test]
    fn test_replication_status() {
        let status = ReplicationStatus::replica("localhost:5432".to_string(), 100, 5, 50);

        assert!(status.role.is_replica());
        assert_eq!(status.current_lsn, 100);
        assert_eq!(status.lag_entries, 5);
        assert_eq!(status.lag_ms, 50);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let entry = ChangeLogEntry::insert(
            "User",
            [1u8; 16],
            vec![1, 2, 3, 4],
            vec!["name".to_string()],
            1,
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived =
            rkyv::access::<ArchivedChangeLogEntry, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: ChangeLogEntry =
            rkyv::deserialize::<ChangeLogEntry, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(entry.entity_type, deserialized.entity_type);
        assert_eq!(entry.change_type, deserialized.change_type);
    }
}
