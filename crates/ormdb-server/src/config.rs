//! Server configuration.

use clap::Parser;
use clap::ValueEnum;
use ormdb_core::storage::RetentionPolicy;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "raft")]
use ormdb_raft::RaftConfig;

/// Authentication method to use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum AuthMethod {
    /// Development mode - grants full admin access (NOT for production).
    #[default]
    Dev,
    /// API Key authentication (from ORMDB_API_KEYS environment variable).
    ApiKey,
    /// Bearer token authentication (from ORMDB_TOKENS environment variable).
    Token,
    /// JWT authentication (from ORMDB_JWT_SECRET environment variable).
    Jwt,
}

/// TLS configuration for secure transport.
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// Enable TLS for client connections.
    pub enabled: bool,
    /// Path to certificate file (PEM format).
    pub cert_path: Option<PathBuf>,
    /// Path to private key file (PEM format).
    pub key_path: Option<PathBuf>,
    /// Path to CA certificate for client verification (optional).
    pub ca_path: Option<PathBuf>,
    /// Require client certificate verification.
    pub require_client_cert: bool,
}

impl TlsConfig {
    /// Create a new TLS config with certificate and key.
    pub fn new(cert_path: impl Into<PathBuf>, key_path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
            ca_path: None,
            require_client_cert: false,
        }
    }

    /// Set CA certificate for client verification.
    pub fn with_ca(mut self, ca_path: impl Into<PathBuf>) -> Self {
        self.ca_path = Some(ca_path.into());
        self
    }

    /// Require client certificate verification.
    pub fn require_client_cert(mut self) -> Self {
        self.require_client_cert = true;
        self
    }
}

/// Rate limiting configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per second per client.
    pub requests_per_second: u32,
    /// Burst size (requests allowed above rate limit).
    pub burst_size: u32,
    /// Enable rate limiting.
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 1000,
            burst_size: 100,
            enabled: false,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit config.
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        Self {
            requests_per_second,
            burst_size,
            enabled: true,
        }
    }
}

/// Connection limits configuration.
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    /// Maximum concurrent connections.
    pub max_connections: u32,
    /// Connection timeout in seconds.
    pub connection_timeout_secs: u64,
    /// Enable connection limits.
    pub enabled: bool,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_connections: 10000,
            connection_timeout_secs: 300,
            enabled: false,
        }
    }
}

impl ConnectionLimits {
    /// Create a new connection limits config.
    pub fn new(max_connections: u32) -> Self {
        Self {
            max_connections,
            connection_timeout_secs: 300,
            enabled: true,
        }
    }
}

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

    /// Authentication method to use.
    pub auth_method: AuthMethod,

    /// TLS configuration for client connections.
    pub tls: TlsConfig,

    /// Rate limiting configuration.
    pub rate_limit: RateLimitConfig,

    /// Connection limits configuration.
    pub connection_limits: ConnectionLimits,

    /// Raft configuration for cluster mode. None for standalone mode.
    #[cfg(feature = "raft")]
    pub raft_config: Option<RaftConfig>,

    /// Whether to initialize the cluster on startup.
    #[cfg(feature = "raft")]
    pub cluster_init: bool,

    /// Cluster members for initialization (node_id, address).
    #[cfg(feature = "raft")]
    pub cluster_members: Vec<(u64, String)>,
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
            auth_method: AuthMethod::Dev,
            tls: TlsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            connection_limits: ConnectionLimits::default(),
            #[cfg(feature = "raft")]
            raft_config: None,
            #[cfg(feature = "raft")]
            cluster_init: false,
            #[cfg(feature = "raft")]
            cluster_members: Vec::new(),
        }
    }

    /// Set the authentication method.
    pub fn with_auth_method(mut self, method: AuthMethod) -> Self {
        self.auth_method = method;
        self
    }

    /// Set TLS configuration.
    pub fn with_tls(mut self, tls: TlsConfig) -> Self {
        self.tls = tls;
        self
    }

    /// Set rate limiting configuration.
    pub fn with_rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    /// Set connection limits configuration.
    pub fn with_connection_limits(mut self, limits: ConnectionLimits) -> Self {
        self.connection_limits = limits;
        self
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

    /// Set the Raft configuration for cluster mode.
    #[cfg(feature = "raft")]
    pub fn with_raft_config(mut self, config: RaftConfig) -> Self {
        self.raft_config = Some(config);
        self
    }

    /// Check if Raft clustering is enabled.
    #[cfg(feature = "raft")]
    pub fn has_raft(&self) -> bool {
        self.raft_config.is_some()
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

    /// Authentication method: dev (no auth), api-key, token, jwt.
    /// Note: 'dev' mode grants full admin access and should NOT be used in production.
    #[arg(long, value_enum, default_value_t = AuthMethod::Dev)]
    pub auth: AuthMethod,

    /// Enable TLS for client connections.
    #[arg(long)]
    pub tls: bool,

    /// Path to TLS certificate file (PEM format).
    #[arg(long, requires = "tls")]
    pub tls_cert: Option<PathBuf>,

    /// Path to TLS private key file (PEM format).
    #[arg(long, requires = "tls")]
    pub tls_key: Option<PathBuf>,

    /// Path to CA certificate for client verification (optional).
    #[arg(long)]
    pub tls_ca: Option<PathBuf>,

    /// Require client certificate verification.
    #[arg(long)]
    pub tls_require_client_cert: bool,

    /// Enable rate limiting.
    #[arg(long)]
    pub rate_limit: bool,

    /// Maximum requests per second (requires --rate-limit).
    #[arg(long, default_value_t = 1000)]
    pub rate_limit_rps: u32,

    /// Rate limit burst size (requires --rate-limit).
    #[arg(long, default_value_t = 100)]
    pub rate_limit_burst: u32,

    /// Maximum concurrent connections (0 = unlimited).
    #[arg(long, default_value_t = 0)]
    pub max_connections: u32,

    /// Enable Raft cluster mode with the given node ID.
    #[cfg(feature = "raft")]
    #[arg(long)]
    pub raft_node_id: Option<u64>,

    /// Raft listen address (e.g., "0.0.0.0:9001").
    #[cfg(feature = "raft")]
    #[arg(long, default_value = "0.0.0.0:9001")]
    pub raft_listen: String,

    /// Raft advertise address (e.g., "192.168.1.10:9001").
    #[cfg(feature = "raft")]
    #[arg(long)]
    pub raft_advertise: Option<String>,

    /// Initialize Raft cluster with this node as the bootstrap node.
    #[cfg(feature = "raft")]
    #[arg(long)]
    pub cluster_init: bool,

    /// Comma-separated list of cluster members for initialization (id:addr format).
    /// Example: "0:node1:9001,1:node2:9001,2:node3:9001"
    #[cfg(feature = "raft")]
    #[arg(long)]
    pub cluster_members: Option<String>,
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

        #[cfg(feature = "raft")]
        let raft_config = self.raft_node_id.map(|node_id| {
            let advertise_addr = self.raft_advertise.unwrap_or_else(|| self.raft_listen.clone());
            RaftConfig::new(node_id)
                .with_data_dir(&self.data_path)
                .with_raft_listen_addr(&self.raft_listen)
                .with_raft_advertise_addr(&advertise_addr)
        });

        #[cfg(feature = "raft")]
        let cluster_members = self
            .cluster_members
            .map(|s| parse_cluster_members(&s))
            .unwrap_or_default();

        // Build TLS config
        let tls = if self.tls {
            TlsConfig {
                enabled: true,
                cert_path: self.tls_cert,
                key_path: self.tls_key,
                ca_path: self.tls_ca,
                require_client_cert: self.tls_require_client_cert,
            }
        } else {
            TlsConfig::default()
        };

        // Build rate limit config
        let rate_limit = if self.rate_limit {
            RateLimitConfig::new(self.rate_limit_rps, self.rate_limit_burst)
        } else {
            RateLimitConfig::default()
        };

        // Build connection limits config
        let connection_limits = if self.max_connections > 0 {
            ConnectionLimits::new(self.max_connections)
        } else {
            ConnectionLimits::default()
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
            auth_method: self.auth,
            tls,
            rate_limit,
            connection_limits,
            #[cfg(feature = "raft")]
            raft_config,
            #[cfg(feature = "raft")]
            cluster_init: self.cluster_init,
            #[cfg(feature = "raft")]
            cluster_members,
        }
    }
}

/// Parse cluster members from string format "id:addr,id:addr,..."
#[cfg(feature = "raft")]
fn parse_cluster_members(s: &str) -> Vec<(u64, String)> {
    s.split(',')
        .filter_map(|member| {
            let parts: Vec<&str> = member.splitn(2, ':').collect();
            if parts.len() == 2 {
                if let Ok(id) = parts[0].parse::<u64>() {
                    Some((id, parts[1].to_string()))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
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
