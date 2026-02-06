//! Factory for creating Raft network connections.

use openraft::network::RaftNetworkFactory;
use openraft::BasicNode;

use crate::network::transport::NngRaftNetwork;
use crate::types::{NodeId, TypeConfig};

/// Factory for creating NNG network connections.
///
/// This implements openraft's `RaftNetworkFactory` trait to create
/// network connections to other cluster nodes on demand.
pub struct NngNetworkFactory {
    /// This node's ID.
    node_id: NodeId,
}

impl NngNetworkFactory {
    /// Create a new network factory.
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }
}

impl RaftNetworkFactory<TypeConfig> for NngNetworkFactory {
    type Network = NngRaftNetwork;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        tracing::debug!(
            "Creating network client from node {} to node {} at {}",
            self.node_id,
            target,
            node.addr
        );
        NngRaftNetwork::new(target, node.clone())
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
