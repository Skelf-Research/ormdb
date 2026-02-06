//! Cluster membership management.

use std::collections::BTreeMap;
use std::sync::Arc;

use openraft::BasicNode;

use crate::error::RaftError;
use crate::types::{Membership, NodeId, OrmdbRaft};

/// Manages cluster membership operations.
pub struct MembershipManager {
    /// The Raft instance.
    raft: Arc<OrmdbRaft>,
    /// This node's ID.
    node_id: NodeId,
}

impl MembershipManager {
    /// Create a new membership manager.
    pub fn new(raft: Arc<OrmdbRaft>, node_id: NodeId) -> Self {
        Self { raft, node_id }
    }

    /// Get current cluster members.
    pub fn get_members(&self) -> BTreeMap<NodeId, BasicNode> {
        let metrics = self.raft.metrics().borrow().clone();
        let membership = metrics.membership_config.membership();
        membership
            .nodes()
            .map(|(id, node)| (*id, node.clone()))
            .collect()
    }

    /// Get current voters.
    pub fn get_voters(&self) -> Vec<NodeId> {
        let metrics = self.raft.metrics().borrow().clone();
        let membership = metrics.membership_config.membership();
        membership.voter_ids().collect()
    }

    /// Get current learners.
    pub fn get_learners(&self) -> Vec<NodeId> {
        let metrics = self.raft.metrics().borrow().clone();
        let membership = metrics.membership_config.membership();
        membership.learner_ids().collect()
    }

    /// Check if a node is part of the cluster.
    pub fn contains(&self, node_id: NodeId) -> bool {
        self.get_members().contains_key(&node_id)
    }

    /// Check if a node is a voter.
    pub fn is_voter(&self, node_id: NodeId) -> bool {
        self.get_voters().contains(&node_id)
    }

    /// Check if a node is a learner.
    pub fn is_learner(&self, node_id: NodeId) -> bool {
        self.get_learners().contains(&node_id)
    }

    /// Get the cluster size (voters only).
    pub fn cluster_size(&self) -> usize {
        self.get_voters().len()
    }

    /// Check if the cluster has quorum.
    ///
    /// A cluster has quorum if more than half of the voters are available.
    pub fn has_quorum(&self, available_nodes: &[NodeId]) -> bool {
        let voters = self.get_voters();
        let available_voters = voters
            .iter()
            .filter(|v| available_nodes.contains(v))
            .count();

        available_voters > voters.len() / 2
    }
}

/// Membership change request.
#[derive(Debug, Clone)]
pub enum MembershipChange {
    /// Add a new node as a learner.
    AddLearner { node_id: NodeId, addr: String },
    /// Promote a learner to voter.
    PromoteToVoter { node_id: NodeId },
    /// Remove a node from the cluster.
    RemoveNode { node_id: NodeId },
}

impl MembershipChange {
    /// Create an add learner change.
    pub fn add_learner(node_id: NodeId, addr: impl Into<String>) -> Self {
        MembershipChange::AddLearner {
            node_id,
            addr: addr.into(),
        }
    }

    /// Create a promote to voter change.
    pub fn promote_to_voter(node_id: NodeId) -> Self {
        MembershipChange::PromoteToVoter { node_id }
    }

    /// Create a remove node change.
    pub fn remove_node(node_id: NodeId) -> Self {
        MembershipChange::RemoveNode { node_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_membership_change_creation() {
        let add = MembershipChange::add_learner(4, "node4:9001");
        match add {
            MembershipChange::AddLearner { node_id, addr } => {
                assert_eq!(node_id, 4);
                assert_eq!(addr, "node4:9001");
            }
            _ => panic!("Expected AddLearner"),
        }

        let promote = MembershipChange::promote_to_voter(4);
        assert!(matches!(
            promote,
            MembershipChange::PromoteToVoter { node_id: 4 }
        ));

        let remove = MembershipChange::remove_node(4);
        assert!(matches!(
            remove,
            MembershipChange::RemoveNode { node_id: 4 }
        ));
    }

    #[test]
    fn test_quorum_calculation() {
        // For a 3-node cluster, need 2 for quorum
        // For a 5-node cluster, need 3 for quorum

        // This is a simplified test - actual quorum calculation
        // happens in the MembershipManager when it has access to raft
        let voters = vec![1, 2, 3];
        let available = vec![1, 2];
        let available_count = voters.iter().filter(|v| available.contains(v)).count();
        assert!(available_count > voters.len() / 2);
    }
}
