//! Migration engine for ORMDB.
//!
//! This module provides safe schema evolution with:
//! - Automatic schema diffing
//! - Safety grading (A/B/C/D)
//! - Expand/contract workflow orchestration
//! - Background backfill execution
//! - Crash recovery via state persistence
//!
//! # Safety Grades
//!
//! | Grade | Description | Examples | Behavior |
//! |-------|-------------|----------|----------|
//! | **A** | Additive, non-breaking | New optional fields, new entities | Online, no blocking |
//! | **B** | Requires backfill but compatible | New required fields with defaults | Background backfill |
//! | **C** | Breaking but migratable | Compatible type changes | Expand/contract |
//! | **D** | Destructive | Field/entity removal | Requires confirmation |
//!
//! # Example
//!
//! ```ignore
//! use ormdb_core::migration::{MigrationExecutor, MigrationConfig};
//!
//! // Create executor
//! let executor = MigrationExecutor::new(engine, catalog, &db, MigrationConfig::default())?;
//!
//! // Plan migration
//! let plan = executor.plan(&from_schema, &to_schema)?;
//!
//! // Check safety grade
//! println!("Migration grade: {}", plan.grade.overall_grade);
//!
//! // Execute if safe
//! if plan.grade.can_run_online() {
//!     let result = executor.execute(&plan)?;
//! }
//! ```

pub mod backfill;
pub mod diff;
pub mod error;
pub mod executor;
pub mod grader;
pub mod plan;
pub mod state;

// Re-export main types

// Diff types
pub use diff::{
    ConstraintChange, EntityChange, FieldChange, IdentityChange, LifecycleChange, RelationChange,
    RelationEntityChange, RelationFieldChange, SchemaDiff,
};

// Error types
pub use error::{MigrationError, SafetyGrade};

// Grader types
pub use grader::{ChangeGrade, MigrationGrade, SafetyGrader};

// Plan types
pub use plan::{
    generate_migration_id, BackfillStep, ContractStep, ExpandStep, FieldTransform, MigrationPlan,
    MigrationPhase, MigrationStep, ValidateStep,
};

// State types
pub use state::{
    BackfillJobState, MigrationState, MigrationStateStore, MigrationStatus, StepProgress,
    StepStatus,
};

// Backfill types
pub use backfill::{BackfillConfig, BackfillError, BackfillExecutor, BackfillProgress};

// Executor types
pub use executor::{MigrationConfig, MigrationExecutor, MigrationResult};
