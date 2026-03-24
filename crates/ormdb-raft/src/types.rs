//! Core type definitions for openraft integration.

use openraft::BasicNode;
use ormdb_proto::{Mutation, MutationBatch, MutationResult};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// Node identifier type.
pub type NodeId = u64;

/// Type alias for the openraft Raft instance with our configuration.
pub type OrmdbRaft = openraft::Raft<TypeConfig>;

/// Type alias for log entry.
pub type LogEntry = openraft::Entry<TypeConfig>;

/// Type alias for log ID.
pub type LogId = openraft::LogId<NodeId>;

/// Type alias for vote.
pub type Vote = openraft::Vote<NodeId>;

/// Type alias for membership - uses NodeId and BasicNode directly.
pub type Membership = openraft::Membership<NodeId, BasicNode>;

/// Type alias for stored membership.
pub type StoredMembership = openraft::StoredMembership<NodeId, BasicNode>;

/// Type alias for snapshot metadata.
pub type SnapshotMeta = openraft::SnapshotMeta<NodeId, BasicNode>;

/// Type configuration for openraft.
///
/// This defines all the associated types that openraft needs.
openraft::declare_raft_types!(
    /// ORMDB Raft type configuration.
    pub TypeConfig:
        D = ClientRequest,
        R = ClientResponse,
        NodeId = NodeId,
        Node = BasicNode,
        Entry = openraft::Entry<TypeConfig>,
        SnapshotData = Cursor<Vec<u8>>,
);

/// Client request to be replicated through Raft.
///
/// These are the operations that get written to the Raft log and applied to
/// all nodes in the cluster.
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize,
)]
pub enum ClientRequest {
    /// Single mutation operation.
    Mutate(Mutation),

    /// Batch of mutations (atomic).
    MutateBatch(MutationBatch),

    /// No-op for leadership confirmation.
    ///
    /// The leader commits a no-op after election to establish its commit index.
    Noop,
}

impl ClientRequest {
    /// Create a mutate request.
    pub fn mutate(mutation: Mutation) -> Self {
        ClientRequest::Mutate(mutation)
    }

    /// Create a batch mutate request.
    pub fn mutate_batch(batch: MutationBatch) -> Self {
        ClientRequest::MutateBatch(batch)
    }

    /// Create a no-op request.
    pub fn noop() -> Self {
        ClientRequest::Noop
    }
}

/// Response after applying a client request.
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize,
)]
pub enum ClientResponse {
    /// Result of a mutation operation.
    MutationResult(MutationResult),

    /// Result of a no-op operation.
    NoopResult,

    /// Error during application.
    Error(String),
}

impl ClientResponse {
    /// Create a mutation result response.
    pub fn mutation_result(result: MutationResult) -> Self {
        ClientResponse::MutationResult(result)
    }

    /// Create a no-op result response.
    pub fn noop() -> Self {
        ClientResponse::NoopResult
    }

    /// Create an error response.
    pub fn error(msg: impl Into<String>) -> Self {
        ClientResponse::Error(msg.into())
    }

    /// Check if this is an error response.
    pub fn is_error(&self) -> bool {
        matches!(self, ClientResponse::Error(_))
    }

    /// Get the error message if this is an error response.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ClientResponse::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_proto::FieldValue;

    #[test]
    fn test_client_request_mutate() {
        let mutation = Mutation::insert("User", vec![FieldValue::new("name", "Alice")]);
        let request = ClientRequest::mutate(mutation.clone());

        match request {
            ClientRequest::Mutate(m) => assert_eq!(m, mutation),
            _ => panic!("Expected Mutate variant"),
        }
    }

    #[test]
    fn test_client_request_noop() {
        let request = ClientRequest::noop();
        assert!(matches!(request, ClientRequest::Noop));
    }

    #[test]
    fn test_client_response_mutation_result() {
        let result = MutationResult::inserted([1u8; 16]);
        let response = ClientResponse::mutation_result(result.clone());

        match response {
            ClientResponse::MutationResult(r) => assert_eq!(r, result),
            _ => panic!("Expected MutationResult variant"),
        }
    }

    #[test]
    fn test_client_response_error() {
        let response = ClientResponse::error("something went wrong");

        assert!(response.is_error());
        assert_eq!(response.error_message(), Some("something went wrong"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let request = ClientRequest::mutate(Mutation::insert(
            "User",
            vec![FieldValue::new("name", "Bob")],
        ));

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
        let archived =
            rkyv::access::<ArchivedClientRequest, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: ClientRequest =
            rkyv::deserialize::<ClientRequest, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(request, deserialized);
    }
}
