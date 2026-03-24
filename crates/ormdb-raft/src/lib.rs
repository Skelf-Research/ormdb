//! ORMDB Raft - Distributed consensus for ORMDB using openraft.
//!
//! This crate provides Raft consensus capabilities for ORMDB, enabling:
//! - Automatic leader election
//! - Log replication across cluster nodes
//! - Snapshot support for efficient state transfer
//! - Dynamic cluster membership changes
//!
//! # Architecture
//!
//! The crate implements openraft's traits:
//! - [`SledRaftLogStorage`] - Persistent log storage using sled
//! - [`OrmdbStateMachine`] - State machine that applies mutations to ORMDB storage
//! - [`NngRaftNetwork`] - Network transport using NNG
//!
//! # Usage
//!
//! ```ignore
//! use ormdb_raft::{RaftClusterManager, RaftConfig};
//!
//! // Create configuration
//! let config = RaftConfig::default()
//!     .with_node_id(1)
//!     .with_raft_addr("0.0.0.0:9001");
//!
//! // Initialize the cluster manager
//! let manager = RaftClusterManager::new(config, storage, db).await?;
//!
//! // Initialize cluster (first node only)
//! manager.initialize_cluster(vec![(1, "node1:9001"), (2, "node2:9001")]).await?;
//!
//! // Submit writes through the leader
//! let response = manager.write(ClientRequest::Mutate(mutation)).await?;
//! ```

pub mod cluster;
pub mod config;
pub mod error;
pub mod network;
pub mod storage;
pub mod types;

// Re-export main types
pub use cluster::manager::RaftClusterManager;
pub use config::{ClusterConfig, NodeMember, RaftConfig};
pub use error::RaftError;
pub use types::{ClientRequest, ClientResponse, NodeId, TypeConfig};

// Re-export openraft types that users might need
pub use openraft::{BasicNode, Raft, RaftMetrics};
