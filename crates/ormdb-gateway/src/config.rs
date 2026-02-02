//! Gateway configuration.

use std::time::Duration;

use clap::Parser;

/// ORMDB HTTP/JSON Gateway command line arguments.
#[derive(Debug, Parser)]
#[command(name = "ormdb-gateway")]
#[command(about = "HTTP/JSON Gateway for ORMDB")]
pub struct Args {
    /// Address to listen on for HTTP requests.
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub listen: String,

    /// Address of the ORMDB server (NNG address).
    #[arg(short, long, default_value = "tcp://127.0.0.1:9000")]
    pub ormdb: String,

    /// Minimum number of pooled connections to maintain.
    #[arg(long, default_value_t = 1)]
    pub pool_min_connections: usize,

    /// Maximum number of pooled connections to allow.
    #[arg(long, default_value_t = 10)]
    pub pool_max_connections: usize,

    /// Timeout (ms) when acquiring a pooled connection.
    #[arg(long, default_value_t = 30_000)]
    pub pool_acquire_timeout_ms: u64,

    /// Idle timeout (ms) after which pooled connections can be closed.
    #[arg(long, default_value_t = 300_000)]
    pub pool_idle_timeout_ms: u64,

    /// Client request timeout (ms) for NNG send/recv.
    #[arg(long, default_value_t = 30_000)]
    pub client_timeout_ms: u64,

    /// Per-request timeout (ms) enforced at the gateway. Defaults to client timeout when unset.
    #[arg(long)]
    pub request_timeout_ms: Option<u64>,

    /// Number of retries for read-only requests.
    #[arg(long, default_value_t = 0)]
    pub request_retries: usize,

    /// Backoff (ms) between retries for read-only requests.
    #[arg(long, default_value_t = 50)]
    pub request_retry_backoff_ms: u64,
}

/// Gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Address to listen on for HTTP requests.
    pub listen_addr: String,
    /// Address of the ORMDB server.
    pub ormdb_addr: String,
    /// Minimum number of pooled connections to maintain.
    pub pool_min_connections: usize,
    /// Maximum number of pooled connections to allow.
    pub pool_max_connections: usize,
    /// Timeout when acquiring a pooled connection.
    pub pool_acquire_timeout: Duration,
    /// Idle timeout after which pooled connections can be closed.
    pub pool_idle_timeout: Duration,
    /// Client request timeout for NNG send/recv.
    pub client_timeout: Duration,
    /// Per-request timeout enforced at the gateway.
    pub request_timeout: Duration,
    /// Number of retries for read-only requests.
    pub request_retries: usize,
    /// Backoff between retries for read-only requests.
    pub request_retry_backoff: Duration,
}

impl From<&Args> for GatewayConfig {
    fn from(args: &Args) -> Self {
        let client_timeout = Duration::from_millis(args.client_timeout_ms);
        let request_timeout = args
            .request_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(client_timeout);

        Self {
            listen_addr: args.listen.clone(),
            ormdb_addr: args.ormdb.clone(),
            pool_min_connections: args.pool_min_connections,
            pool_max_connections: args.pool_max_connections,
            pool_acquire_timeout: Duration::from_millis(args.pool_acquire_timeout_ms),
            pool_idle_timeout: Duration::from_millis(args.pool_idle_timeout_ms),
            client_timeout,
            request_timeout,
            request_retries: args.request_retries,
            request_retry_backoff: Duration::from_millis(args.request_retry_backoff_ms),
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".to_string(),
            ormdb_addr: "tcp://127.0.0.1:9000".to_string(),
            pool_min_connections: 1,
            pool_max_connections: 10,
            pool_acquire_timeout: Duration::from_secs(30),
            pool_idle_timeout: Duration::from_secs(300),
            client_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(30),
            request_retries: 0,
            request_retry_backoff: Duration::from_millis(50),
        }
    }
}
