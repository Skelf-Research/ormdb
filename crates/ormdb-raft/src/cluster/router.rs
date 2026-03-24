//! Request routing and leader forwarding.

use std::sync::Arc;
use std::time::Duration;

use nng::options::{Options, RecvTimeout, SendTimeout};
use nng::{Protocol, Socket};
use tokio::sync::RwLock;

use crate::cluster::manager::RaftClusterManager;
use crate::error::RaftError;
use crate::types::NodeId;

/// Routes client requests to the current leader.
///
/// When a follower receives a write request, this router forwards
/// it to the current leader.
pub struct RequestRouter {
    /// The cluster manager.
    manager: Arc<RaftClusterManager>,
    /// Cached leader connection.
    leader_conn: RwLock<Option<LeaderConnection>>,
    /// Request timeout.
    timeout: Duration,
}

/// Cached connection to the leader.
struct LeaderConnection {
    /// Leader node ID.
    node_id: NodeId,
    /// Leader address.
    addr: String,
    /// NNG socket.
    socket: Socket,
}

impl RequestRouter {
    /// Create a new request router.
    pub fn new(manager: Arc<RaftClusterManager>) -> Self {
        Self {
            manager,
            leader_conn: RwLock::new(None),
            timeout: Duration::from_secs(10),
        }
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.manager.is_leader()
    }

    /// Get the current leader information.
    pub async fn get_leader(&self) -> Option<(NodeId, String)> {
        self.manager.get_leader().await
    }

    /// Forward a request to the leader.
    ///
    /// Returns the raw response bytes from the leader.
    pub async fn forward_to_leader(&self, request: &[u8]) -> Result<Vec<u8>, RaftError> {
        // Get current leader
        let (leader_id, leader_addr) = self.manager.get_leader().await.ok_or(RaftError::NoLeader)?;

        // Check if we need to reconnect
        let needs_reconnect = {
            let conn = self.leader_conn.read().await;
            match &*conn {
                Some(c) => c.node_id != leader_id,
                None => true,
            }
        };

        if needs_reconnect {
            // Create new connection to leader
            let socket = Socket::new(Protocol::Req0)
                .map_err(|e| RaftError::Network(format!("Failed to create socket: {}", e)))?;

            socket
                .set_opt::<SendTimeout>(Some(self.timeout))
                .map_err(|e| RaftError::Network(format!("Failed to set timeout: {}", e)))?;
            socket
                .set_opt::<RecvTimeout>(Some(self.timeout))
                .map_err(|e| RaftError::Network(format!("Failed to set timeout: {}", e)))?;

            // Connect to leader's client port (not Raft port)
            let addr = format!("tcp://{}", leader_addr);
            socket
                .dial(&addr)
                .map_err(|e| RaftError::Network(format!("Failed to connect to {}: {}", addr, e)))?;

            *self.leader_conn.write().await = Some(LeaderConnection {
                node_id: leader_id,
                addr: leader_addr,
                socket,
            });
        }

        // Send request
        let conn = self.leader_conn.read().await;
        let conn = conn.as_ref().unwrap();

        let msg = nng::Message::from(request);
        conn.socket
            .send(msg)
            .map_err(|(_, e)| RaftError::Network(format!("Failed to send: {}", e)))?;

        // Receive response
        let response = conn
            .socket
            .recv()
            .map_err(|e| RaftError::Network(format!("Failed to receive: {}", e)))?;

        Ok(response.to_vec())
    }

    /// Clear the cached leader connection.
    ///
    /// This should be called when a connection error occurs.
    pub async fn clear_leader_cache(&self) {
        *self.leader_conn.write().await = None;
    }
}

/// Response from leader forwarding.
#[derive(Debug)]
pub enum ForwardResult {
    /// Request was handled locally (we are the leader).
    Local,
    /// Request was forwarded to the leader.
    Forwarded { response: Vec<u8> },
    /// No leader available.
    NoLeader,
}

impl ForwardResult {
    /// Check if this was handled locally.
    pub fn is_local(&self) -> bool {
        matches!(self, ForwardResult::Local)
    }

    /// Check if this was forwarded.
    pub fn is_forwarded(&self) -> bool {
        matches!(self, ForwardResult::Forwarded { .. })
    }

    /// Get the forwarded response, if any.
    pub fn forwarded_response(&self) -> Option<&[u8]> {
        match self {
            ForwardResult::Forwarded { response } => Some(response),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_result() {
        let local = ForwardResult::Local;
        assert!(local.is_local());
        assert!(!local.is_forwarded());
        assert!(local.forwarded_response().is_none());

        let forwarded = ForwardResult::Forwarded {
            response: vec![1, 2, 3],
        };
        assert!(!forwarded.is_local());
        assert!(forwarded.is_forwarded());
        assert_eq!(forwarded.forwarded_response(), Some([1, 2, 3].as_slice()));

        let no_leader = ForwardResult::NoLeader;
        assert!(!no_leader.is_local());
        assert!(!no_leader.is_forwarded());
    }
}
