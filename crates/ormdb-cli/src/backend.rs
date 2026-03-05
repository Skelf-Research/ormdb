//! Backend abstraction for embedded and client execution.
//!
//! Provides a unified interface for executing queries and mutations
//! regardless of whether we're in embedded or client mode.

use crate::mode::ConnectionMode;
use ormdb_proto::{AggregateResult, ExplainResult, MetricsResult, QueryResult};
use thiserror::Error;

/// Backend execution errors.
#[derive(Debug, Error)]
pub enum BackendError {
    /// Parse or compile error.
    #[error("{0}")]
    Language(String),

    /// Client communication error.
    #[error("client error: {0}")]
    Client(#[from] ormdb_client::Error),

    /// Embedded database error.
    #[error("database error: {0}")]
    Database(#[from] ormdb::Error),

    /// Not connected/opened.
    #[error("not connected")]
    NotConnected,

    /// Operation not supported in this mode.
    #[error("operation not supported: {0}")]
    NotSupported(String),
}

/// Result of a mutation operation.
pub struct MutationResult {
    pub affected: usize,
    pub inserted_ids: Vec<[u8; 16]>,
}

/// Backend enum for unified query/mutation execution.
/// Using an enum instead of a trait for simpler async handling.
pub enum Backend {
    /// Client mode backend.
    Client(ClientBackend),
    /// Embedded mode backend.
    Embedded(EmbeddedBackend),
}

impl Backend {
    /// Get the connection mode.
    pub fn mode(&self) -> &ConnectionMode {
        match self {
            Backend::Client(b) => &b.mode,
            Backend::Embedded(b) => &b.mode,
        }
    }

    /// Get the schema version.
    pub fn schema_version(&self) -> u64 {
        match self {
            Backend::Client(b) => b.client.schema_version(),
            Backend::Embedded(b) => b.db.schema_version(),
        }
    }

    /// Execute a compiled query.
    pub async fn query(&self, query: ormdb_proto::GraphQuery) -> Result<QueryResult, BackendError> {
        match self {
            Backend::Client(b) => Ok(b.client.query(query).await?),
            Backend::Embedded(b) => Ok(b.db.execute_raw(&query)?),
        }
    }

    /// Execute a compiled aggregate query.
    pub async fn aggregate(
        &self,
        query: ormdb_proto::AggregateQuery,
    ) -> Result<AggregateResult, BackendError> {
        match self {
            Backend::Client(b) => Ok(b.client.aggregate(query).await?),
            Backend::Embedded(_) => Err(BackendError::NotSupported(
                "aggregate queries not yet implemented in embedded mode".to_string(),
            )),
        }
    }

    /// Execute a mutation.
    pub async fn mutate(
        &self,
        mutation: ormdb_proto::Mutation,
    ) -> Result<MutationResult, BackendError> {
        match self {
            Backend::Client(b) => {
                let result = b.client.mutate(mutation).await?;
                Ok(MutationResult {
                    affected: result.affected as usize,
                    inserted_ids: result.inserted_ids,
                })
            }
            Backend::Embedded(b) => {
                let mut inserted_ids = Vec::new();
                let mut affected = 0;

                // Handle mutation based on variant
                match mutation {
                    ormdb_proto::Mutation::Insert { entity, data } => {
                        let mut builder = b.db.insert(&entity);
                        for field_value in data {
                            builder = builder.set(&field_value.field, field_value.value);
                        }
                        let id = builder.execute()?;
                        inserted_ids.push(id);
                        affected = 1;
                    }
                    ormdb_proto::Mutation::Update { entity, id, data } => {
                        let mut builder = b.db.update(&entity, id);
                        for field_value in data {
                            builder = builder.set(&field_value.field, field_value.value);
                        }
                        builder.execute()?;
                        affected = 1;
                    }
                    ormdb_proto::Mutation::Delete { entity, id } => {
                        b.db.delete(&entity, id).execute()?;
                        affected = 1;
                    }
                    ormdb_proto::Mutation::Upsert { entity, id, data } => {
                        if let Some(existing_id) = id {
                            // Update existing
                            let mut builder = b.db.update(&entity, existing_id);
                            for field_value in data {
                                builder = builder.set(&field_value.field, field_value.value);
                            }
                            builder.execute()?;
                        } else {
                            // Insert new
                            let mut builder = b.db.insert(&entity);
                            for field_value in data {
                                builder = builder.set(&field_value.field, field_value.value);
                            }
                            let new_id = builder.execute()?;
                            inserted_ids.push(new_id);
                        }
                        affected = 1;
                    }
                }

                Ok(MutationResult {
                    affected,
                    inserted_ids,
                })
            }
        }
    }

    /// Explain a query.
    pub async fn explain(
        &self,
        query: ormdb_proto::GraphQuery,
    ) -> Result<ExplainResult, BackendError> {
        match self {
            Backend::Client(b) => Ok(b.client.explain(query).await?),
            Backend::Embedded(_) => {
                use ormdb_proto::{BudgetSummary, CostSummary, QueryPlanSummary};
                // For embedded mode, generate a simplified explain
                Ok(ExplainResult {
                    plan: QueryPlanSummary {
                        root_entity: query.root_entity.clone(),
                        fields: vec![],
                        filter_description: Some("Embedded mode execution".to_string()),
                        filter_selectivity: None,
                        includes: vec![],
                        budget: BudgetSummary::default(),
                        order_by: vec![],
                        pagination: None,
                    },
                    cost: CostSummary::zero(),
                    joins: vec![],
                    plan_cached: false,
                    explanation: "Embedded mode: query will be executed locally".to_string(),
                })
            }
        }
    }

    /// Get metrics (client mode only).
    pub async fn get_metrics(&self) -> Result<MetricsResult, BackendError> {
        match self {
            Backend::Client(b) => Ok(b.client.get_metrics().await?),
            Backend::Embedded(_) => Err(BackendError::NotSupported(
                "metrics are only available in client mode".to_string(),
            )),
        }
    }

    /// Get schema bytes.
    pub async fn get_schema(&self) -> Result<(u64, Vec<u8>), BackendError> {
        match self {
            Backend::Client(b) => Ok(b.client.get_schema().await?),
            Backend::Embedded(b) => {
                // For embedded mode, return schema version and empty bytes
                Ok((b.db.schema_version(), vec![]))
            }
        }
    }

    /// Flush writes to disk (embedded mode only).
    pub async fn flush(&self) -> Result<(), BackendError> {
        match self {
            Backend::Client(_) => Err(BackendError::NotSupported(
                "flush is only available in embedded mode".to_string(),
            )),
            Backend::Embedded(b) => {
                b.db.flush()?;
                Ok(())
            }
        }
    }

    /// Run compaction (embedded mode only).
    pub async fn compact(&self) -> Result<String, BackendError> {
        match self {
            Backend::Client(_) => Err(BackendError::NotSupported(
                "compact is only available in embedded mode".to_string(),
            )),
            Backend::Embedded(b) => {
                let result = b.db.compact();
                Ok(format!(
                    "Compaction complete: {} versions removed, {} tombstones removed, {} bytes freed",
                    result.versions_removed, result.tombstones_removed, result.bytes_reclaimed
                ))
            }
        }
    }

    /// Close the connection/database.
    pub async fn close(&self) -> Result<(), BackendError> {
        match self {
            Backend::Client(b) => {
                b.client.close().await;
                Ok(())
            }
            Backend::Embedded(b) => {
                b.db.flush()?;
                Ok(())
            }
        }
    }

    /// Create a backup (embedded mode only).
    pub async fn backup(&self, destination: &str, incremental: bool) -> Result<String, BackendError> {
        match self {
            Backend::Client(_) => Err(BackendError::NotSupported(
                "backup is only available in embedded mode".to_string(),
            )),
            Backend::Embedded(_b) => {
                // TODO: Integrate with ormdb-backup crate
                // For now, return a placeholder message
                let backup_type = if incremental { "incremental" } else { "full" };
                Ok(format!(
                    "Backup ({}) to {} not yet implemented.\n\
                     The ormdb-backup crate provides this functionality.\n\
                     Configuration: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION",
                    backup_type, destination
                ))
            }
        }
    }

    /// Get backup status (embedded mode only).
    pub async fn backup_status(&self) -> Result<String, BackendError> {
        match self {
            Backend::Client(_) => Err(BackendError::NotSupported(
                "backup-status is only available in embedded mode".to_string(),
            )),
            Backend::Embedded(_b) => {
                // TODO: Integrate with ormdb-backup crate
                Ok("Backup status not yet implemented.\n\
                    The ormdb-backup crate provides backup/restore functionality.".to_string())
            }
        }
    }
}

/// Client-mode backend using ormdb-client.
pub struct ClientBackend {
    pub client: ormdb_client::Client,
    pub mode: ConnectionMode,
}

impl ClientBackend {
    /// Connect to a server.
    pub async fn connect(url: &str, timeout_secs: u64) -> Result<Self, BackendError> {
        let config = ormdb_client::ClientConfig::new(url)
            .with_timeout(std::time::Duration::from_secs(timeout_secs));
        let client = ormdb_client::Client::connect(config).await?;
        Ok(Self {
            client,
            mode: ConnectionMode::Client {
                url: url.to_string(),
            },
        })
    }

    /// Get the schema version.
    pub fn schema_version(&self) -> u64 {
        self.client.schema_version()
    }
}

/// Embedded-mode backend using ormdb.
pub struct EmbeddedBackend {
    pub db: ormdb::Database,
    pub mode: ConnectionMode,
}

impl EmbeddedBackend {
    /// Open an embedded database.
    pub fn open(mode: ConnectionMode) -> Result<Self, BackendError> {
        let db = match &mode {
            ConnectionMode::Embedded { path: None } => ormdb::Database::open_memory()?,
            ConnectionMode::Embedded { path: Some(p) } => ormdb::Database::open(p)?,
            ConnectionMode::Client { .. } => {
                return Err(BackendError::NotSupported(
                    "cannot open client URL as embedded database".to_string(),
                ))
            }
        };
        Ok(Self { db, mode })
    }
}

/// Create a backend from a connection mode.
pub async fn create_backend(
    mode: ConnectionMode,
    timeout_secs: u64,
) -> Result<Backend, BackendError> {
    match &mode {
        ConnectionMode::Embedded { .. } => {
            let backend = EmbeddedBackend::open(mode)?;
            Ok(Backend::Embedded(backend))
        }
        ConnectionMode::Client { url } => {
            let backend = ClientBackend::connect(url, timeout_secs).await?;
            Ok(Backend::Client(backend))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedded_backend_memory() {
        let mode = ConnectionMode::Embedded { path: None };
        let backend = create_backend(mode, 30).await.unwrap();
        assert!(backend.mode().is_embedded());
        assert_eq!(backend.schema_version(), 0);
    }
}
