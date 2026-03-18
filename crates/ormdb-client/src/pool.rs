//! Connection pooling for ORMDB client.
//!
//! Provides a pool of connections for concurrent access to an ORMDB server.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};

use ormdb_proto::mutation::{Mutation, MutationBatch};
use ormdb_proto::query::GraphQuery;
use ormdb_proto::result::{MutationResult, QueryResult};
use ormdb_proto::{Request, Response, ResponsePayload, Status};

use crate::config::ClientConfig;
use crate::connection::Connection;
use crate::error::Error;

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain.
    pub min_connections: usize,
    /// Maximum number of connections allowed.
    pub max_connections: usize,
    /// Timeout for acquiring a connection from the pool.
    pub acquire_timeout: Duration,
    /// Idle timeout after which unused connections are closed.
    pub idle_timeout: Duration,
    /// Client configuration for creating new connections.
    pub client_config: ClientConfig,
}

impl PoolConfig {
    /// Create a new pool configuration.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            client_config: ClientConfig::new(address),
        }
    }

    /// Set the minimum connections.
    pub fn with_min_connections(mut self, min: usize) -> Self {
        self.min_connections = min;
        self
    }

    /// Set the maximum connections.
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the acquire timeout.
    pub fn with_acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Set the idle timeout.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the client configuration.
    pub fn with_client_config(mut self, config: ClientConfig) -> Self {
        self.client_config = config;
        self
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self::new(crate::config::DEFAULT_ADDRESS)
    }
}

/// A pooled connection that returns itself to the pool when dropped.
pub struct PooledConnection {
    connection: Option<Connection>,
    pool: Arc<ConnectionPoolInner>,
}

impl PooledConnection {
    /// Send a request using this connection.
    pub async fn send_request(&self, request: &Request) -> Result<Response, Error> {
        match &self.connection {
            Some(conn) => conn.send_request(request).await,
            None => Err(Error::Pool("connection is not available".to_string())),
        }
    }

    /// Check if this connection is still valid.
    pub fn is_connected(&self) -> bool {
        self.connection.as_ref().map(|c| c.is_connected()).unwrap_or(false)
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.connection.take() {
            // Return connection to pool
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.return_connection(conn).await;
            });
        }
    }
}

/// Internal pool state.
struct ConnectionPoolInner {
    config: PoolConfig,
    connections: Mutex<Vec<Connection>>,
    semaphore: Semaphore,
    next_request_id: AtomicU64,
    schema_version: AtomicU64,
}

impl ConnectionPoolInner {
    fn new(config: PoolConfig) -> Self {
        let semaphore = Semaphore::new(config.max_connections);
        Self {
            config,
            connections: Mutex::new(Vec::new()),
            semaphore,
            next_request_id: AtomicU64::new(1),
            schema_version: AtomicU64::new(0),
        }
    }

    async fn acquire(&self) -> Result<Connection, Error> {
        // Try to get an existing connection
        {
            let mut conns = self.connections.lock().await;
            while let Some(conn) = conns.pop() {
                if conn.is_connected() {
                    return Ok(conn);
                }
                // Connection is dead, discard it
            }
        }

        // Create a new connection
        let mut conn = Connection::establish(self.config.client_config.clone()).await?;
        conn.handshake().await?;

        // Update schema version from the new connection
        self.schema_version.store(conn.schema_version(), Ordering::SeqCst);

        Ok(conn)
    }

    async fn return_connection(&self, conn: Connection) {
        if conn.is_connected() {
            let mut conns = self.connections.lock().await;
            if conns.len() < self.config.max_connections {
                conns.push(conn);
            }
            // If pool is full, connection is dropped
        }
    }

    fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst)
    }

    fn schema_version(&self) -> u64 {
        self.schema_version.load(Ordering::SeqCst)
    }
}

/// A pool of connections to an ORMDB server.
///
/// The pool maintains a set of reusable connections and automatically
/// manages their lifecycle.
///
/// # Example
///
/// ```ignore
/// use ormdb_client::{ConnectionPool, PoolConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = ConnectionPool::new(PoolConfig::default()).await?;
///
///     // Execute queries using the pool
///     let result = pool.query(GraphQuery::new("User")).await?;
///
///     pool.close().await;
///     Ok(())
/// }
/// ```
pub struct ConnectionPool {
    inner: Arc<ConnectionPoolInner>,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub async fn new(config: PoolConfig) -> Result<Self, Error> {
        let inner = Arc::new(ConnectionPoolInner::new(config.clone()));

        // Create minimum connections
        let mut initial_conns = Vec::new();
        for _ in 0..config.min_connections {
            let conn = inner.acquire().await?;
            initial_conns.push(conn);
        }

        // Return them to the pool
        {
            let mut conns = inner.connections.lock().await;
            conns.extend(initial_conns);
        }

        Ok(Self { inner })
    }

    /// Acquire a connection from the pool.
    pub async fn acquire(&self) -> Result<PooledConnection, Error> {
        // Wait for a permit (limits concurrent connections)
        let permit = tokio::time::timeout(
            self.inner.config.acquire_timeout,
            self.inner.semaphore.acquire(),
        )
        .await
        .map_err(|_| Error::Pool("timeout waiting for connection".to_string()))?
        .map_err(|_| Error::Pool("semaphore closed".to_string()))?;

        // Get or create a connection
        let conn = self.inner.acquire().await?;

        // Forget the permit - it will be released when connection is returned
        permit.forget();

        Ok(PooledConnection {
            connection: Some(conn),
            pool: self.inner.clone(),
        })
    }

    /// Execute a graph query.
    pub async fn query(&self, query: GraphQuery) -> Result<QueryResult, Error> {
        let conn = self.acquire().await?;
        let request_id = self.inner.next_request_id();
        let schema_version = self.inner.schema_version();

        let request = Request::query(request_id, schema_version, query);
        let response = conn.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Query(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected query result".to_string(),
            ))),
        })
    }

    /// Execute a single mutation.
    pub async fn mutate(&self, mutation: Mutation) -> Result<MutationResult, Error> {
        let conn = self.acquire().await?;
        let request_id = self.inner.next_request_id();
        let schema_version = self.inner.schema_version();

        let request = Request::mutate(request_id, schema_version, mutation);
        let response = conn.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Mutation(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected mutation result".to_string(),
            ))),
        })
    }

    /// Execute a batch of mutations atomically.
    pub async fn mutate_batch(&self, batch: MutationBatch) -> Result<MutationResult, Error> {
        let conn = self.acquire().await?;
        let request_id = self.inner.next_request_id();
        let schema_version = self.inner.schema_version();

        let request = Request::mutate_batch(request_id, schema_version, batch);
        let response = conn.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Mutation(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected mutation result".to_string(),
            ))),
        })
    }

    /// Get the current schema from the server.
    pub async fn get_schema(&self) -> Result<(u64, Vec<u8>), Error> {
        let conn = self.acquire().await?;
        let request_id = self.inner.next_request_id();

        let request = Request::get_schema(request_id);
        let response = conn.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Schema { version, data } => {
                self.inner.schema_version.store(version, Ordering::SeqCst);
                Ok((version, data))
            }
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected schema result".to_string(),
            ))),
        })
    }

    /// Ping the server to check connectivity.
    pub async fn ping(&self) -> Result<(), Error> {
        let conn = self.acquire().await?;
        let request_id = self.inner.next_request_id();

        let request = Request::ping(request_id);
        let response = conn.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Pong => Ok(()),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected pong response".to_string(),
            ))),
        })
    }

    /// Close all connections in the pool.
    pub async fn close(&self) {
        let mut conns = self.inner.connections.lock().await;
        for mut conn in conns.drain(..) {
            conn.close();
        }
    }

    /// Get the current number of idle connections.
    pub async fn idle_connections(&self) -> usize {
        self.inner.connections.lock().await.len()
    }

    /// Get the current schema version.
    pub fn schema_version(&self) -> u64 {
        self.inner.schema_version()
    }

    /// Handle a response, extracting the payload or converting errors.
    fn handle_response<T, F>(&self, response: Response, extract: F) -> Result<T, Error>
    where
        F: FnOnce(ResponsePayload) -> Result<T, Error>,
    {
        match response.status {
            Status::Ok => extract(response.payload),
            Status::Error { code, message } => {
                Err(Error::Server { code, message })
            }
        }
    }
}

impl std::fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionPool")
            .field("max_connections", &self.inner.config.max_connections)
            .field("schema_version", &self.inner.schema_version())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new("tcp://localhost:9000")
            .with_min_connections(2)
            .with_max_connections(20)
            .with_acquire_timeout(Duration::from_secs(60));

        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.acquire_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.max_connections, 10);
    }
}
