//! Connection management for ORMDB client.

use std::sync::atomic::{AtomicU64, Ordering};

use async_nng::AsyncContext;
use nng::options::Options;
use nng::{Message, Protocol, Socket};

use ormdb_proto::framing::{encode_frame, extract_payload};
use ormdb_proto::message::ArchivedResponse;
use ormdb_proto::{Handshake, HandshakeResponse, Request, Response};

use crate::config::ClientConfig;
use crate::error::Error;

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection not yet established.
    Disconnected,
    /// Socket connected, handshake not performed.
    Connected,
    /// Handshake completed successfully.
    Ready,
    /// Connection closed.
    Closed,
}

/// A connection to an ORMDB server.
pub struct Connection {
    socket: Socket,
    state: ConnectionState,
    config: ClientConfig,
    server_capabilities: Vec<String>,
    schema_version: AtomicU64,
    server_id: String,
}

impl Connection {
    /// Establish a new connection to the server.
    pub async fn establish(config: ClientConfig) -> Result<Self, Error> {
        // Create REQ socket
        let socket = Socket::new(Protocol::Req0)
            .map_err(|e| Error::Connection(format!("failed to create socket: {}", e)))?;

        // Set socket options
        socket
            .set_opt::<nng::options::RecvMaxSize>(config.max_message_size)
            .map_err(|e| Error::Connection(format!("failed to set max message size: {}", e)))?;

        // Set timeout options
        socket
            .set_opt::<nng::options::SendTimeout>(Some(config.timeout))
            .map_err(|e| Error::Connection(format!("failed to set send timeout: {}", e)))?;
        socket
            .set_opt::<nng::options::RecvTimeout>(Some(config.timeout))
            .map_err(|e| Error::Connection(format!("failed to set recv timeout: {}", e)))?;

        // Connect to server
        socket
            .dial(&config.address)
            .map_err(|e| Error::Connection(format!("failed to connect to {}: {}", config.address, e)))?;

        Ok(Self {
            socket,
            state: ConnectionState::Connected,
            config,
            server_capabilities: Vec::new(),
            schema_version: AtomicU64::new(0),
            server_id: String::new(),
        })
    }

    /// Create an async context for this connection.
    fn create_context(&self) -> Result<AsyncContext<'_>, Error> {
        AsyncContext::try_from(&self.socket)
            .map_err(|e| Error::Connection(format!("failed to create async context: {}", e)))
    }

    /// Perform the protocol handshake with the server.
    pub async fn handshake(&mut self) -> Result<(), Error> {
        if self.state != ConnectionState::Connected {
            return Err(Error::Connection(format!(
                "cannot handshake in state {:?}",
                self.state
            )));
        }

        // Create async context
        let mut ctx = self.create_context()?;

        // Create handshake message
        let handshake = Handshake::new(&self.config.client_id);

        // Serialize and frame
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&handshake)
            .map_err(|e| Error::Protocol(ormdb_proto::Error::Serialization(format!(
                "failed to serialize handshake: {}", e
            ))))?;
        let framed = encode_frame(&payload)?;

        // Send handshake
        let msg = Message::from(framed.as_slice());
        ctx.send(msg, Some(self.config.timeout))
            .await
            .map_err(|(_, e)| match e {
                nng::Error::TimedOut => Error::Timeout,
                _ => Error::Connection(format!("failed to send handshake: {}", e)),
            })?;

        // Receive response
        let response_msg = ctx
            .receive(Some(self.config.timeout))
            .await
            .map_err(|e| match e {
                nng::Error::TimedOut => Error::Timeout,
                _ => Error::Connection(format!("failed to receive handshake response: {}", e)),
            })?;

        // Parse response
        let response_payload = extract_payload(response_msg.as_slice())?;
        let archived = rkyv::access::<ormdb_proto::handshake::ArchivedHandshakeResponse, rkyv::rancor::Error>(
            response_payload,
        )
        .map_err(|e| Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
            "failed to access handshake response: {}", e
        ))))?;

        let response: HandshakeResponse = rkyv::deserialize::<HandshakeResponse, rkyv::rancor::Error>(archived)
            .map_err(|e| Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "failed to deserialize handshake response: {}", e
            ))))?;

        // Check if accepted
        if !response.accepted {
            self.state = ConnectionState::Closed;
            return Err(Error::Connection(format!(
                "handshake rejected: {}",
                response.error.unwrap_or_else(|| "unknown reason".to_string())
            )));
        }

        // Store server info
        self.server_capabilities = response.capabilities;
        self.schema_version.store(response.schema_version, Ordering::SeqCst);
        self.server_id = response.server_id;
        self.state = ConnectionState::Ready;

        Ok(())
    }

    /// Send a request and receive a response.
    pub async fn send_request(&self, request: &Request) -> Result<Response, Error> {
        if self.state != ConnectionState::Ready {
            return Err(Error::Connection(format!(
                "cannot send request in state {:?}",
                self.state
            )));
        }

        // Create async context
        let mut ctx = self.create_context()?;

        // Serialize request
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(request)
            .map_err(|e| Error::Protocol(ormdb_proto::Error::Serialization(format!(
                "failed to serialize request: {}", e
            ))))?;

        // Check message size
        if payload.len() > self.config.max_message_size {
            return Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "request too large: {} bytes (max: {})",
                payload.len(),
                self.config.max_message_size
            ))));
        }

        // Frame and send
        let framed = encode_frame(&payload)?;
        let msg = Message::from(framed.as_slice());
        ctx.send(msg, Some(self.config.timeout))
            .await
            .map_err(|(_, e)| match e {
                nng::Error::TimedOut => Error::Timeout,
                _ => Error::Connection(format!("failed to send request: {}", e)),
            })?;

        // Receive response
        let response_msg = ctx
            .receive(Some(self.config.timeout))
            .await
            .map_err(|e| match e {
                nng::Error::TimedOut => Error::Timeout,
                _ => Error::Connection(format!("failed to receive response: {}", e)),
            })?;

        // Parse response
        let response_payload = extract_payload(response_msg.as_slice())?;
        let archived = rkyv::access::<ArchivedResponse, rkyv::rancor::Error>(response_payload)
            .map_err(|e| Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "failed to access response: {}", e
            ))))?;

        let response: Response = rkyv::deserialize::<Response, rkyv::rancor::Error>(archived)
            .map_err(|e| Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "failed to deserialize response: {}", e
            ))))?;

        // Verify request ID matches
        if response.id != request.id {
            return Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "response ID mismatch: expected {}, got {}",
                request.id, response.id
            ))));
        }

        Ok(response)
    }

    /// Close the connection.
    pub fn close(&mut self) {
        self.state = ConnectionState::Closed;
        // Socket is dropped automatically
    }

    /// Check if the connection is ready for requests.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Ready
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get the schema version from the server.
    pub fn schema_version(&self) -> u64 {
        self.schema_version.load(Ordering::SeqCst)
    }

    /// Get the server capabilities.
    pub fn server_capabilities(&self) -> &[String] {
        &self.server_capabilities
    }

    /// Get the server ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Check if the server supports a capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.server_capabilities.iter().any(|c| c == capability)
    }

    /// Update the cached schema version.
    pub fn update_schema_version(&self, version: u64) {
        self.schema_version.store(version, Ordering::SeqCst);
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("address", &self.config.address)
            .field("state", &self.state)
            .field("client_id", &self.config.client_id)
            .field("server_id", &self.server_id)
            .field("schema_version", &self.schema_version.load(Ordering::SeqCst))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_ne!(ConnectionState::Connected, ConnectionState::Ready);
    }

    #[test]
    fn test_config_defaults() {
        let config = ClientConfig::default();
        assert_eq!(config.address, crate::config::DEFAULT_ADDRESS);
        assert!(config.client_id.starts_with("client-"));
    }

    // Integration tests would require a running server
    // Those will be added in the integration test module
}
