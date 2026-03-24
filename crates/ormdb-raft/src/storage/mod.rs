//! Storage implementations for Raft.
//!
//! This module provides:
//! - [`SledRaftLogStorage`] - Persistent log storage using sled
//! - [`OrmdbStateMachine`] - State machine that applies mutations
//! - Snapshot support for efficient state transfer

pub mod log_storage;
pub mod snapshot;
pub mod state_machine;

pub use log_storage::SledRaftLogStorage;
pub use snapshot::SnapshotBuilder;
pub use state_machine::OrmdbStateMachine;
