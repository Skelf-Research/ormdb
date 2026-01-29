//! Request handler for processing client requests.

use std::sync::Arc;

use ormdb_proto::{error_codes, Operation, Request, Response};

use crate::database::Database;
use crate::error::Error;
use crate::mutation::MutationExecutor;

/// Handles incoming requests and dispatches to appropriate handlers.
pub struct RequestHandler {
    database: Arc<Database>,
}

impl RequestHandler {
    /// Create a new request handler with the given database.
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Handle a request and return a response.
    pub fn handle(&self, request: &Request) -> Response {
        let result = self.handle_inner(request);

        match result {
            Ok(response) => response,
            Err(e) => self.error_response(request.id, e),
        }
    }

    /// Internal handler that can return errors.
    fn handle_inner(&self, request: &Request) -> Result<Response, Error> {
        // Check schema version for operations that require it
        if matches!(
            request.operation,
            Operation::Query(_) | Operation::Mutate(_) | Operation::MutateBatch(_)
        ) {
            let server_version = self.database.schema_version();
            if request.schema_version != 0 && request.schema_version != server_version {
                return Ok(Response::error(
                    request.id,
                    error_codes::SCHEMA_MISMATCH,
                    format!(
                        "schema version mismatch: client has {}, server has {}",
                        request.schema_version, server_version
                    ),
                ));
            }
        }

        match &request.operation {
            Operation::Query(query) => self.handle_query(request.id, query),
            Operation::Mutate(mutation) => self.handle_mutate(request.id, mutation),
            Operation::MutateBatch(batch) => self.handle_batch(request.id, batch),
            Operation::GetSchema => self.handle_get_schema(request.id),
            Operation::Ping => Ok(Response::pong(request.id)),
        }
    }

    /// Handle a query operation.
    fn handle_query(
        &self,
        request_id: u64,
        query: &ormdb_proto::GraphQuery,
    ) -> Result<Response, Error> {
        let executor = self.database.executor();
        let result = executor
            .execute(query)
            .map_err(|e| Error::Database(format!("query execution failed: {}", e)))?;

        Ok(Response::query_ok(request_id, result))
    }

    /// Handle a single mutation operation.
    fn handle_mutate(
        &self,
        request_id: u64,
        mutation: &ormdb_proto::Mutation,
    ) -> Result<Response, Error> {
        let executor = MutationExecutor::new(&self.database);
        let result = executor.execute(mutation)?;

        Ok(Response::mutation_ok(request_id, result))
    }

    /// Handle a batch mutation operation.
    fn handle_batch(
        &self,
        request_id: u64,
        batch: &ormdb_proto::MutationBatch,
    ) -> Result<Response, Error> {
        let executor = MutationExecutor::new(&self.database);
        let result = executor.execute_batch(batch)?;

        Ok(Response::mutation_ok(request_id, result))
    }

    /// Handle a get schema request.
    fn handle_get_schema(&self, request_id: u64) -> Result<Response, Error> {
        let version = self.database.schema_version();

        let data = if version == 0 {
            // No schema applied yet
            Vec::new()
        } else {
            // Get the current schema and serialize it
            let schema = self
                .database
                .catalog()
                .current_schema()
                .map_err(|e| Error::Database(format!("failed to get schema: {}", e)))?
                .ok_or_else(|| {
                    Error::Database("schema version is non-zero but no schema found".to_string())
                })?;

            schema
                .to_bytes()
                .map_err(|e| Error::Database(format!("failed to serialize schema: {}", e)))?
        };

        Ok(Response::schema_ok(request_id, version, data))
    }

    /// Convert an error to an error response.
    fn error_response(&self, request_id: u64, error: Error) -> Response {
        let (code, message) = match &error {
            Error::Database(msg) => {
                if msg.contains("not found") {
                    (error_codes::NOT_FOUND, msg.clone())
                } else {
                    (error_codes::INTERNAL, msg.clone())
                }
            }
            Error::Storage(e) => (error_codes::INTERNAL, e.to_string()),
            Error::Protocol(e) => (error_codes::INVALID_REQUEST, e.to_string()),
            Error::Transport(msg) => (error_codes::INTERNAL, msg.clone()),
            Error::Config(msg) => (error_codes::INTERNAL, msg.clone()),
            Error::Io(e) => (error_codes::INTERNAL, e.to_string()),
        };

        Response::error(request_id, code, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
    use ormdb_proto::{FieldValue, GraphQuery, Mutation, MutationBatch, ResponsePayload, Status};

    fn setup_test_db() -> (tempfile::TempDir, Arc<Database>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        // Create schema
        let schema = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
                .with_field(FieldDef::new("age", FieldType::Scalar(ScalarType::Int32))),
        );
        db.catalog().apply_schema(schema).unwrap();

        (dir, Arc::new(db))
    }

    #[test]
    fn test_ping() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        let request = Request::ping(1);
        let response = handler.handle(&request);

        assert_eq!(response.id, 1);
        assert!(response.status.is_ok());
        assert!(matches!(response.payload, ResponsePayload::Pong));
    }

    #[test]
    fn test_get_schema() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        let request = Request::get_schema(2);
        let response = handler.handle(&request);

        assert_eq!(response.id, 2);
        assert!(response.status.is_ok());

        if let ResponsePayload::Schema { version, data } = &response.payload {
            assert_eq!(*version, 1);
            assert!(!data.is_empty());
        } else {
            panic!("Expected Schema payload");
        }
    }

    #[test]
    fn test_query_empty() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        let request = Request::query(3, 1, GraphQuery::new("User"));
        let response = handler.handle(&request);

        assert_eq!(response.id, 3);
        assert!(response.status.is_ok());

        if let ResponsePayload::Query(result) = &response.payload {
            assert_eq!(result.entities.len(), 1);
            assert!(result.entities[0].is_empty());
        } else {
            panic!("Expected Query payload");
        }
    }

    #[test]
    fn test_mutation_insert() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        let mutation = Mutation::insert(
            "User",
            vec![
                FieldValue::new("name", "Alice"),
                FieldValue::new("age", 30i32),
            ],
        );
        let request = Request::mutate(4, 1, mutation);
        let response = handler.handle(&request);

        assert_eq!(response.id, 4);
        assert!(response.status.is_ok());

        if let ResponsePayload::Mutation(result) = &response.payload {
            assert_eq!(result.affected, 1);
            assert_eq!(result.inserted_ids.len(), 1);
        } else {
            panic!("Expected Mutation payload");
        }
    }

    #[test]
    fn test_mutation_batch() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        let batch = MutationBatch::from_mutations(vec![
            Mutation::insert("User", vec![FieldValue::new("name", "User1")]),
            Mutation::insert("User", vec![FieldValue::new("name", "User2")]),
        ]);
        let request = Request::mutate_batch(5, 1, batch);
        let response = handler.handle(&request);

        assert_eq!(response.id, 5);
        assert!(response.status.is_ok());

        if let ResponsePayload::Mutation(result) = &response.payload {
            assert_eq!(result.affected, 2);
            assert_eq!(result.inserted_ids.len(), 2);
        } else {
            panic!("Expected Mutation payload");
        }
    }

    #[test]
    fn test_schema_mismatch() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        // Client has wrong schema version
        let request = Request::query(6, 99, GraphQuery::new("User"));
        let response = handler.handle(&request);

        assert_eq!(response.id, 6);
        assert!(response.status.is_error());

        if let Status::Error { code, message } = &response.status {
            assert_eq!(*code, error_codes::SCHEMA_MISMATCH);
            assert!(message.contains("mismatch"));
        } else {
            panic!("Expected error status");
        }
    }

    #[test]
    fn test_insert_and_query() {
        let (_dir, db) = setup_test_db();
        let handler = RequestHandler::new(db);

        // Insert a user
        let mutation = Mutation::insert(
            "User",
            vec![
                FieldValue::new("name", "Bob"),
                FieldValue::new("age", 25i32),
            ],
        );
        let insert_request = Request::mutate(7, 1, mutation);
        let insert_response = handler.handle(&insert_request);
        assert!(insert_response.status.is_ok());

        // Query users
        let query_request = Request::query(8, 1, GraphQuery::new("User"));
        let query_response = handler.handle(&query_request);

        assert!(query_response.status.is_ok());
        if let ResponsePayload::Query(result) = &query_response.payload {
            assert_eq!(result.entities[0].len(), 1);
        } else {
            panic!("Expected Query payload");
        }
    }
}
