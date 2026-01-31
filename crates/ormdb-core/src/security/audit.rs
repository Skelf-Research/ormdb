//! Audit logging infrastructure.
//!
//! Provides structured audit logging for security events.

use super::context::SecurityContext;
use super::field_security::FieldSensitivity;
use crate::storage::key::current_timestamp;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Counter for generating unique event IDs.
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique event ID using timestamp and counter.
fn generate_event_id() -> [u8; 16] {
    let ts = current_timestamp();
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut id = [0u8; 16];
    id[0..8].copy_from_slice(&ts.to_be_bytes());
    id[8..16].copy_from_slice(&counter.to_be_bytes());
    id
}

/// Types of audit events.
#[derive(Debug, Clone)]
pub enum AuditEventType {
    /// Query executed.
    Query {
        /// Entity type queried.
        entity: String,
        /// Summary of the filter (if any).
        filter_summary: Option<String>,
        /// Number of results returned.
        result_count: usize,
        /// Query duration in milliseconds.
        duration_ms: u64,
    },
    /// Mutation performed.
    Mutation {
        /// Entity type affected.
        entity: String,
        /// Type of mutation (insert, update, delete).
        operation: MutationOp,
        /// IDs of affected entities.
        entity_ids: Vec<[u8; 16]>,
    },
    /// Access was denied.
    AccessDenied {
        /// Operation that was attempted.
        operation: String,
        /// Entity type (if applicable).
        entity: Option<String>,
        /// Reason for denial.
        reason: String,
    },
    /// Field was masked in output.
    FieldMasked {
        /// Entity containing the field.
        entity: String,
        /// Field that was masked.
        field: String,
        /// Sensitivity level.
        sensitivity: FieldSensitivity,
    },
    /// RLS policy was applied.
    RlsApplied {
        /// Entity the policy was applied to.
        entity: String,
        /// Policy name.
        policy: String,
    },
    /// Authentication event.
    Authentication {
        /// Whether authentication succeeded.
        success: bool,
        /// Client ID attempting authentication.
        client_id: String,
        /// Capabilities granted (if successful).
        capabilities_granted: Vec<String>,
        /// Error message (if failed).
        error: Option<String>,
    },
    /// Connection established.
    ConnectionOpened {
        /// Remote address (if available).
        remote_addr: Option<String>,
    },
    /// Connection closed.
    ConnectionClosed {
        /// Duration of connection in seconds.
        duration_secs: u64,
        /// Number of requests handled.
        request_count: u64,
    },
}

/// Mutation operation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOp {
    /// Insert new entity.
    Insert,
    /// Update existing entity.
    Update,
    /// Upsert (insert or update).
    Upsert,
    /// Delete entity.
    Delete,
}

impl std::fmt::Display for MutationOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutationOp::Insert => write!(f, "insert"),
            MutationOp::Update => write!(f, "update"),
            MutationOp::Upsert => write!(f, "upsert"),
            MutationOp::Delete => write!(f, "delete"),
        }
    }
}

/// An audit event with metadata.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Unique event ID.
    pub id: [u8; 16],
    /// Timestamp when event occurred.
    pub timestamp: u64,
    /// Connection ID that triggered the event.
    pub connection_id: String,
    /// Client ID that triggered the event.
    pub client_id: String,
    /// Event details.
    pub event_type: AuditEventType,
}

impl AuditEvent {
    /// Create a new audit event.
    pub fn new(context: &SecurityContext, event_type: AuditEventType) -> Self {
        Self {
            id: generate_event_id(),
            timestamp: current_timestamp(),
            connection_id: context.connection_id.clone(),
            client_id: context.client_id.clone(),
            event_type,
        }
    }

    /// Create a query event.
    pub fn query(
        context: &SecurityContext,
        entity: impl Into<String>,
        filter_summary: Option<String>,
        result_count: usize,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            context,
            AuditEventType::Query {
                entity: entity.into(),
                filter_summary,
                result_count,
                duration_ms,
            },
        )
    }

    /// Create a mutation event.
    pub fn mutation(
        context: &SecurityContext,
        entity: impl Into<String>,
        operation: MutationOp,
        entity_ids: Vec<[u8; 16]>,
    ) -> Self {
        Self::new(
            context,
            AuditEventType::Mutation {
                entity: entity.into(),
                operation,
                entity_ids,
            },
        )
    }

    /// Create an access denied event.
    pub fn access_denied(
        context: &SecurityContext,
        operation: impl Into<String>,
        entity: Option<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::new(
            context,
            AuditEventType::AccessDenied {
                operation: operation.into(),
                entity,
                reason: reason.into(),
            },
        )
    }

    /// Create a field masked event.
    pub fn field_masked(
        context: &SecurityContext,
        entity: impl Into<String>,
        field: impl Into<String>,
        sensitivity: FieldSensitivity,
    ) -> Self {
        Self::new(
            context,
            AuditEventType::FieldMasked {
                entity: entity.into(),
                field: field.into(),
                sensitivity,
            },
        )
    }

    /// Create an RLS applied event.
    pub fn rls_applied(
        context: &SecurityContext,
        entity: impl Into<String>,
        policy: impl Into<String>,
    ) -> Self {
        Self::new(
            context,
            AuditEventType::RlsApplied {
                entity: entity.into(),
                policy: policy.into(),
            },
        )
    }

    /// Create an authentication event.
    pub fn authentication(
        connection_id: impl Into<String>,
        client_id: impl Into<String>,
        success: bool,
        capabilities: Vec<String>,
        error: Option<String>,
    ) -> Self {
        Self {
            id: generate_event_id(),
            timestamp: current_timestamp(),
            connection_id: connection_id.into(),
            client_id: client_id.into(),
            event_type: AuditEventType::Authentication {
                success,
                client_id: String::new(), // Already in the outer struct
                capabilities_granted: capabilities,
                error,
            },
        }
    }

    /// Format the event as a log line.
    pub fn to_log_line(&self) -> String {
        let id_hex: String = self.id.iter().map(|b| format!("{:02x}", b)).collect();
        let event_desc = match &self.event_type {
            AuditEventType::Query {
                entity,
                result_count,
                duration_ms,
                ..
            } => format!(
                "QUERY entity={} results={} duration_ms={}",
                entity, result_count, duration_ms
            ),
            AuditEventType::Mutation {
                entity,
                operation,
                entity_ids,
            } => format!(
                "MUTATION entity={} op={} count={}",
                entity,
                operation,
                entity_ids.len()
            ),
            AuditEventType::AccessDenied {
                operation,
                entity,
                reason,
            } => format!(
                "ACCESS_DENIED op={} entity={:?} reason={}",
                operation, entity, reason
            ),
            AuditEventType::FieldMasked {
                entity,
                field,
                sensitivity,
            } => format!(
                "FIELD_MASKED entity={} field={} sensitivity={:?}",
                entity, field, sensitivity
            ),
            AuditEventType::RlsApplied { entity, policy } => {
                format!("RLS_APPLIED entity={} policy={}", entity, policy)
            }
            AuditEventType::Authentication {
                success,
                capabilities_granted,
                error,
                ..
            } => {
                if *success {
                    format!(
                        "AUTH_SUCCESS capabilities=[{}]",
                        capabilities_granted.join(",")
                    )
                } else {
                    format!("AUTH_FAILED error={:?}", error)
                }
            }
            AuditEventType::ConnectionOpened { remote_addr } => {
                format!("CONN_OPEN remote={:?}", remote_addr)
            }
            AuditEventType::ConnectionClosed {
                duration_secs,
                request_count,
            } => format!(
                "CONN_CLOSE duration_secs={} requests={}",
                duration_secs, request_count
            ),
        };

        format!(
            "{} id={} conn={} client={} {}",
            self.timestamp, id_hex, self.connection_id, self.client_id, event_desc
        )
    }
}

/// Trait for audit log backends.
pub trait AuditLogger: Send + Sync {
    /// Log an audit event.
    fn log(&self, event: AuditEvent);

    /// Flush any buffered events.
    fn flush(&self) -> Result<(), AuditError>;
}

/// Audit logging error.
#[derive(Debug)]
pub struct AuditError(pub String);

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "audit error: {}", self.0)
    }
}

impl std::error::Error for AuditError {}

/// In-memory audit logger for testing.
#[derive(Debug, Default)]
pub struct MemoryAuditLogger {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl MemoryAuditLogger {
    /// Create a new memory logger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all logged events.
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get event count.
    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }
}

impl AuditLogger for MemoryAuditLogger {
    fn log(&self, event: AuditEvent) {
        self.events.lock().unwrap().push(event);
    }

    fn flush(&self) -> Result<(), AuditError> {
        Ok(())
    }
}

/// No-op audit logger that discards all events.
#[derive(Debug, Default)]
pub struct NullAuditLogger;

impl AuditLogger for NullAuditLogger {
    fn log(&self, _event: AuditEvent) {
        // Discard
    }

    fn flush(&self) -> Result<(), AuditError> {
        Ok(())
    }
}

/// Audit logger that prints to stderr.
#[derive(Debug, Default)]
pub struct StderrAuditLogger;

impl AuditLogger for StderrAuditLogger {
    fn log(&self, event: AuditEvent) {
        eprintln!("[AUDIT] {}", event.to_log_line());
    }

    fn flush(&self) -> Result<(), AuditError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::capability::{Capability, CapabilitySet, EntityScope};

    fn test_context() -> SecurityContext {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Read(EntityScope::All));
        SecurityContext::new("conn-123", "client-456", caps)
    }

    #[test]
    fn test_query_event() {
        let ctx = test_context();
        let event = AuditEvent::query(&ctx, "User", Some("status=active".into()), 42, 15);

        assert_eq!(event.connection_id, "conn-123");
        assert_eq!(event.client_id, "client-456");

        match event.event_type {
            AuditEventType::Query {
                entity,
                result_count,
                duration_ms,
                ..
            } => {
                assert_eq!(entity, "User");
                assert_eq!(result_count, 42);
                assert_eq!(duration_ms, 15);
            }
            _ => panic!("Expected Query event"),
        }
    }

    #[test]
    fn test_mutation_event() {
        let ctx = test_context();
        let ids = vec![[1u8; 16], [2u8; 16]];
        let event = AuditEvent::mutation(&ctx, "Post", MutationOp::Insert, ids);

        match event.event_type {
            AuditEventType::Mutation {
                entity,
                operation,
                entity_ids,
            } => {
                assert_eq!(entity, "Post");
                assert_eq!(operation, MutationOp::Insert);
                assert_eq!(entity_ids.len(), 2);
            }
            _ => panic!("Expected Mutation event"),
        }
    }

    #[test]
    fn test_access_denied_event() {
        let ctx = test_context();
        let event = AuditEvent::access_denied(
            &ctx,
            "write",
            Some("Admin".into()),
            "capability not granted",
        );

        match event.event_type {
            AuditEventType::AccessDenied {
                operation,
                entity,
                reason,
            } => {
                assert_eq!(operation, "write");
                assert_eq!(entity, Some("Admin".to_string()));
                assert!(reason.contains("capability"));
            }
            _ => panic!("Expected AccessDenied event"),
        }
    }

    #[test]
    fn test_memory_logger() {
        let logger = MemoryAuditLogger::new();
        let ctx = test_context();

        logger.log(AuditEvent::query(&ctx, "User", None, 10, 5));
        logger.log(AuditEvent::query(&ctx, "Post", None, 20, 8));

        assert_eq!(logger.len(), 2);

        let events = logger.events();
        assert_eq!(events.len(), 2);

        logger.clear();
        assert!(logger.is_empty());
    }

    #[test]
    fn test_event_to_log_line() {
        let ctx = test_context();
        let event = AuditEvent::query(&ctx, "User", None, 42, 15);
        let line = event.to_log_line();

        assert!(line.contains("QUERY"));
        assert!(line.contains("entity=User"));
        assert!(line.contains("results=42"));
        assert!(line.contains("conn=conn-123"));
    }

    #[test]
    fn test_null_logger() {
        let logger = NullAuditLogger;
        let ctx = test_context();

        // Should not panic
        logger.log(AuditEvent::query(&ctx, "User", None, 0, 0));
        logger.flush().unwrap();
    }
}
