//! Request and response message types.

use crate::explain::ExplainResult;
use crate::metrics::MetricsResult;
use crate::mutation::{Mutation, MutationBatch};
use crate::query::{AggregateQuery, GraphQuery};
use crate::replication::{ReplicationStatus, StreamChangesRequest, StreamChangesResponse};
use crate::result::{AggregateResult, MutationResult, QueryResult};
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// A request from client to server.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Request {
    /// Unique request identifier for correlation.
    pub id: u64,
    /// Schema version the client expects.
    pub schema_version: u64,
    /// The operation to perform.
    pub operation: Operation,
}

/// Operations that can be requested.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum Operation {
    /// Execute a graph query.
    Query(GraphQuery),
    /// Execute a single mutation.
    Mutate(Mutation),
    /// Execute a batch of mutations atomically.
    MutateBatch(MutationBatch),
    /// Get the current schema.
    GetSchema,
    /// Ping the server (for health checks).
    Ping,
    /// Subscribe to changes on an entity or relation.
    Subscribe(Subscription),
    /// Unsubscribe from a previous subscription.
    Unsubscribe {
        /// The subscription ID to cancel.
        subscription_id: u64,
    },
    /// Explain a query plan without executing it.
    Explain(GraphQuery),
    /// Get server metrics.
    GetMetrics,
    /// Execute an aggregate query.
    Aggregate(AggregateQuery),
    /// Stream changes from the changelog (CDC/replication).
    StreamChanges(StreamChangesRequest),
    /// Get replication status.
    GetReplicationStatus,
}

/// A subscription request for change notifications.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Subscription {
    /// The entity type to watch for changes.
    pub entity: String,
    /// Optional filter to limit which changes are sent.
    pub filter: Option<crate::query::Filter>,
    /// Optional list of fields to include in change notifications.
    pub fields: Option<Vec<String>>,
    /// Include related entity changes.
    pub include_relations: bool,
}

impl Request {
    /// Create a query request.
    pub fn query(id: u64, schema_version: u64, query: GraphQuery) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::Query(query),
        }
    }

    /// Create a mutation request.
    pub fn mutate(id: u64, schema_version: u64, mutation: Mutation) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::Mutate(mutation),
        }
    }

    /// Create a batch mutation request.
    pub fn mutate_batch(id: u64, schema_version: u64, batch: MutationBatch) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::MutateBatch(batch),
        }
    }

    /// Create a get schema request.
    pub fn get_schema(id: u64) -> Self {
        Self {
            id,
            schema_version: 0, // Not relevant for schema fetch
            operation: Operation::GetSchema,
        }
    }

    /// Create a ping request.
    pub fn ping(id: u64) -> Self {
        Self {
            id,
            schema_version: 0,
            operation: Operation::Ping,
        }
    }

    /// Create a subscribe request.
    pub fn subscribe(id: u64, schema_version: u64, subscription: Subscription) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::Subscribe(subscription),
        }
    }

    /// Create an unsubscribe request.
    pub fn unsubscribe(id: u64, subscription_id: u64) -> Self {
        Self {
            id,
            schema_version: 0,
            operation: Operation::Unsubscribe { subscription_id },
        }
    }

    /// Create an explain request.
    pub fn explain(id: u64, schema_version: u64, query: GraphQuery) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::Explain(query),
        }
    }

    /// Create a get metrics request.
    pub fn get_metrics(id: u64) -> Self {
        Self {
            id,
            schema_version: 0,
            operation: Operation::GetMetrics,
        }
    }

    /// Create an aggregate query request.
    pub fn aggregate(id: u64, schema_version: u64, query: AggregateQuery) -> Self {
        Self {
            id,
            schema_version,
            operation: Operation::Aggregate(query),
        }
    }

    /// Create a stream changes request (CDC/replication).
    pub fn stream_changes(id: u64, request: StreamChangesRequest) -> Self {
        Self {
            id,
            schema_version: 0,
            operation: Operation::StreamChanges(request),
        }
    }

    /// Create a get replication status request.
    pub fn get_replication_status(id: u64) -> Self {
        Self {
            id,
            schema_version: 0,
            operation: Operation::GetReplicationStatus,
        }
    }
}

impl Subscription {
    /// Create a new subscription for an entity.
    pub fn new(entity: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
            filter: None,
            fields: None,
            include_relations: false,
        }
    }

    /// Add a filter to the subscription.
    pub fn with_filter(mut self, filter: crate::query::Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Specify which fields to include in change notifications.
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = Some(fields);
        self
    }

    /// Include related entity changes.
    pub fn with_relations(mut self) -> Self {
        self.include_relations = true;
        self
    }
}

/// A response from server to client.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Response {
    /// Request ID this response correlates to.
    pub id: u64,
    /// Response status.
    pub status: Status,
    /// Response payload.
    pub payload: ResponsePayload,
}

/// Response status.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum Status {
    /// Request succeeded.
    Ok,
    /// Request failed with an error.
    Error {
        /// Error code for programmatic handling.
        code: u32,
        /// Human-readable error message.
        message: String,
    },
}

impl Status {
    /// Create a success status.
    pub fn ok() -> Self {
        Status::Ok
    }

    /// Create an error status.
    pub fn error(code: u32, message: impl Into<String>) -> Self {
        Status::Error {
            code,
            message: message.into(),
        }
    }

    /// Check if this is a success status.
    pub fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }

    /// Check if this is an error status.
    pub fn is_error(&self) -> bool {
        matches!(self, Status::Error { .. })
    }
}

/// Response payload variants.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum ResponsePayload {
    /// Query result.
    Query(QueryResult),
    /// Mutation result.
    Mutation(MutationResult),
    /// Schema data (serialized SchemaBundle).
    Schema {
        /// Schema version.
        version: u64,
        /// Serialized schema data.
        data: Vec<u8>,
    },
    /// Pong response to ping.
    Pong,
    /// Subscription confirmed.
    SubscriptionConfirmed {
        /// The assigned subscription ID.
        subscription_id: u64,
    },
    /// Subscription cancelled.
    Unsubscribed,
    /// Empty payload (for errors).
    Empty,
    /// Query explanation result.
    Explain(ExplainResult),
    /// Server metrics result.
    Metrics(MetricsResult),
    /// Aggregate query result.
    Aggregate(AggregateResult),
    /// Stream changes response (CDC/replication).
    StreamChanges(StreamChangesResponse),
    /// Replication status response.
    ReplicationStatus(ReplicationStatus),
}

/// A change event for pub-sub notifications.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct ChangeEvent {
    /// The subscription ID this event relates to.
    pub subscription_id: u64,
    /// The type of change.
    pub change_type: ChangeType,
    /// The entity type that changed.
    pub entity: String,
    /// The ID of the changed entity.
    pub entity_id: [u8; 16],
    /// The fields that changed (if available).
    pub changed_fields: Vec<String>,
    /// Schema version when the change occurred.
    pub schema_version: u64,
}

/// Types of changes that can occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum ChangeType {
    /// A new entity was inserted.
    Insert,
    /// An existing entity was updated.
    Update,
    /// An entity was deleted.
    Delete,
}

impl Response {
    /// Create a successful query response.
    pub fn query_ok(id: u64, result: QueryResult) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Query(result),
        }
    }

    /// Create a successful mutation response.
    pub fn mutation_ok(id: u64, result: MutationResult) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Mutation(result),
        }
    }

    /// Create a schema response.
    pub fn schema_ok(id: u64, version: u64, data: Vec<u8>) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Schema { version, data },
        }
    }

    /// Create a pong response.
    pub fn pong(id: u64) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Pong,
        }
    }

    /// Create an error response.
    pub fn error(id: u64, code: u32, message: impl Into<String>) -> Self {
        Self {
            id,
            status: Status::error(code, message),
            payload: ResponsePayload::Empty,
        }
    }

    /// Create a subscription confirmed response.
    pub fn subscription_confirmed(id: u64, subscription_id: u64) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::SubscriptionConfirmed { subscription_id },
        }
    }

    /// Create an unsubscribed response.
    pub fn unsubscribed(id: u64) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Unsubscribed,
        }
    }

    /// Create a successful explain response.
    pub fn explain_ok(id: u64, result: ExplainResult) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Explain(result),
        }
    }

    /// Create a successful metrics response.
    pub fn metrics_ok(id: u64, result: MetricsResult) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Metrics(result),
        }
    }

    /// Create a successful aggregate query response.
    pub fn aggregate_ok(id: u64, result: AggregateResult) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::Aggregate(result),
        }
    }

    /// Create a successful stream changes response.
    pub fn stream_changes_ok(id: u64, result: StreamChangesResponse) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::StreamChanges(result),
        }
    }

    /// Create a successful replication status response.
    pub fn replication_status_ok(id: u64, status: ReplicationStatus) -> Self {
        Self {
            id,
            status: Status::ok(),
            payload: ResponsePayload::ReplicationStatus(status),
        }
    }
}

/// Standard error codes.
pub mod error_codes {
    /// Unknown/internal error.
    pub const INTERNAL: u32 = 1;
    /// Invalid request format.
    pub const INVALID_REQUEST: u32 = 2;
    /// Entity not found.
    pub const NOT_FOUND: u32 = 3;
    /// Constraint violation.
    pub const CONSTRAINT_VIOLATION: u32 = 4;
    /// Schema version mismatch.
    pub const SCHEMA_MISMATCH: u32 = 5;
    /// Permission denied.
    pub const PERMISSION_DENIED: u32 = 6;
    /// Transaction conflict.
    pub const CONFLICT: u32 = 7;
    /// Query budget exceeded.
    pub const BUDGET_EXCEEDED: u32 = 8;
    /// Request timeout.
    pub const TIMEOUT: u32 = 9;
    /// Write rejected because server is a read-only replica.
    pub const READ_ONLY_REPLICA: u32 = 10;
    /// Invalid LSN in replication request.
    pub const INVALID_LSN: u32 = 11;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation::FieldValue;
    use crate::query::FilterExpr;
    use crate::result::{ColumnData, EntityBlock};

    #[test]
    fn test_query_request() {
        let request = Request::query(
            1,
            5,
            GraphQuery::new("User").with_fields(vec!["id".into(), "name".into()]),
        );

        assert_eq!(request.id, 1);
        assert_eq!(request.schema_version, 5);
        if let Operation::Query(query) = &request.operation {
            assert_eq!(query.root_entity, "User");
        } else {
            panic!("Expected Query operation");
        }
    }

    #[test]
    fn test_mutation_request() {
        let request = Request::mutate(
            2,
            5,
            Mutation::insert("User", vec![FieldValue::new("name", "Alice")]),
        );

        assert_eq!(request.id, 2);
        if let Operation::Mutate(mutation) = &request.operation {
            assert_eq!(mutation.entity(), "User");
        } else {
            panic!("Expected Mutate operation");
        }
    }

    #[test]
    fn test_query_response() {
        let result = QueryResult::new(
            vec![EntityBlock::with_data(
                "User",
                vec![[1u8; 16]],
                vec![ColumnData::new("name", vec!["Alice".into()])],
            )],
            vec![],
            false,
        );

        let response = Response::query_ok(1, result);
        assert_eq!(response.id, 1);
        assert!(response.status.is_ok());

        if let ResponsePayload::Query(result) = &response.payload {
            assert_eq!(result.total_entities(), 1);
        } else {
            panic!("Expected Query payload");
        }
    }

    #[test]
    fn test_error_response() {
        let response = Response::error(42, error_codes::NOT_FOUND, "User not found");

        assert_eq!(response.id, 42);
        assert!(response.status.is_error());

        if let Status::Error { code, message } = &response.status {
            assert_eq!(*code, error_codes::NOT_FOUND);
            assert_eq!(message, "User not found");
        }
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let request = Request::query(
            100,
            1,
            GraphQuery::new("Post")
                .with_filter(FilterExpr::eq("published", true).into()),
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
        let archived = rkyv::access::<ArchivedRequest, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Request =
            rkyv::deserialize::<Request, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(request, deserialized);

        // Test response
        let response = Response::error(100, error_codes::INVALID_REQUEST, "Bad query");

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&response).unwrap();
        let archived = rkyv::access::<ArchivedResponse, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Response =
            rkyv::deserialize::<Response, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(response, deserialized);
    }
}
