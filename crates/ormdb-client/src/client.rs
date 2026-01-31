//! ORMDB client API.
//!
//! This module provides the main `Client` struct for interacting with an ORMDB server.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use ormdb_proto::mutation::{Mutation, MutationBatch};
use ormdb_proto::query::{AggregateQuery, GraphQuery};
use ormdb_proto::replication::{ReplicationStatus, StreamChangesRequest, StreamChangesResponse};
use ormdb_proto::result::{AggregateResult, MutationResult, QueryResult};
use ormdb_proto::{ExplainResult, MetricsResult, Request, Response, ResponsePayload, Status};

use crate::config::ClientConfig;
use crate::connection::Connection;
use crate::error::Error;

/// An ORMDB client for connecting to and interacting with an ORMDB server.
///
/// # Example
///
/// ```ignore
/// use ormdb_client::{Client, ClientConfig};
/// use ormdb_proto::GraphQuery;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Connect to the server
///     let client = Client::connect(ClientConfig::localhost()).await?;
///
///     // Execute a query
///     let query = GraphQuery::new("User")
///         .with_fields(vec!["id".into(), "name".into()]);
///     let result = client.query(query).await?;
///
///     // Close the connection
///     client.close().await;
///     Ok(())
/// }
/// ```
pub struct Client {
    connection: Arc<Mutex<Connection>>,
    next_request_id: AtomicU64,
    schema_version: AtomicU64,
}

impl Client {
    /// Connect to an ORMDB server.
    pub async fn connect(config: ClientConfig) -> Result<Self, Error> {
        // Establish connection
        let mut connection = Connection::establish(config).await?;

        // Perform handshake
        connection.handshake().await?;

        // Get schema version from handshake
        let schema_version = connection.schema_version();

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            next_request_id: AtomicU64::new(1),
            schema_version: AtomicU64::new(schema_version),
        })
    }

    /// Connect to a server at the given address.
    pub async fn connect_to(address: impl Into<String>) -> Result<Self, Error> {
        Self::connect(ClientConfig::new(address)).await
    }

    /// Connect to localhost on the default port.
    pub async fn connect_localhost() -> Result<Self, Error> {
        Self::connect(ClientConfig::localhost()).await
    }

    /// Execute a graph query.
    pub async fn query(&self, query: GraphQuery) -> Result<QueryResult, Error> {
        let request_id = self.next_request_id();
        let schema_version = self.schema_version();

        let request = Request::query(request_id, schema_version, query);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Query(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected query result".to_string(),
            ))),
        })
    }

    /// Execute an aggregate query.
    pub async fn aggregate(&self, query: AggregateQuery) -> Result<AggregateResult, Error> {
        let request_id = self.next_request_id();
        let schema_version = self.schema_version();

        let request = Request::aggregate(request_id, schema_version, query);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Aggregate(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected aggregate result".to_string(),
            ))),
        })
    }

    /// Execute a single mutation.
    pub async fn mutate(&self, mutation: Mutation) -> Result<MutationResult, Error> {
        let request_id = self.next_request_id();
        let schema_version = self.schema_version();

        let request = Request::mutate(request_id, schema_version, mutation);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Mutation(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected mutation result".to_string(),
            ))),
        })
    }

    /// Execute a batch of mutations atomically.
    pub async fn mutate_batch(&self, batch: MutationBatch) -> Result<MutationResult, Error> {
        let request_id = self.next_request_id();
        let schema_version = self.schema_version();

        let request = Request::mutate_batch(request_id, schema_version, batch);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Mutation(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected mutation result".to_string(),
            ))),
        })
    }

    /// Get the current schema from the server.
    pub async fn get_schema(&self) -> Result<(u64, Vec<u8>), Error> {
        let request_id = self.next_request_id();

        let request = Request::get_schema(request_id);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Schema { version, data } => {
                // Update our cached schema version
                self.schema_version.store(version, Ordering::SeqCst);
                Ok((version, data))
            }
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected schema result".to_string(),
            ))),
        })
    }

    /// Ping the server to check connectivity.
    pub async fn ping(&self) -> Result<(), Error> {
        let request_id = self.next_request_id();

        let request = Request::ping(request_id);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Pong => Ok(()),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected pong response".to_string(),
            ))),
        })
    }

    /// Explain a query plan without executing it.
    pub async fn explain(&self, query: GraphQuery) -> Result<ExplainResult, Error> {
        let request_id = self.next_request_id();
        let schema_version = self.schema_version();

        let request = Request::explain(request_id, schema_version, query);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Explain(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected explain result".to_string(),
            ))),
        })
    }

    /// Get server metrics.
    pub async fn get_metrics(&self) -> Result<MetricsResult, Error> {
        let request_id = self.next_request_id();

        let request = Request::get_metrics(request_id);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::Metrics(result) => Ok(result),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected metrics result".to_string(),
            ))),
        })
    }

    /// Get replication status.
    pub async fn get_replication_status(&self) -> Result<ReplicationStatus, Error> {
        let request_id = self.next_request_id();

        let request = Request::get_replication_status(request_id);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::ReplicationStatus(status) => Ok(status),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected replication status".to_string(),
            ))),
        })
    }

    /// Stream changes from the changelog.
    pub async fn stream_changes(
        &self,
        from_lsn: u64,
        batch_size: u32,
        entity_filter: Option<Vec<String>>,
    ) -> Result<StreamChangesResponse, Error> {
        let request_id = self.next_request_id();

        let req = StreamChangesRequest {
            from_lsn,
            batch_size,
            entity_filter,
        };
        let request = Request::stream_changes(request_id, req);
        let response = self.send_request(&request).await?;

        self.handle_response(response, |payload| match payload {
            ResponsePayload::StreamChanges(response) => Ok(response),
            _ => Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(
                "expected stream changes response".to_string(),
            ))),
        })
    }

    /// Close the client connection.
    pub async fn close(&self) {
        let mut conn = self.connection.lock().await;
        conn.close();
    }

    /// Check if the client is connected.
    pub async fn is_connected(&self) -> bool {
        let conn = self.connection.lock().await;
        conn.is_connected()
    }

    /// Get the current schema version.
    pub fn schema_version(&self) -> u64 {
        self.schema_version.load(Ordering::SeqCst)
    }

    /// Get the server capabilities.
    pub async fn server_capabilities(&self) -> Vec<String> {
        let conn = self.connection.lock().await;
        conn.server_capabilities().to_vec()
    }

    /// Check if the server supports a capability.
    pub async fn has_capability(&self, capability: &str) -> bool {
        let conn = self.connection.lock().await;
        conn.has_capability(capability)
    }

    /// Get the server ID.
    pub async fn server_id(&self) -> String {
        let conn = self.connection.lock().await;
        conn.server_id().to_string()
    }

    /// Get the next request ID.
    fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Send a request and get the response.
    async fn send_request(&self, request: &Request) -> Result<Response, Error> {
        let conn = self.connection.lock().await;
        conn.send_request(request).await
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

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("schema_version", &self.schema_version())
            .field("next_request_id", &self.next_request_id.load(Ordering::SeqCst))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generation() {
        let id = AtomicU64::new(1);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 1);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 2);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 3);
    }

    // Integration tests would require a running server
    // Those will be added in the integration test module
}
