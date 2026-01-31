//! Replication support for ORMDB server.
//!
//! This module provides:
//! - [`ReplicationManager`] - Coordinates replication state and operations

mod manager;

pub use manager::{ReplicationManager, SharedReplicationManager};
