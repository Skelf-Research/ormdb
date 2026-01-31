//! Client configuration.

use std::time::Duration;

/// Default TCP address for ORMDB server.
pub const DEFAULT_ADDRESS: &str = "tcp://127.0.0.1:9000";

/// Default request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum message size (64 MB).
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Server address (e.g., "tcp://127.0.0.1:9000" or "ipc:///tmp/ormdb.sock").
    pub address: String,

    /// Request timeout.
    pub timeout: Duration,

    /// Maximum message size in bytes.
    pub max_message_size: usize,

    /// Client identifier for server-side tracking.
    pub client_id: String,
}

impl ClientConfig {
    /// Create a new client configuration with the specified address.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            timeout: DEFAULT_TIMEOUT,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            client_id: generate_client_id(),
        }
    }

    /// Create a configuration for connecting to localhost on the default port.
    pub fn localhost() -> Self {
        Self::new(DEFAULT_ADDRESS)
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum message size.
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set the client identifier.
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = client_id.into();
        self
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::localhost()
    }
}

/// Generate a unique client identifier.
fn generate_client_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    format!("client-{:x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.address, DEFAULT_ADDRESS);
        assert_eq!(config.timeout, DEFAULT_TIMEOUT);
        assert_eq!(config.max_message_size, DEFAULT_MAX_MESSAGE_SIZE);
        assert!(config.client_id.starts_with("client-"));
    }

    #[test]
    fn test_config_builder() {
        let config = ClientConfig::new("tcp://192.168.1.1:9000")
            .with_timeout(Duration::from_secs(60))
            .with_max_message_size(1024 * 1024)
            .with_client_id("my-client");

        assert_eq!(config.address, "tcp://192.168.1.1:9000");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_message_size, 1024 * 1024);
        assert_eq!(config.client_id, "my-client");
    }

    #[test]
    fn test_ipc_address() {
        let config = ClientConfig::new("ipc:///tmp/ormdb.sock");
        assert_eq!(config.address, "ipc:///tmp/ormdb.sock");
    }
}
