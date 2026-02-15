//! Raft RPC server for handling incoming requests.

use std::sync::Arc;

use nng::options::Options;
use nng::{Protocol, Socket};
use openraft::raft::{
    AppendEntriesRequest as OpenraftAppendRequest, VoteRequest as OpenraftVoteRequest,
};
use tokio::sync::oneshot;

use crate::config::RaftTlsConfig;
use crate::error::RaftError;
use crate::network::messages::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotResponse, RaftMessage,
    VoteRequest, VoteResponse,
};
use crate::types::{NodeId, OrmdbRaft, TypeConfig};

/// Raft RPC server that handles incoming Raft protocol messages.
///
/// This server listens on an NNG REP socket and dispatches incoming
/// Raft RPCs to the local Raft instance.
pub struct RaftTransport {
    /// This node's ID.
    node_id: NodeId,
    /// Listen address.
    listen_addr: String,
    /// The Raft instance to dispatch to.
    raft: Arc<OrmdbRaft>,
    /// TLS configuration.
    tls_config: Option<RaftTlsConfig>,
}

impl RaftTransport {
    /// Create a new Raft transport server.
    pub fn new(node_id: NodeId, listen_addr: impl Into<String>, raft: Arc<OrmdbRaft>) -> Self {
        Self {
            node_id,
            listen_addr: listen_addr.into(),
            raft,
            tls_config: None,
        }
    }

    /// Create a new Raft transport server with TLS.
    pub fn with_tls(
        node_id: NodeId,
        listen_addr: impl Into<String>,
        raft: Arc<OrmdbRaft>,
        tls_config: RaftTlsConfig,
    ) -> Self {
        Self {
            node_id,
            listen_addr: listen_addr.into(),
            raft,
            tls_config: if tls_config.enabled { Some(tls_config) } else { None },
        }
    }

    /// Check if TLS is enabled.
    pub fn is_tls_enabled(&self) -> bool {
        self.tls_config.as_ref().map(|c| c.enabled).unwrap_or(false)
    }

    /// Configure TLS on the socket.
    fn configure_tls(&self, socket: &Socket) -> Result<(), RaftError> {
        if let Some(ref tls) = self.tls_config {
            if tls.enabled {
                // Set certificate and key
                if let (Some(cert_path), Some(key_path)) = (&tls.cert_path, &tls.key_path) {
                    let cert_key_path = format!("{}:{}", cert_path.display(), key_path.display());
                    socket
                        .set_opt::<nng::options::transport::tls::CertKeyFile>(cert_key_path)
                        .map_err(|e| RaftError::Network(format!("Failed to set TLS cert/key: {}", e)))?;
                }

                // Set CA certificate for peer verification
                if let Some(ca_path) = &tls.ca_path {
                    socket
                        .set_opt::<nng::options::transport::tls::CaFile>(ca_path.display().to_string())
                        .map_err(|e| RaftError::Network(format!("Failed to set TLS CA: {}", e)))?;
                }
            }
        }
        Ok(())
    }

    /// Run the transport server synchronously.
    ///
    /// This blocks the calling thread. Use `spawn_transport` for async usage.
    pub fn run_sync(self, mut shutdown_rx: oneshot::Receiver<()>) -> Result<(), RaftError> {
        let socket = Socket::new(Protocol::Rep0)
            .map_err(|e| RaftError::Network(format!("Failed to create socket: {}", e)))?;

        // Configure TLS if enabled
        self.configure_tls(&socket)?;

        // Use TLS scheme if enabled
        let addr = if self.is_tls_enabled() {
            format!("tls+tcp://{}", self.listen_addr)
        } else {
            format!("tcp://{}", self.listen_addr)
        };
        socket
            .listen(&addr)
            .map_err(|e| RaftError::Network(format!("Failed to listen on {}: {}", addr, e)))?;

        tracing::info!(
            "Raft transport server started on {} for node {} (TLS: {})",
            addr,
            self.node_id,
            self.is_tls_enabled()
        );

        let raft = self.raft.clone();
        let node_id = self.node_id;

        // Create a runtime for handling async raft calls
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| RaftError::Internal(format!("Failed to create runtime: {}", e)))?;

        // Main request loop
        loop {
            // Check for shutdown using try_recv
            match shutdown_rx.try_recv() {
                Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    tracing::info!("Raft transport shutting down for node {}", node_id);
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Not shut down yet, continue
                }
            }

            // Receive request with timeout
            let request = match socket.recv() {
                Ok(msg) => msg,
                Err(nng::Error::TimedOut) => continue,
                Err(e) => {
                    tracing::error!("Failed to receive message: {}", e);
                    continue;
                }
            };

            // Parse the message
            let raft_msg: RaftMessage = match serde_json::from_slice(request.as_slice()) {
                Ok(msg) => msg,
                Err(e) => {
                    tracing::error!("Failed to parse Raft message: {}", e);
                    continue;
                }
            };

            // Handle the message
            let response = rt.block_on(handle_message(&raft, raft_msg));

            // Serialize response
            let response_bytes = match serde_json::to_vec(&response) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Failed to serialize response: {}", e);
                    continue;
                }
            };

            // Send response
            let reply = nng::Message::from(response_bytes.as_slice());
            if let Err((_, e)) = socket.send(reply) {
                tracing::error!("Failed to send response: {}", e);
            }
        }

        Ok(())
    }
}

/// Handle a Raft message and return the response.
async fn handle_message(raft: &OrmdbRaft, msg: RaftMessage) -> RaftMessage {
    match msg {
        RaftMessage::VoteRequest(req) => handle_vote_request(raft, req).await,
        RaftMessage::AppendEntriesRequest(req) => handle_append_entries(raft, req).await,
        RaftMessage::InstallSnapshotRequest(req) => handle_install_snapshot(raft, req).await,
        // Response messages shouldn't be received by the server
        _ => {
            tracing::warn!("Received unexpected message type");
            RaftMessage::VoteResponse(VoteResponse::new(
                openraft::Vote::default(),
                false,
                None,
            ))
        }
    }
}

/// Handle a vote request.
async fn handle_vote_request(raft: &OrmdbRaft, req: VoteRequest) -> RaftMessage {
    let openraft_req = OpenraftVoteRequest {
        vote: req.vote,
        last_log_id: req.last_log_id,
    };

    match raft.vote(openraft_req).await {
        Ok(resp) => RaftMessage::VoteResponse(VoteResponse::new(
            resp.vote,
            resp.vote_granted,
            resp.last_log_id,
        )),
        Err(e) => {
            tracing::error!("Vote request failed: {}", e);
            RaftMessage::VoteResponse(VoteResponse::new(
                openraft::Vote::default(),
                false,
                None,
            ))
        }
    }
}

/// Handle an AppendEntries request.
async fn handle_append_entries(raft: &OrmdbRaft, req: AppendEntriesRequest) -> RaftMessage {
    use openraft::raft::AppendEntriesResponse as OpenraftAppendResponse;

    let openraft_req: OpenraftAppendRequest<TypeConfig> = OpenraftAppendRequest {
        vote: req.vote,
        prev_log_id: req.prev_log_id,
        entries: req.entries,
        leader_commit: req.leader_commit,
    };

    match raft.append_entries(openraft_req).await {
        Ok(resp) => {
            // Convert openraft enum response to our struct format
            let (success, conflict) = match resp {
                OpenraftAppendResponse::Success => (true, None),
                OpenraftAppendResponse::PartialSuccess(log_id) => (true, log_id),
                OpenraftAppendResponse::Conflict => (false, None),
                OpenraftAppendResponse::HigherVote(vote) => {
                    // Higher vote means we need to step down, return conflict with the vote
                    return RaftMessage::AppendEntriesResponse(AppendEntriesResponse {
                        vote,
                        success: false,
                        conflict: None,
                    });
                }
            };
            RaftMessage::AppendEntriesResponse(AppendEntriesResponse {
                vote: req.vote, // Use the request vote since response doesn't contain it
                success,
                conflict,
            })
        },
        Err(e) => {
            tracing::error!("AppendEntries request failed: {}", e);
            RaftMessage::AppendEntriesResponse(AppendEntriesResponse::conflict(
                openraft::Vote::default(),
                None,
            ))
        }
    }
}

/// Handle an InstallSnapshot request.
async fn handle_install_snapshot(
    _raft: &OrmdbRaft,
    req: crate::network::messages::InstallSnapshotRequest,
) -> RaftMessage {
    // For now, just acknowledge the snapshot chunk
    // A full implementation would buffer chunks and install when complete
    tracing::debug!(
        "Received snapshot chunk: offset={}, size={}, done={}",
        req.offset,
        req.data.len(),
        req.done
    );

    RaftMessage::InstallSnapshotResponse(InstallSnapshotResponse::new(req.vote))
}

/// Spawn the Raft transport server as a background task.
pub fn spawn_transport(
    node_id: NodeId,
    listen_addr: impl Into<String>,
    raft: Arc<OrmdbRaft>,
) -> (
    tokio::task::JoinHandle<Result<(), RaftError>>,
    oneshot::Sender<()>,
) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let listen_addr = listen_addr.into();

    let handle = tokio::task::spawn_blocking(move || {
        let transport = RaftTransport::new(node_id, listen_addr, raft);
        transport.run_sync(shutdown_rx)
    });

    (handle, shutdown_tx)
}

/// Spawn the Raft transport server as a background task with TLS.
pub fn spawn_transport_with_tls(
    node_id: NodeId,
    listen_addr: impl Into<String>,
    raft: Arc<OrmdbRaft>,
    tls_config: RaftTlsConfig,
) -> (
    tokio::task::JoinHandle<Result<(), RaftError>>,
    oneshot::Sender<()>,
) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let listen_addr = listen_addr.into();

    let handle = tokio::task::spawn_blocking(move || {
        let transport = RaftTransport::with_tls(node_id, listen_addr, raft, tls_config);
        transport.run_sync(shutdown_rx)
    });

    (handle, shutdown_tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::messages::VoteRequest;

    #[test]
    fn test_parse_message() {
        let vote = openraft::Vote::new(1, 5);
        let msg = RaftMessage::VoteRequest(VoteRequest::new(vote, None));

        let bytes = serde_json::to_vec(&msg).unwrap();
        let parsed: RaftMessage = serde_json::from_slice(&bytes).unwrap();

        match parsed {
            RaftMessage::VoteRequest(req) => {
                assert_eq!(req.vote, vote);
            }
            _ => panic!("Expected VoteRequest"),
        }
    }
}
