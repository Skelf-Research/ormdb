//! Configuration types for Raft cluster.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS configuration for Raft cluster communication.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RaftTlsConfig {
    /// Whether TLS is enabled for Raft communication.
    pub enabled: bool,

    /// Path to the TLS certificate file.
    pub cert_path: Option<PathBuf>,

    /// Path to the TLS private key file.
    pub key_path: Option<PathBuf>,

    /// Path to the CA certificate for verifying peer certificates.
    pub ca_path: Option<PathBuf>,

    /// Whether to require client certificates (mutual TLS).
    pub require_client_cert: bool,
}

impl RaftTlsConfig {
    /// Create a new TLS configuration with certificates.
    pub fn new(cert_path: PathBuf, key_path: PathBuf) -> Self {
        Self {
            enabled: true,
            cert_path: Some(cert_path),
            key_path: Some(key_path),
            ca_path: None,
            require_client_cert: false,
        }
    }

    /// Enable mutual TLS with CA certificate.
    pub fn with_ca(mut self, ca_path: PathBuf) -> Self {
        self.ca_path = Some(ca_path);
        self
    }

    /// Require client certificates.
    pub fn with_client_cert_required(mut self) -> Self {
        self.require_client_cert = true;
        self
    }
}

/// Configuration for a Raft node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// This node's unique ID.
    pub node_id: u64,

    /// Address this node listens on for Raft RPCs.
    pub raft_listen_addr: String,

    /// Address other nodes use to reach this node.
    pub raft_advertise_addr: String,

    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval_ms: u64,

    /// Minimum election timeout in milliseconds.
    pub election_timeout_min_ms: u64,

    /// Maximum election timeout in milliseconds.
    pub election_timeout_max_ms: u64,

    /// Number of log entries between snapshots.
    pub snapshot_threshold: u64,

    /// Maximum entries per AppendEntries RPC.
    pub max_entries_per_append: u64,

    /// Directory for Raft data (logs, snapshots).
    pub data_dir: PathBuf,

    /// TLS configuration for cluster communication.
    #[serde(default)]
    pub tls: RaftTlsConfig,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            raft_listen_addr: "0.0.0.0:9001".to_string(),
            raft_advertise_addr: "127.0.0.1:9001".to_string(),
            heartbeat_interval_ms: 150,
            election_timeout_min_ms: 300,
            election_timeout_max_ms: 500,
            snapshot_threshold: 10000,
            max_entries_per_append: 100,
            data_dir: PathBuf::from("./raft-data"),
            tls: RaftTlsConfig::default(),
        }
    }
}

impl RaftConfig {
    /// Create a new configuration with the given node ID.
    pub fn new(node_id: u64) -> Self {
        Self {
            node_id,
            ..Default::default()
        }
    }

    /// Set the node ID.
    pub fn with_node_id(mut self, node_id: u64) -> Self {
        self.node_id = node_id;
        self
    }

    /// Set the Raft listen address.
    pub fn with_raft_listen_addr(mut self, addr: impl Into<String>) -> Self {
        self.raft_listen_addr = addr.into();
        self
    }

    /// Set the Raft advertise address.
    pub fn with_raft_advertise_addr(mut self, addr: impl Into<String>) -> Self {
        self.raft_advertise_addr = addr.into();
        self
    }

    /// Set the heartbeat interval.
    pub fn with_heartbeat_interval_ms(mut self, ms: u64) -> Self {
        self.heartbeat_interval_ms = ms;
        self
    }

    /// Set the election timeout range.
    pub fn with_election_timeout_ms(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.election_timeout_min_ms = min_ms;
        self.election_timeout_max_ms = max_ms;
        self
    }

    /// Set the snapshot threshold.
    pub fn with_snapshot_threshold(mut self, threshold: u64) -> Self {
        self.snapshot_threshold = threshold;
        self
    }

    /// Set the data directory.
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = dir.into();
        self
    }

    /// Set the TLS configuration.
    pub fn with_tls(mut self, tls: RaftTlsConfig) -> Self {
        self.tls = tls;
        self
    }

    /// Enable TLS with the given certificate and key files.
    pub fn with_tls_enabled(mut self, cert_path: PathBuf, key_path: PathBuf) -> Self {
        self.tls = RaftTlsConfig::new(cert_path, key_path);
        self
    }
}

/// Configuration for cluster membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Initial cluster members for bootstrap.
    pub initial_members: Vec<NodeMember>,
}

impl ClusterConfig {
    /// Create a new cluster configuration.
    pub fn new(members: Vec<NodeMember>) -> Self {
        Self {
            initial_members: members,
        }
    }

    /// Create a single-node cluster configuration.
    pub fn single_node(node_id: u64, addr: impl Into<String>) -> Self {
        Self {
            initial_members: vec![NodeMember::new(node_id, addr)],
        }
    }
}

/// A node member in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMember {
    /// Node ID.
    pub id: u64,
    /// Node address (host:port).
    pub addr: String,
}

impl NodeMember {
    /// Create a new node member.
    pub fn new(id: u64, addr: impl Into<String>) -> Self {
        Self {
            id,
            addr: addr.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_config_default() {
        let config = RaftConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.heartbeat_interval_ms, 150);
        assert_eq!(config.election_timeout_min_ms, 300);
        assert_eq!(config.election_timeout_max_ms, 500);
    }

    #[test]
    fn test_raft_config_builder() {
        let config = RaftConfig::new(5)
            .with_raft_listen_addr("0.0.0.0:9999")
            .with_heartbeat_interval_ms(100)
            .with_election_timeout_ms(200, 400);

        assert_eq!(config.node_id, 5);
        assert_eq!(config.raft_listen_addr, "0.0.0.0:9999");
        assert_eq!(config.heartbeat_interval_ms, 100);
        assert_eq!(config.election_timeout_min_ms, 200);
        assert_eq!(config.election_timeout_max_ms, 400);
    }

    #[test]
    fn test_cluster_config() {
        let config = ClusterConfig::new(vec![
            NodeMember::new(1, "node1:9001"),
            NodeMember::new(2, "node2:9001"),
            NodeMember::new(3, "node3:9001"),
        ]);

        assert_eq!(config.initial_members.len(), 3);
        assert_eq!(config.initial_members[0].id, 1);
        assert_eq!(config.initial_members[0].addr, "node1:9001");
    }

    #[test]
    fn test_single_node_config() {
        let config = ClusterConfig::single_node(1, "localhost:9001");
        assert_eq!(config.initial_members.len(), 1);
        assert_eq!(config.initial_members[0].id, 1);
    }
}
