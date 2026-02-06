//! Raft RPC message types.
//!
//! Note: These messages use serde for serialization since openraft types
//! don't implement rkyv. We use JSON for simplicity and compatibility.

use openraft::{LogId, Vote};
use serde::{Deserialize, Serialize};

use crate::types::{LogEntry, NodeId, StoredMembership};

/// Raft RPC message wrapper.
///
/// All Raft communication between nodes uses this message format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Vote request (RequestVote RPC).
    VoteRequest(VoteRequest),
    /// Vote response.
    VoteResponse(VoteResponse),
    /// AppendEntries request.
    AppendEntriesRequest(AppendEntriesRequest),
    /// AppendEntries response.
    AppendEntriesResponse(AppendEntriesResponse),
    /// InstallSnapshot request (first chunk or metadata).
    InstallSnapshotRequest(InstallSnapshotRequest),
    /// InstallSnapshot response.
    InstallSnapshotResponse(InstallSnapshotResponse),
}

/// Vote request message (RequestVote RPC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    /// The vote being requested.
    pub vote: Vote<NodeId>,
    /// Last log ID of the candidate.
    pub last_log_id: Option<LogId<NodeId>>,
}

impl VoteRequest {
    /// Create a new vote request.
    pub fn new(vote: Vote<NodeId>, last_log_id: Option<LogId<NodeId>>) -> Self {
        Self { vote, last_log_id }
    }
}

/// Vote response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    /// The vote of the responder.
    pub vote: Vote<NodeId>,
    /// Whether the vote was granted.
    pub vote_granted: bool,
    /// Last log ID of the responder.
    pub last_log_id: Option<LogId<NodeId>>,
}

impl VoteResponse {
    /// Create a new vote response.
    pub fn new(
        vote: Vote<NodeId>,
        vote_granted: bool,
        last_log_id: Option<LogId<NodeId>>,
    ) -> Self {
        Self {
            vote,
            vote_granted,
            last_log_id,
        }
    }
}

/// AppendEntries request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    /// The vote of the leader.
    pub vote: Vote<NodeId>,
    /// Log ID of the entry preceding the new entries.
    pub prev_log_id: Option<LogId<NodeId>>,
    /// New entries to append.
    pub entries: Vec<LogEntry>,
    /// Leader's commit index.
    pub leader_commit: Option<LogId<NodeId>>,
}

impl AppendEntriesRequest {
    /// Create a new AppendEntries request.
    pub fn new(
        vote: Vote<NodeId>,
        prev_log_id: Option<LogId<NodeId>>,
        entries: Vec<LogEntry>,
        leader_commit: Option<LogId<NodeId>>,
    ) -> Self {
        Self {
            vote,
            prev_log_id,
            entries,
            leader_commit,
        }
    }

    /// Create a heartbeat request (no entries).
    pub fn heartbeat(
        vote: Vote<NodeId>,
        prev_log_id: Option<LogId<NodeId>>,
        leader_commit: Option<LogId<NodeId>>,
    ) -> Self {
        Self::new(vote, prev_log_id, vec![], leader_commit)
    }
}

/// AppendEntries response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    /// The vote of the responder.
    pub vote: Vote<NodeId>,
    /// Whether the append was successful.
    pub success: bool,
    /// Conflicting log ID if append failed.
    pub conflict: Option<LogId<NodeId>>,
}

impl AppendEntriesResponse {
    /// Create a successful response.
    pub fn success(vote: Vote<NodeId>) -> Self {
        Self {
            vote,
            success: true,
            conflict: None,
        }
    }

    /// Create a failure response with conflict information.
    pub fn conflict(vote: Vote<NodeId>, conflict: Option<LogId<NodeId>>) -> Self {
        Self {
            vote,
            success: false,
            conflict,
        }
    }
}

/// InstallSnapshot request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    /// The vote of the leader.
    pub vote: Vote<NodeId>,
    /// Snapshot metadata.
    pub meta: NetworkSnapshotMeta,
    /// Byte offset of this chunk.
    pub offset: u64,
    /// Snapshot data chunk.
    pub data: Vec<u8>,
    /// Whether this is the last chunk.
    pub done: bool,
}

impl InstallSnapshotRequest {
    /// Create a new InstallSnapshot request.
    pub fn new(
        vote: Vote<NodeId>,
        meta: NetworkSnapshotMeta,
        offset: u64,
        data: Vec<u8>,
        done: bool,
    ) -> Self {
        Self {
            vote,
            meta,
            offset,
            data,
            done,
        }
    }
}

/// Snapshot metadata for network transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSnapshotMeta {
    /// Last log ID included in the snapshot.
    pub last_log_id: Option<LogId<NodeId>>,
    /// Membership configuration at snapshot time.
    pub last_membership: StoredMembership,
    /// Unique snapshot identifier.
    pub snapshot_id: String,
}

impl NetworkSnapshotMeta {
    /// Create new snapshot metadata.
    pub fn new(
        last_log_id: Option<LogId<NodeId>>,
        last_membership: StoredMembership,
        snapshot_id: impl Into<String>,
    ) -> Self {
        Self {
            last_log_id,
            last_membership,
            snapshot_id: snapshot_id.into(),
        }
    }
}

/// InstallSnapshot response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    /// The vote of the responder.
    pub vote: Vote<NodeId>,
}

impl InstallSnapshotResponse {
    /// Create a new InstallSnapshot response.
    pub fn new(vote: Vote<NodeId>) -> Self {
        Self { vote }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn test_vote_request_serialization() {
        let vote = Vote::new(1, 5);
        let last_log_id = Some(LogId::new(openraft::CommittedLeaderId::new(1, 1), 10));
        let request = VoteRequest::new(vote, last_log_id);

        let json = serde_json::to_string(&RaftMessage::VoteRequest(request.clone())).unwrap();
        let deserialized: RaftMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            RaftMessage::VoteRequest(req) => {
                assert_eq!(req.vote, vote);
                assert_eq!(req.last_log_id, last_log_id);
            }
            _ => panic!("Expected VoteRequest"),
        }
    }

    #[test]
    fn test_append_entries_serialization() {
        let vote = Vote::new(1, 5);
        let request = AppendEntriesRequest::heartbeat(vote, None, None);

        let json = serde_json::to_string(&RaftMessage::AppendEntriesRequest(request)).unwrap();
        let deserialized: RaftMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            RaftMessage::AppendEntriesRequest(req) => {
                assert_eq!(req.vote, vote);
                assert!(req.entries.is_empty());
            }
            _ => panic!("Expected AppendEntriesRequest"),
        }
    }

    #[test]
    fn test_snapshot_meta() {
        let membership = openraft::Membership::new(vec![BTreeSet::from([1, 2, 3])], None);
        let stored_membership = openraft::StoredMembership::new(None, membership);
        let meta = NetworkSnapshotMeta::new(
            Some(LogId::new(openraft::CommittedLeaderId::new(1, 1), 100)),
            stored_membership,
            "snap-100-12345",
        );

        assert_eq!(meta.snapshot_id, "snap-100-12345");
        assert_eq!(meta.last_log_id.unwrap().index, 100);
    }
}
