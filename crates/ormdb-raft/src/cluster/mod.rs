//! Cluster management for Raft.
//!
//! This module provides:
//! - [`RaftClusterManager`] - Main orchestrator for the Raft cluster
//! - Membership change operations
//! - Request routing and leader forwarding

pub mod manager;
pub mod membership;
pub mod router;

pub use manager::RaftClusterManager;
pub use membership::MembershipManager;
pub use router::RequestRouter;
