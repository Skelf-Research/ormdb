//! Factory for creating Raft network connections.

use openraft::network::RaftNetworkFactory;
use openraft::BasicNode;

use crate::config::RaftTlsConfig;
use crate::network::transport::NngRaftNetwork;
use crate::types::{NodeId, TypeConfig};

/// Factory for creating NNG network connections.
///
/// This implements openraft's `RaftNetworkFactory` trait to create
/// network connections to other cluster nodes on demand.
pub struct NngNetworkFactory {
    /// This node's ID.
    node_id: NodeId,
    /// TLS configuration for connections.
    tls_config: Option<RaftTlsConfig>,
}

impl NngNetworkFactory {
    /// Create a new network factory.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            tls_config: None,
        }
    }

    /// Create a new network factory with TLS configuration.
    pub fn with_tls(node_id: NodeId, tls_config: RaftTlsConfig) -> Self {
        Self {
            node_id,
            tls_config: if tls_config.enabled { Some(tls_config) } else { None },
        }
    }
}

impl RaftNetworkFactory<TypeConfig> for NngNetworkFactory {
    type Network = NngRaftNetwork;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        tracing::debug!(
            "Creating network client from node {} to node {} at {} (TLS: {})",
            self.node_id,
            target,
            node.addr,
            self.tls_config.is_some()
        );

        if let Some(ref tls) = self.tls_config {
            NngRaftNetwork::with_tls(target, node.clone(), tls.clone())
        } else {
            NngRaftNetwork::new(target, node.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_creates_client() {
        let mut factory = NngNetworkFactory::new(1);
        let target_node = BasicNode {
            addr: "localhost:9001".to_string(),
        };

        let network = factory.new_client(2, &target_node).await;
        assert_eq!(network.target_id, 2);
    }
}
