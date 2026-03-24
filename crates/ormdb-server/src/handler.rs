//! Request handler for processing client requests.

use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, instrument, warn};

use ormdb_core::metrics::SharedMetricsRegistry;
use ormdb_core::query::{AggregateExecutor, ExplainService};
use ormdb_proto::{
    error_codes, AggregateQuery, CacheMetrics, EntityCount, EntityQueryCount, MetricsResult,
    MutationMetrics, Operation, QueryMetrics, ReplicationRole, ReplicationStatus, Request,
    Response, StorageMetrics, StreamChangesRequest, StreamChangesResponse, TransportMetrics,
};

#[cfg(feature = "raft")]
use ormdb_raft::RaftClusterManager;

use crate::database::Database;
use crate::error::Error;
use crate::mutation::MutationExecutor;

const STATS_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Handles incoming requests and dispatches to appropriate handlers.
pub struct RequestHandler {
    database: Arc<Database>,
    metrics: Option<SharedMetricsRegistry>,
    #[cfg(feature = "raft")]
    raft_manager: Option<Arc<RaftClusterManager>>,
}

impl RequestHandler {
    /// Create a new request handler with the given database.
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            metrics: None,
            #[cfg(feature = "raft")]
            raft_manager: None,
        }
    }

    /// Create a new request handler with metrics support.
    pub fn with_metrics(database: Arc<Database>, metrics: SharedMetricsRegistry) -> Self {
        Self {
            database,
            metrics: Some(metrics),
            #[cfg(feature = "raft")]
            raft_manager: None,
        }
    }

    /// Create a new request handler with metrics and Raft support.
    #[cfg(feature = "raft")]
    pub fn with_metrics_and_raft(
        database: Arc<Database>,
        metrics: SharedMetricsRegistry,
        raft_manager: Option<Arc<RaftClusterManager>>,
    ) -> Self {
        Self {
            database,
            metrics: Some(metrics),
            raft_manager,
        }
    }

    /// Handle a request and return a response.
    #[instrument(skip(self, request), fields(request_id = request.id, op = ?std::mem::discriminant(&request.operation)))]
    pub fn handle(&self, request: &Request) -> Response {
        let start = std::time::Instant::now();
        let result = self.handle_inner(request);

        let response = match result {
            Ok(response) => response,
            Err(e) => self.error_response(request.id, e),
        };

        debug!(duration_us = start.elapsed().as_micros() as u64, success = response.status.is_ok(), "request handled");
        response
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
            Operation::Explain(query) => self.handle_explain(request.id, query),
            Operation::GetMetrics => self.handle_get_metrics(request.id),
            Operation::Aggregate(query) => self.handle_aggregate(request.id, query),
            Operation::Subscribe(_) | Operation::Unsubscribe { .. } => {
                // Pub-sub operations require async handler integration (Phase 6)
                Ok(Response::error(
                    request.id,
                    error_codes::INVALID_REQUEST,
                    "pub-sub operations not yet available on this handler",
                ))
            }
            Operation::StreamChanges(req) => self.handle_stream_changes(request.id, req),
            Operation::GetReplicationStatus => self.handle_replication_status(request.id),
            Operation::ApplySchema(bytes) => self.handle_apply_schema(request.id, bytes),
        }
    }

    /// Handle a query operation.
    #[instrument(skip(self, query), fields(entity = %query.root_entity))]
    fn handle_query(
        &self,
        request_id: u64,
        query: &ormdb_proto::GraphQuery,
    ) -> Result<Response, Error> {
        if let Err(e) = self
            .database
            .refresh_statistics_if_stale(STATS_REFRESH_INTERVAL)
        {
            warn!(error = %e, "Failed to refresh statistics");
        }

        let executor = if let Some(metrics) = &self.metrics {
            self.database.executor_with_metrics(metrics.clone())
        } else {
            self.database.executor()
        };
        let statistics = self.database.statistics();
        let cache = self.database.plan_cache();
        let result = executor
            .execute_with_cache(query, cache, Some(statistics))
            .map_err(|e| Error::Database(format!("query execution failed: {}", e)))?;

        debug!(entities_returned = result.entities.get(0).map(|e| e.len()).unwrap_or(0), "query completed");
        Ok(Response::query_ok(request_id, result))
    }

    /// Handle an aggregate query operation.
    #[instrument(skip(self, query), fields(entity = %query.root_entity))]
    fn handle_aggregate(
        &self,
        request_id: u64,
        query: &AggregateQuery,
    ) -> Result<Response, Error> {
        let executor = AggregateExecutor::new(
            self.database.storage(),
            self.database.columnar(),
        );
        let result = executor
            .execute(query)
            .map_err(|e| Error::Database(format!("aggregate query failed: {}", e)))?;

        debug!(entity = %query.root_entity, aggregations = query.aggregations.len(), "aggregate query completed");
        Ok(Response::aggregate_ok(request_id, result))
    }

    /// Handle a single mutation operation.
    #[instrument(skip(self, mutation), fields(entity = %mutation.entity(), mutation_type = ?std::mem::discriminant(mutation)))]
    fn handle_mutate(
        &self,
        request_id: u64,
        mutation: &ormdb_proto::Mutation,
    ) -> Result<Response, Error> {
        let executor = MutationExecutor::new(&self.database);
        let result = executor.execute(mutation)?;

        debug!(affected = result.affected, "mutation completed");
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

    /// Handle an apply schema request.
    fn handle_apply_schema(&self, request_id: u64, bytes: &[u8]) -> Result<Response, Error> {
        use ormdb_core::catalog::SchemaBundle;

        // Deserialize the schema from bytes
        let schema = SchemaBundle::from_bytes(bytes)
            .map_err(|e| Error::Database(format!("failed to deserialize schema: {}", e)))?;

        // Apply the schema
        self.database
            .catalog()
            .apply_schema(schema)
            .map_err(|e| Error::Database(format!("failed to apply schema: {}", e)))?;

        // Return the new version
        let version = self.database.schema_version();
        Ok(Response::schema_applied_ok(request_id, version))
    }

    /// Handle an explain request.
    fn handle_explain(
        &self,
        request_id: u64,
        query: &ormdb_proto::GraphQuery,
    ) -> Result<Response, Error> {
        if let Err(e) = self
            .database
            .refresh_statistics_if_stale(STATS_REFRESH_INTERVAL)
        {
            warn!(error = %e, "Failed to refresh statistics");
        }

        let catalog = self.database.catalog();
        let statistics = self.database.statistics();
        let cache = self.database.plan_cache();

        let service = ExplainService::new(catalog)
            .with_statistics(statistics)
            .with_cache(cache);

        let result = service
            .explain(query)
            .map_err(|e| Error::Database(format!("explain failed: {}", e)))?;

        Ok(Response::explain_ok(request_id, result))
    }

    /// Handle a get metrics request.
    fn handle_get_metrics(&self, request_id: u64) -> Result<Response, Error> {
        let result = self.collect_metrics();
        Ok(Response::metrics_ok(request_id, result))
    }

    /// Collect current server metrics.
    fn collect_metrics(&self) -> MetricsResult {
        // Get metrics from registry if available
        let (uptime_secs, query_metrics, mutations, cache) = if let Some(ref registry) = self.metrics {
            let queries_by_entity: Vec<EntityQueryCount> = registry
                .queries_by_entity()
                .into_iter()
                .map(|(entity, count)| EntityQueryCount { entity, count })
                .collect();

            (
                registry.uptime_secs(),
                QueryMetrics {
                    total_count: registry.query_count(),
                    avg_duration_us: registry.avg_query_latency_us(),
                    p50_duration_us: registry.p50_query_latency_us(),
                    p99_duration_us: registry.p99_query_latency_us(),
                    max_duration_us: registry.max_query_latency_us(),
                    by_entity: queries_by_entity,
                },
                MutationMetrics {
                    total_count: registry.mutation_count(),
                    inserts: registry.insert_count(),
                    updates: registry.update_count(),
                    deletes: registry.delete_count(),
                    upserts: registry.upsert_count(),
                    rows_affected: registry.rows_affected(),
                },
                CacheMetrics {
                    hits: registry.cache_hits(),
                    misses: registry.cache_misses(),
                    hit_rate: registry.cache_hit_rate(),
                    size: self.database.plan_cache().len() as u64,
                    capacity: 1000, // Default capacity
                    evictions: registry.cache_evictions(),
                },
            )
        } else {
            // No metrics registry, return defaults
            (
                0,
                QueryMetrics::default(),
                MutationMetrics::default(),
                CacheMetrics::default(),
            )
        };

        // Get storage metrics from statistics
        let statistics = self.database.statistics();
        let entity_counts: Vec<EntityCount> = statistics
            .snapshot()
            .into_iter()
            .map(|(entity, count)| EntityCount { entity, count })
            .collect();

        let total_entities: u64 = entity_counts.iter().map(|e| e.count).sum();

        MetricsResult::new(
            uptime_secs,
            query_metrics,
            mutations,
            cache,
            StorageMetrics {
                entity_counts,
                total_entities,
                size_bytes: None,
                active_transactions: 0,
            },
            TransportMetrics::default(),
        )
    }

    /// Handle a stream changes request (CDC/replication).
    fn handle_stream_changes(
        &self,
        request_id: u64,
        req: &StreamChangesRequest,
    ) -> Result<Response, Error> {
        let changelog = self.database.changelog();

        // Scan entries from the changelog
        let (entries, has_more) = if let Some(ref filter) = req.entity_filter {
            changelog.scan_filtered(req.from_lsn, req.batch_size as usize, Some(filter))
        } else {
            changelog.scan_batch(req.from_lsn, req.batch_size as usize)
        }
        .map_err(|e| Error::Database(format!("failed to scan changelog: {}", e)))?;

        // Calculate next LSN
        let next_lsn = entries.last().map(|e| e.lsn + 1).unwrap_or(req.from_lsn);

        let response = StreamChangesResponse::new(entries, next_lsn, has_more);
        Ok(Response::stream_changes_ok(request_id, response))
    }

    /// Handle a get replication status request.
    fn handle_replication_status(&self, request_id: u64) -> Result<Response, Error> {
        let changelog = self.database.changelog();
        let current_lsn = changelog.current_lsn();

        // For now, all servers are standalone (full replication manager comes later)
        let status = ReplicationStatus::new(ReplicationRole::Standalone, current_lsn);

        Ok(Response::replication_status_ok(request_id, status))
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
