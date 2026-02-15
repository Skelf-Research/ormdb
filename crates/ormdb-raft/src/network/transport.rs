//! NNG-based Raft network transport.

use std::future::Future;
use std::time::Duration;

use nng::options::{Options, RecvTimeout, SendTimeout};
use nng::{Protocol, Socket};
use openraft::error::{NetworkError, RPCError, RaftError, ReplicationClosed, Unreachable};
use openraft::network::{RPCOption, RaftNetwork};
use openraft::raft::{
    AppendEntriesRequest as OpenraftAppendRequest, AppendEntriesResponse as OpenraftAppendResponse,
    SnapshotResponse, VoteRequest as OpenraftVoteRequest, VoteResponse as OpenraftVoteResponse,
};
use openraft::storage::Snapshot;
use openraft::{BasicNode, Vote};

use crate::config::RaftTlsConfig;
use crate::error::RaftError as OrmdbRaftError;
use crate::network::messages::{
    AppendEntriesRequest, InstallSnapshotRequest, NetworkSnapshotMeta, RaftMessage,
    VoteRequest,
};
use crate::types::{NodeId, TypeConfig};

/// NNG-based Raft network transport.
///
/// This implements openraft's `RaftNetwork` trait using NNG sockets
/// for communication between cluster nodes.
pub struct NngRaftNetwork {
    /// Target node information.
    target: BasicNode,
    /// Target node ID.
    pub target_id: NodeId,
    /// Request timeout.
    timeout: Duration,
    /// TLS configuration.
    tls_config: Option<RaftTlsConfig>,
}

impl NngRaftNetwork {
    /// Create a new NNG network connection to the target node.
    pub fn new(target_id: NodeId, target: BasicNode) -> Self {
        Self {
            target,
            target_id,
            timeout: Duration::from_secs(5),
            tls_config: None,
        }
    }

    /// Create a new NNG network connection with TLS.
    pub fn with_tls(target_id: NodeId, target: BasicNode, tls_config: RaftTlsConfig) -> Self {
        Self {
            target,
            target_id,
            timeout: Duration::from_secs(5),
            tls_config: if tls_config.enabled { Some(tls_config) } else { None },
        }
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if TLS is enabled.
    pub fn is_tls_enabled(&self) -> bool {
        self.tls_config.as_ref().map(|c| c.enabled).unwrap_or(false)
    }

    /// Get the target's Raft address.
    fn raft_addr(&self) -> String {
        if self.is_tls_enabled() {
            format!("tls+tcp://{}", self.target.addr)
        } else {
            format!("tcp://{}", self.target.addr)
        }
    }

    /// Configure TLS on the socket.
    fn configure_tls(&self, socket: &Socket) -> Result<(), OrmdbRaftError> {
        if let Some(ref tls) = self.tls_config {
            if tls.enabled {
                // Set certificate and key
                if let (Some(cert_path), Some(key_path)) = (&tls.cert_path, &tls.key_path) {
                    let cert_key_path = format!("{}:{}", cert_path.display(), key_path.display());
                    socket
                        .set_opt::<nng::options::transport::tls::CertKeyFile>(cert_key_path)
                        .map_err(|e| OrmdbRaftError::Network(format!("Failed to set TLS cert/key: {}", e)))?;
                }

                // Set CA certificate for peer verification
                if let Some(ca_path) = &tls.ca_path {
                    socket
                        .set_opt::<nng::options::transport::tls::CaFile>(ca_path.display().to_string())
                        .map_err(|e| OrmdbRaftError::Network(format!("Failed to set TLS CA: {}", e)))?;
                }
            }
        }
        Ok(())
    }

    /// Send a message and receive response synchronously.
    fn send_message_sync(&self, msg: &RaftMessage) -> Result<RaftMessage, OrmdbRaftError> {
        // Create REQ socket for RPC
        let socket = Socket::new(Protocol::Req0)
            .map_err(|e| OrmdbRaftError::Network(format!("Failed to create socket: {}", e)))?;

        // Configure TLS if enabled
        self.configure_tls(&socket)?;

        // Set timeouts
        socket
            .set_opt::<SendTimeout>(Some(self.timeout))
            .map_err(|e| OrmdbRaftError::Network(format!("Failed to set send timeout: {}", e)))?;
        socket
            .set_opt::<RecvTimeout>(Some(self.timeout))
            .map_err(|e| OrmdbRaftError::Network(format!("Failed to set recv timeout: {}", e)))?;

        // Connect to target node's Raft port
        let addr = self.raft_addr();
        socket
            .dial(&addr)
            .map_err(|e| OrmdbRaftError::Network(format!("Failed to connect to {}: {}", addr, e)))?;

        // Serialize message using serde_json
        let payload = serde_json::to_vec(msg)
            .map_err(|e| OrmdbRaftError::Serialization(e.to_string()))?;

        // Send request
        let request = nng::Message::from(payload.as_slice());
        socket
            .send(request)
            .map_err(|(_, e)| OrmdbRaftError::Network(format!("Send failed: {}", e)))?;

        // Receive response
        let response = socket
            .recv()
            .map_err(|e| OrmdbRaftError::Network(format!("Recv failed: {}", e)))?;

        // Deserialize response
        let response_msg: RaftMessage = serde_json::from_slice(response.as_slice())
            .map_err(|e| OrmdbRaftError::Serialization(e.to_string()))?;

        Ok(response_msg)
    }

    /// Send a message asynchronously by wrapping the sync call.
    async fn send_message(&self, msg: RaftMessage) -> Result<RaftMessage, OrmdbRaftError> {
        let target = self.target.clone();
        let target_id = self.target_id;
        let timeout = self.timeout;
        let tls_config = self.tls_config.clone();

        tokio::task::spawn_blocking(move || {
            let network = NngRaftNetwork {
                target,
                target_id,
                timeout,
                tls_config,
            };
            network.send_message_sync(&msg)
        })
        .await
        .map_err(|e| OrmdbRaftError::Network(format!("Task join failed: {}", e)))?
    }
}

impl RaftNetwork<TypeConfig> for NngRaftNetwork {
    async fn vote(
        &mut self,
        rpc: OpenraftVoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<OpenraftVoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let msg = RaftMessage::VoteRequest(VoteRequest::new(rpc.vote, rpc.last_log_id));

        match self.send_message(msg).await {
            Ok(RaftMessage::VoteResponse(resp)) => Ok(OpenraftVoteResponse {
                vote: resp.vote,
                vote_granted: resp.vote_granted,
                last_log_id: resp.last_log_id,
            }),
            Ok(_) => Err(RPCError::Network(NetworkError::new(&std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unexpected response type",
            )))),
            Err(e) => {
                tracing::warn!("Vote RPC to node {} failed: {}", self.target_id, e);
                Err(RPCError::Unreachable(Unreachable::new(&e)))
            }
        }
    }

    async fn append_entries(
        &mut self,
        rpc: OpenraftAppendRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<OpenraftAppendResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let msg = RaftMessage::AppendEntriesRequest(AppendEntriesRequest::new(
            rpc.vote,
            rpc.prev_log_id,
            rpc.entries,
            rpc.leader_commit,
        ));

        match self.send_message(msg).await {
            Ok(RaftMessage::AppendEntriesResponse(resp)) => {
                // Convert our response format to openraft's enum format
                if resp.success {
                    Ok(OpenraftAppendResponse::Success)
                } else if let Some(conflict) = resp.conflict {
                    // If there was a conflict, report it
                    Ok(OpenraftAppendResponse::Conflict)
                } else {
                    // No conflict info, treat as higher vote
                    Ok(OpenraftAppendResponse::HigherVote(resp.vote))
                }
            }
            Ok(_) => Err(RPCError::Network(NetworkError::new(&std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unexpected response type",
            )))),
            Err(e) => {
                tracing::warn!(
                    "AppendEntries RPC to node {} failed: {}",
                    self.target_id,
                    e
                );
                Err(RPCError::Unreachable(Unreachable::new(&e)))
            }
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: openraft::raft::InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        openraft::raft::InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, openraft::error::InstallSnapshotError>>,
    > {
        // Convert openraft snapshot request to our network format
        let meta = NetworkSnapshotMeta::new(
            rpc.meta.last_log_id,
            rpc.meta.last_membership.clone(),
            rpc.meta.snapshot_id.clone(),
        );

        let msg = RaftMessage::InstallSnapshotRequest(InstallSnapshotRequest::new(
            rpc.vote,
            meta,
            rpc.offset,
            rpc.data.clone(),
            rpc.done,
        ));

        match self.send_message(msg).await {
            Ok(RaftMessage::InstallSnapshotResponse(resp)) => {
                Ok(openraft::raft::InstallSnapshotResponse { vote: resp.vote })
            }
            Ok(_) => Err(RPCError::Network(NetworkError::new(&std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unexpected response type",
            )))),
            Err(e) => {
                tracing::warn!("InstallSnapshot RPC to node {} failed: {}", self.target_id, e);
                Err(RPCError::Unreachable(Unreachable::new(&e)))
            }
        }
    }

    async fn full_snapshot(
        &mut self,
        vote: Vote<NodeId>,
        snapshot: Snapshot<TypeConfig>,
        _cancel: impl Future<Output = ReplicationClosed> + Send + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<NodeId>, openraft::error::StreamingError<TypeConfig, openraft::error::Fatal<NodeId>>> {
        // Convert openraft snapshot meta to our network format
        let meta = NetworkSnapshotMeta::new(
            snapshot.meta.last_log_id,
            snapshot.meta.last_membership.clone(),
            snapshot.meta.snapshot_id.clone(),
        );

        // Get snapshot data
        let data = snapshot.snapshot.into_inner();

        // Send snapshot in chunks
        let chunk_size = 1024 * 1024; // 1MB chunks

        for (i, chunk) in data.chunks(chunk_size).enumerate() {
            let is_last = (i + 1) * chunk_size >= data.len();
            let offset = (i * chunk_size) as u64;

            let msg = RaftMessage::InstallSnapshotRequest(InstallSnapshotRequest::new(
                vote,
                meta.clone(),
                offset,
                chunk.to_vec(),
                is_last,
            ));

            match self.send_message(msg).await {
                Ok(RaftMessage::InstallSnapshotResponse(_)) => {
                    // Continue to next chunk
                }
                Ok(_) => {
                    return Err(openraft::error::StreamingError::Network(NetworkError::new(
                        &std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Unexpected response type",
                        ),
                    )));
                }
                Err(e) => {
                    tracing::warn!("Snapshot RPC to node {} failed: {}", self.target_id, e);
                    return Err(openraft::error::StreamingError::Unreachable(
                        Unreachable::new(&e),
                    ));
                }
            }
        }

        Ok(SnapshotResponse { vote })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_raft_addr() {
        let network = NngRaftNetwork::new(
            1,
            BasicNode {
                addr: "192.168.1.10:9001".to_string(),
            },
        );
        assert_eq!(network.raft_addr(), "tcp://192.168.1.10:9001");
    }

    #[test]
    fn test_raft_addr_with_tls() {
        let tls_config = RaftTlsConfig {
            enabled: true,
            cert_path: Some(PathBuf::from("/path/to/cert.pem")),
            key_path: Some(PathBuf::from("/path/to/key.pem")),
            ca_path: None,
            require_client_cert: false,
        };
        let network = NngRaftNetwork::with_tls(
            1,
            BasicNode {
                addr: "192.168.1.10:9001".to_string(),
            },
            tls_config,
        );
        assert_eq!(network.raft_addr(), "tls+tcp://192.168.1.10:9001");
        assert!(network.is_tls_enabled());
    }

    #[test]
    fn test_timeout_configuration() {
        let network = NngRaftNetwork::new(
            1,
            BasicNode {
                addr: "localhost:9001".to_string(),
            },
        )
        .with_timeout(Duration::from_secs(10));

        assert_eq!(network.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_tls_disabled_by_default() {
        let network = NngRaftNetwork::new(
            1,
            BasicNode {
                addr: "localhost:9001".to_string(),
            },
        );
        assert!(!network.is_tls_enabled());
    }
}
