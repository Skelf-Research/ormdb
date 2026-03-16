//! Request and response message types.

use crate::mutation::{Mutation, MutationBatch};
use crate::query::GraphQuery;
use crate::result::{MutationResult, QueryResult};
use rkyv::{Archive, Deserialize, Serialize};

/// A request from client to server.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct Request {
    /// Unique request identifier for correlation.
    pub id: u64,
    /// Schema version the client expects.
    pub schema_version: u64,
    /// The operation to perform.
    pub operation: Operation,
}

/// Operations that can be requested.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
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
}

/// A response from server to client.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct Response {
    /// Request ID this response correlates to.
    pub id: u64,
    /// Response status.
    pub status: Status,
    /// Response payload.
    pub payload: ResponsePayload,
}

/// Response status.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
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
    /// Empty payload (for errors).
    Empty,
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
