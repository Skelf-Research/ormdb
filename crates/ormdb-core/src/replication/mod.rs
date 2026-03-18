//! Replication and CDC support.
//!
//! This module provides:
//! - [`ChangeLog`] - Persistent changelog with LSN ordering for CDC
//! - [`ReplicaApplier`] - Applies changelog entries to a replica

mod applier;
mod changelog;

pub use applier::ReplicaApplier;
pub use changelog::ChangeLog;
