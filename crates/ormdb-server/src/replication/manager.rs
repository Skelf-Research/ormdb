//! Replication manager for coordinating replication state.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info};

use ormdb_core::replication::{ChangeLog, ReplicaApplier};
use ormdb_proto::replication::{ReplicationRole, ReplicationStatus, StreamChangesResponse};

/// Replication manager that coordinates replication state and operations.
///
/// This manages:
/// - Server replication role (Primary, Replica, Standalone)
/// - Write access control (replicas are read-only)
/// - Change streaming for replica consumers
/// - Replication status reporting
pub struct ReplicationManager {
    /// Current replication role.
    role: RwLock<ReplicationRole>,
    /// Changelog for streaming changes.
    changelog: Arc<ChangeLog>,
    /// Applier for applying changes (replica mode only).
    applier: Option<Arc<ReplicaApplier>>,
}

impl ReplicationManager {
    /// Create a new replication manager in primary mode.
    pub fn new_primary(changelog: Arc<ChangeLog>) -> Self {
        info!("Initializing replication manager in primary mode");
        Self {
            role: RwLock::new(ReplicationRole::Primary),
            changelog,
            applier: None,
        }
    }

    /// Create a new replication manager in standalone mode.
    pub fn new_standalone(changelog: Arc<ChangeLog>) -> Self {
        info!("Initializing replication manager in standalone mode");
        Self {
            role: RwLock::new(ReplicationRole::Standalone),
            changelog,
            applier: None,
        }
    }

    /// Create a new replication manager in replica mode.
    pub fn new_replica(
        changelog: Arc<ChangeLog>,
        applier: Arc<ReplicaApplier>,
        primary_addr: String,
    ) -> Self {
        info!(primary = %primary_addr, "Initializing replication manager in replica mode");
        Self {
            role: RwLock::new(ReplicationRole::Replica { primary_addr }),
            changelog,
            applier: Some(applier),
        }
    }

    /// Check if writes are allowed (i.e., not a replica).
    pub async fn can_write(&self) -> bool {
        let role = self.role.read().await;
        role.can_write()
    }

    /// Get the current replication role.
    pub async fn role(&self) -> ReplicationRole {
        self.role.read().await.clone()
    }

    /// Check if this server is a replica.
    pub async fn is_replica(&self) -> bool {
        let role = self.role.read().await;
        role.is_replica()
    }

    /// Check if this server is the primary.
    pub async fn is_primary(&self) -> bool {
        let role = self.role.read().await;
        role.is_primary()
    }

    /// Get the current replication status.
    pub async fn status(&self) -> ReplicationStatus {
        let role = self.role.read().await.clone();
        let current_lsn = self.changelog.current_lsn();

        let (lag_entries, lag_ms) = match &role {
            ReplicationRole::Replica { .. } => {
                if let Some(ref applier) = self.applier {
                    let applied = applier.applied_lsn();
                    let lag = current_lsn.saturating_sub(applied);
                    // TODO: compute lag_ms based on timestamps
                    (lag, 0)
                } else {
                    (0, 0)
                }
            }
            _ => (0, 0),
        };

        ReplicationStatus {
            role,
            current_lsn,
            lag_entries,
            lag_ms,
        }
    }

    /// Get the current LSN.
    pub fn current_lsn(&self) -> u64 {
        self.changelog.current_lsn()
    }

    /// Stream changes from the given LSN.
    pub fn stream_changes(&self, from_lsn: u64, batch_size: u32) -> StreamChangesResponse {
        let (entries, has_more) = self
            .changelog
            .scan_batch(from_lsn, batch_size as usize)
            .unwrap_or_else(|e| {
                debug!(error = %e, "failed to scan changelog");
                (vec![], false)
            });

        let next_lsn = entries.last().map(|e| e.lsn + 1).unwrap_or(from_lsn);

        StreamChangesResponse::new(entries, next_lsn, has_more)
    }

    /// Stream changes with entity filter.
    pub fn stream_changes_filtered(
        &self,
        from_lsn: u64,
        batch_size: u32,
        entity_filter: Option<&[String]>,
    ) -> StreamChangesResponse {
        let (entries, has_more) = self
            .changelog
            .scan_filtered(from_lsn, batch_size as usize, entity_filter)
            .unwrap_or_else(|e| {
                debug!(error = %e, "failed to scan changelog");
                (vec![], false)
            });

        let next_lsn = entries.last().map(|e| e.lsn + 1).unwrap_or(from_lsn);

        StreamChangesResponse::new(entries, next_lsn, has_more)
    }

    /// Get the applied LSN (for replicas).
    pub fn applied_lsn(&self) -> Option<u64> {
        self.applier.as_ref().map(|a| a.applied_lsn())
    }

    /// Get a reference to the changelog.
    pub fn changelog(&self) -> &ChangeLog {
        &self.changelog
    }

    /// Promote this server to primary.
    ///
    /// This is used when the primary fails and a replica needs to take over.
    pub async fn promote_to_primary(&self) {
        let mut role = self.role.write().await;
        if role.is_replica() {
            info!("Promoting server from replica to primary");
            *role = ReplicationRole::Primary;
        }
    }

    /// Demote this server to replica.
    ///
    /// This is used when another server becomes primary.
    pub async fn demote_to_replica(&self, primary_addr: String) {
        let mut role = self.role.write().await;
        if !role.is_replica() {
            info!(primary = %primary_addr, "Demoting server to replica");
            *role = ReplicationRole::Replica { primary_addr };
        }
    }
}

/// Shared replication manager handle.
pub type SharedReplicationManager = Arc<ReplicationManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::storage::{StorageConfig, StorageEngine};

    fn create_test_changelog() -> Arc<ChangeLog> {
        let db = sled::Config::new().temporary(true).open().unwrap();
        Arc::new(ChangeLog::open(&db).unwrap())
    }

    #[tokio::test]
    async fn test_standalone_mode() {
        let changelog = create_test_changelog();
        let manager = ReplicationManager::new_standalone(changelog);

        assert!(manager.can_write().await);
        assert!(!manager.is_replica().await);
        assert!(!manager.is_primary().await);

        let status = manager.status().await;
        assert!(matches!(status.role, ReplicationRole::Standalone));
    }

    #[tokio::test]
    async fn test_primary_mode() {
        let changelog = create_test_changelog();
        let manager = ReplicationManager::new_primary(changelog);

        assert!(manager.can_write().await);
        assert!(manager.is_primary().await);
        assert!(!manager.is_replica().await);

        let status = manager.status().await;
        assert!(matches!(status.role, ReplicationRole::Primary));
    }

    #[tokio::test]
    async fn test_replica_mode() {
        let changelog = create_test_changelog();

        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            StorageEngine::open(StorageConfig::new(dir.path())).unwrap()
        );
        let applier = Arc::new(ReplicaApplier::new(storage));

        let manager = ReplicationManager::new_replica(
            changelog,
            applier,
            "localhost:5432".to_string(),
        );

        assert!(!manager.can_write().await);
        assert!(manager.is_replica().await);
        assert!(!manager.is_primary().await);

        let status = manager.status().await;
        assert!(matches!(status.role, ReplicationRole::Replica { .. }));
    }

    #[tokio::test]
    async fn test_promote_to_primary() {
        let changelog = create_test_changelog();

        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            StorageEngine::open(StorageConfig::new(dir.path())).unwrap()
        );
        let applier = Arc::new(ReplicaApplier::new(storage));

        let manager = ReplicationManager::new_replica(
            changelog,
            applier,
            "localhost:5432".to_string(),
        );

        assert!(!manager.can_write().await);

        manager.promote_to_primary().await;

        assert!(manager.can_write().await);
        assert!(manager.is_primary().await);
    }

    #[tokio::test]
    async fn test_stream_changes_empty() {
        let changelog = create_test_changelog();
        let manager = ReplicationManager::new_standalone(changelog);

        let response = manager.stream_changes(1, 10);

        assert!(response.entries.is_empty());
        assert_eq!(response.next_lsn, 1);
        assert!(!response.has_more);
    }
}
