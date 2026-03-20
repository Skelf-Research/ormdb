//! Server configuration.

use clap::Parser;
use ormdb_core::storage::RetentionPolicy;
use std::path::PathBuf;
use std::time::Duration;

/// Default TCP address for the server.
pub const DEFAULT_TCP_ADDRESS: &str = "tcp://0.0.0.0:9000";

/// Default request timeout in seconds.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Default maximum message size (64 MB).
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Default compaction interval in seconds (1 hour).
pub const DEFAULT_COMPACTION_INTERVAL_SECS: u64 = 3600;

fn default_transport_workers() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .max(1)
}

/// ORMDB Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// TCP address to bind to (e.g., "tcp://0.0.0.0:9000").
    pub tcp_address: Option<String>,

    /// IPC address to bind to (e.g., "ipc:///tmp/ormdb.sock").
    pub ipc_address: Option<String>,

    /// Path to the database storage directory.
    pub data_path: PathBuf,

    /// Request timeout duration.
    pub request_timeout: Duration,

    /// Maximum message size in bytes.
    pub max_message_size: usize,

    /// Version retention policy for compaction.
    pub retention_policy: RetentionPolicy,

    /// Interval between automatic compaction runs. None disables auto-compaction.
    pub compaction_interval: Option<Duration>,

    /// Number of transport worker loops (AsyncContext instances).
    pub transport_workers: usize,
}

impl ServerConfig {
    /// Create a new server configuration with the given data path.
    pub fn new(data_path: impl Into<PathBuf>) -> Self {
        Self {
            tcp_address: Some(DEFAULT_TCP_ADDRESS.to_string()),
            ipc_address: None,
            data_path: data_path.into(),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            retention_policy: RetentionPolicy::default(),
            compaction_interval: Some(Duration::from_secs(DEFAULT_COMPACTION_INTERVAL_SECS)),
            transport_workers: default_transport_workers(),
        }
    }

    /// Set the TCP address.
    pub fn with_tcp_address(mut self, address: impl Into<String>) -> Self {
        self.tcp_address = Some(address.into());
        self
    }

    /// Disable TCP transport.
    pub fn without_tcp(mut self) -> Self {
        self.tcp_address = None;
        self
    }

    /// Set the IPC address.
    pub fn with_ipc_address(mut self, address: impl Into<String>) -> Self {
        self.ipc_address = Some(address.into());
        self
    }

    /// Set the request timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set the maximum message size.
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set the retention policy for version compaction.
    pub fn with_retention_policy(mut self, policy: RetentionPolicy) -> Self {
        self.retention_policy = policy;
        self
    }

    /// Set the compaction interval.
    pub fn with_compaction_interval(mut self, interval: Duration) -> Self {
        self.compaction_interval = Some(interval);
        self
    }

    /// Disable automatic compaction.
    pub fn without_compaction(mut self) -> Self {
        self.compaction_interval = None;
        self
    }

    /// Set the number of transport worker loops.
    pub fn with_transport_workers(mut self, workers: usize) -> Self {
        self.transport_workers = workers.max(1);
        self
    }

    /// Check if at least one transport is configured.
    pub fn has_transport(&self) -> bool {
        self.tcp_address.is_some() || self.ipc_address.is_some()
    }

    /// Check if automatic compaction is enabled.
    pub fn has_compaction(&self) -> bool {
        self.compaction_interval.is_some()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new("./data")
    }
}

/// Command-line arguments for the server.
#[derive(Parser, Debug)]
#[command(name = "ormdb-server")]
#[command(version, about = "ORMDB Database Server", long_about = None)]
pub struct Args {
    /// Path to the database storage directory.
    #[arg(short, long, default_value = "./data")]
    pub data_path: PathBuf,

    /// TCP address to bind to.
    #[arg(long, default_value = DEFAULT_TCP_ADDRESS)]
    pub tcp: String,

    /// IPC address to bind to (optional).
    #[arg(long)]
    pub ipc: Option<String>,

    /// Request timeout in seconds.
    #[arg(long, default_value_t = DEFAULT_REQUEST_TIMEOUT_SECS)]
    pub timeout: u64,

    /// Maximum message size in megabytes.
    #[arg(long, default_value_t = 64)]
    pub max_message_mb: usize,

    /// Disable TCP transport (requires --ipc to be set).
    #[arg(long)]
    pub no_tcp: bool,

    /// Compaction interval in seconds. Set to 0 to disable auto-compaction.
    #[arg(long, default_value_t = DEFAULT_COMPACTION_INTERVAL_SECS)]
    pub compaction_interval: u64,

    /// Maximum versions to keep per entity.
    #[arg(long, default_value_t = 100)]
    pub max_versions: usize,

    /// Transport worker loops (0 = auto).
    #[arg(long, default_value_t = 0)]
    pub workers: usize,
}

impl Args {
    /// Convert command-line arguments to server configuration.
    pub fn into_config(self) -> ServerConfig {
        let tcp_address = if self.no_tcp { None } else { Some(self.tcp) };

        let compaction_interval = if self.compaction_interval == 0 {
            None
        } else {
            Some(Duration::from_secs(self.compaction_interval))
        };

        let retention_policy = RetentionPolicy::with_max_versions(self.max_versions);
        let transport_workers = if self.workers == 0 {
            default_transport_workers()
        } else {
            self.workers.max(1)
        };

        ServerConfig {
            tcp_address,
            ipc_address: self.ipc,
            data_path: self.data_path,
            request_timeout: Duration::from_secs(self.timeout),
            max_message_size: self.max_message_mb * 1024 * 1024,
            retention_policy,
            compaction_interval,
            transport_workers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.tcp_address, Some(DEFAULT_TCP_ADDRESS.to_string()));
        assert!(config.ipc_address.is_none());
        assert_eq!(config.data_path, PathBuf::from("./data"));
        assert_eq!(
            config.request_timeout,
            Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)
        );
        assert!(config.has_transport());
    }

    #[test]
    fn test_config_builder() {
        let config = ServerConfig::new("/var/lib/ormdb")
            .with_tcp_address("tcp://127.0.0.1:8080")
            .with_ipc_address("ipc:///tmp/ormdb.sock")
            .with_request_timeout(Duration::from_secs(60))
            .with_max_message_size(128 * 1024 * 1024);

        assert_eq!(
            config.tcp_address,
            Some("tcp://127.0.0.1:8080".to_string())
        );
        assert_eq!(
            config.ipc_address,
            Some("ipc:///tmp/ormdb.sock".to_string())
        );
        assert_eq!(config.data_path, PathBuf::from("/var/lib/ormdb"));
        assert_eq!(config.request_timeout, Duration::from_secs(60));
        assert_eq!(config.max_message_size, 128 * 1024 * 1024);
    }

    #[test]
    fn test_without_tcp() {
        let config = ServerConfig::new("./data")
            .without_tcp()
            .with_ipc_address("ipc:///tmp/ormdb.sock");

        assert!(config.tcp_address.is_none());
        assert!(config.ipc_address.is_some());
        assert!(config.has_transport());
    }

    #[test]
    fn test_no_transport() {
        let config = ServerConfig::new("./data").without_tcp();
        assert!(!config.has_transport());
    }
}
