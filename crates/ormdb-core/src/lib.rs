//! ORMDB Core - Storage engine, catalog, and query execution.
//!
//! This crate provides the core database functionality for ORMDB.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod catalog;
pub mod constraint;
pub mod error;
pub mod metrics;
pub mod migration;
pub mod query;
pub mod replication;
pub mod security;
pub mod storage;

pub use catalog::{
    Cardinality, Catalog, ConstraintDef, DeleteBehavior, EntityDef, FieldDef, FieldType,
    LifecycleRules, OrderBy, OrderDirection, RelationDef, ScalarType, SchemaBundle,
};
pub use constraint::{CheckEvaluator, ConstraintValidator, UniqueIndex};
pub use error::{CascadeError, ConstraintError, Error};
pub use migration::{
    BackfillConfig, BackfillExecutor, MigrationConfig, MigrationError, MigrationExecutor,
    MigrationGrade, MigrationPlan, MigrationState, MigrationStatus, SafetyGrade, SafetyGrader,
    SchemaDiff,
};
pub use storage::{Record, StorageConfig, StorageEngine, Transaction, VersionedKey};

// Replication exports
pub use replication::{ChangeLog, ReplicaApplier};

// Metrics exports
pub use metrics::{Histogram, MetricsRegistry, MutationType, SharedMetricsRegistry, new_shared_registry};

// Security exports
pub use security::{
    AuditEvent, AuditEventType, AuditLogger, Capability, CapabilityAuthenticator, CapabilityLevel,
    CapabilitySet, DefaultAuthenticator, EntityScope, FieldMasker, FieldResult, FieldSecurity,
    FieldSensitivity, MaskingStrategy, MemoryAuditLogger, PolicyStore, PolicyType, RlsFilterExpr,
    RlsOperation, RlsPolicy, RlsPolicyCompiler, SecurityBudget, SecurityContext, SecurityError,
    SecurityResult, SensitiveLevel,
};

/// Re-export protocol types.
pub use ormdb_proto as proto;
