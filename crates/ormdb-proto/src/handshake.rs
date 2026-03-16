//! Protocol handshake types for connection negotiation.

use rkyv::{Archive, Deserialize, Serialize};

/// Client handshake message sent when establishing a connection.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct Handshake {
    /// Protocol version the client supports.
    pub protocol_version: u32,
    /// Client identifier (for logging and debugging).
    pub client_id: String,
    /// Capabilities the client supports.
    pub capabilities: Vec<String>,
}

impl Handshake {
    /// Create a new handshake with the current protocol version.
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            protocol_version: crate::PROTOCOL_VERSION,
            client_id: client_id.into(),
            capabilities: vec![],
        }
    }

    /// Create a handshake with a specific protocol version.
    pub fn with_version(protocol_version: u32, client_id: impl Into<String>) -> Self {
        Self {
            protocol_version,
            client_id: client_id.into(),
            capabilities: vec![],
        }
    }

    /// Add a capability to the handshake.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Add multiple capabilities to the handshake.
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }
}

/// Server response to a client handshake.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct HandshakeResponse {
    /// Whether the handshake was accepted.
    pub accepted: bool,
    /// Protocol version the server will use for this connection.
    pub protocol_version: u32,
    /// Current schema version on the server.
    pub schema_version: u64,
    /// Server identifier.
    pub server_id: String,
    /// Capabilities the server supports.
    pub capabilities: Vec<String>,
    /// Error message if handshake was rejected.
    pub error: Option<String>,
}

impl HandshakeResponse {
    /// Create a successful handshake response.
    pub fn accept(
        protocol_version: u32,
        schema_version: u64,
        server_id: impl Into<String>,
    ) -> Self {
        Self {
            accepted: true,
            protocol_version,
            schema_version,
            server_id: server_id.into(),
            capabilities: vec![],
            error: None,
        }
    }

    /// Create a rejected handshake response.
    pub fn reject(error: impl Into<String>) -> Self {
        Self {
            accepted: false,
            protocol_version: 0,
            schema_version: 0,
            server_id: String::new(),
            capabilities: vec![],
            error: Some(error.into()),
        }
    }

    /// Add a capability to the response.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Add multiple capabilities to the response.
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }
}

/// Standard capability identifiers.
pub mod capabilities {
    /// Streaming query results.
    pub const STREAMING: &str = "streaming";
    /// Change data capture / subscriptions.
    pub const CDC: &str = "cdc";
    /// Batch operations.
    pub const BATCH: &str = "batch";
    /// Compression support.
    pub const COMPRESSION: &str = "compression";
    /// Transaction support.
    pub const TRANSACTIONS: &str = "transactions";
}

/// Check if a protocol version is compatible with the current version.
pub fn is_version_compatible(client_version: u32, server_version: u32) -> bool {
    // For now, require exact match. In the future, we can support
    // version ranges or negotiate down to a common version.
    client_version == server_version
}

/// Negotiate the protocol version between client and server.
pub fn negotiate_version(client_version: u32, server_version: u32) -> Option<u32> {
    if is_version_compatible(client_version, server_version) {
        Some(server_version)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_creation() {
        let handshake = Handshake::new("test-client")
            .with_capability(capabilities::STREAMING)
            .with_capability(capabilities::BATCH);

        assert_eq!(handshake.protocol_version, crate::PROTOCOL_VERSION);
        assert_eq!(handshake.client_id, "test-client");
        assert_eq!(handshake.capabilities.len(), 2);
        assert!(handshake.capabilities.contains(&capabilities::STREAMING.to_string()));
    }

    #[test]
    fn test_handshake_response_accept() {
        let response = HandshakeResponse::accept(1, 5, "server-1")
            .with_capability(capabilities::STREAMING)
            .with_capability(capabilities::TRANSACTIONS);

        assert!(response.accepted);
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.schema_version, 5);
        assert_eq!(response.server_id, "server-1");
        assert!(response.error.is_none());
        assert_eq!(response.capabilities.len(), 2);
    }

    #[test]
    fn test_handshake_response_reject() {
        let response = HandshakeResponse::reject("Unsupported protocol version");

        assert!(!response.accepted);
        assert_eq!(response.error, Some("Unsupported protocol version".to_string()));
    }

    #[test]
    fn test_version_compatibility() {
        assert!(is_version_compatible(1, 1));
        assert!(!is_version_compatible(1, 2));
        assert!(!is_version_compatible(2, 1));

        assert_eq!(negotiate_version(1, 1), Some(1));
        assert_eq!(negotiate_version(1, 2), None);
    }

    #[test]
    fn test_handshake_serialization_roundtrip() {
        let handshake = Handshake::new("rust-client-v1")
            .with_capabilities(vec![
                capabilities::STREAMING.into(),
                capabilities::CDC.into(),
            ]);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&handshake).unwrap();
        let archived = rkyv::access::<ArchivedHandshake, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Handshake =
            rkyv::deserialize::<Handshake, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(handshake, deserialized);

        // Test response
        let response = HandshakeResponse::accept(1, 10, "ormdb-server")
            .with_capability(capabilities::TRANSACTIONS);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&response).unwrap();
        let archived =
            rkyv::access::<ArchivedHandshakeResponse, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: HandshakeResponse =
            rkyv::deserialize::<HandshakeResponse, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(response, deserialized);
    }
}
