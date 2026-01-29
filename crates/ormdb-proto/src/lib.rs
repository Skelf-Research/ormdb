//! ORMDB Protocol types and serialization.
//!
//! This crate defines the wire protocol types for ORMDB, using rkyv for
//! zero-copy serialization.
//!
//! # Modules
//!
//! - [`value`] - Runtime value types for query parameters and results
//! - [`query`] - Query IR types for graph queries
//! - [`mutation`] - Mutation types for write operations
//! - [`result`] - Result types for query responses
//! - [`message`] - Request/response message wrappers
//! - [`handshake`] - Protocol negotiation types
//! - [`error`] - Protocol error types
//!
//! # Serialization
//!
//! All types in this crate derive `rkyv::Archive`, `rkyv::Serialize`, and
//! `rkyv::Deserialize`. Use rkyv directly for serialization:
//!
//! ```ignore
//! use ormdb_proto::{Value, GraphQuery};
//!
//! // Serialize
//! let value = Value::String("hello".into());
//! let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();
//!
//! // Deserialize
//! let archived = rkyv::access::<ArchivedValue, rkyv::rancor::Error>(&bytes).unwrap();
//! let deserialized: Value = rkyv::deserialize::<Value, rkyv::rancor::Error>(archived).unwrap();
//! ```

pub mod error;
pub mod framing;
pub mod handshake;
pub mod message;
pub mod mutation;
pub mod query;
pub mod result;
pub mod value;

pub use error::Error;

// Re-export commonly used types at crate root
pub use handshake::{Handshake, HandshakeResponse};
pub use message::{error_codes, Operation, Request, Response, ResponsePayload, Status};
pub use mutation::{FieldValue, Mutation, MutationBatch};
pub use query::{
    Filter, FilterExpr, GraphQuery, OrderDirection, OrderSpec, Pagination, RelationInclude,
    SimpleFilter,
};
pub use result::{ColumnData, Edge, EdgeBlock, EntityBlock, MutationResult, QueryResult};
pub use value::Value;

/// Protocol version for wire compatibility.
///
/// This version is included in handshake messages to ensure client and server
/// can communicate properly. When the protocol changes in incompatible ways,
/// this version should be incremented.
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_value_roundtrip() {
        let value = Value::String("hello".into());
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();
        let archived =
            rkyv::access::<value::ArchivedValue, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Value =
            rkyv::deserialize::<Value, rkyv::rancor::Error>(archived).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_request_roundtrip() {
        let request = Request::query(
            1,
            5,
            GraphQuery::new("User")
                .with_fields(vec!["id".into(), "name".into()])
                .include(RelationInclude::new("posts"))
                .with_filter(FilterExpr::eq("active", true).into())
                .with_order(OrderSpec::asc("name"))
                .with_pagination(Pagination::limit(10)),
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
        let archived =
            rkyv::access::<message::ArchivedRequest, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Request =
            rkyv::deserialize::<Request, rkyv::rancor::Error>(archived).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_response_roundtrip() {
        let response = Response::query_ok(
            1,
            QueryResult::new(
                vec![EntityBlock::with_data(
                    "User",
                    vec![[1u8; 16]],
                    vec![
                        ColumnData::new("id", vec![Value::Uuid([1u8; 16])]),
                        ColumnData::new("name", vec![Value::String("Alice".into())]),
                    ],
                )],
                vec![],
                false,
            ),
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&response).unwrap();
        let archived =
            rkyv::access::<message::ArchivedResponse, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: Response =
            rkyv::deserialize::<Response, rkyv::rancor::Error>(archived).unwrap();
        assert_eq!(response, deserialized);
    }
}
