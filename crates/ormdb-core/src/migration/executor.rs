//! Migration executor - orchestrates the migration workflow.
//!
//! Coordinates the expand, backfill, validate, and contract phases.

use super::backfill::{BackfillConfig, BackfillExecutor, BackfillProgress};
use super::diff::SchemaDiff;
use super::error::{MigrationError, SafetyGrade};
use super::grader::{MigrationGrade, SafetyGrader};
use super::plan::{
    generate_migration_id, BackfillStep, ContractStep, ExpandStep, MigrationPlan, MigrationStep,
    ValidateStep,
};
use super::state::{BackfillJobState, MigrationState, MigrationStateStore, MigrationStatus};
use crate::catalog::{Catalog, SchemaBundle};
use crate::storage::key::current_timestamp;
use crate::storage::StorageEngine;
use std::sync::Arc;

/// Migration executor configuration.
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Backfill configuration.
    pub backfill: BackfillConfig,
    /// Whether to allow Grade D (destructive) migrations.
    pub allow_destructive: bool,
    /// Whether to run in dry-run mode (no actual changes).
    pub dry_run: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            backfill: BackfillConfig::default(),
            allow_destructive: false,
            dry_run: false,
        }
    }
}

/// Result of a migration execution.
#[derive(Debug)]
pub struct MigrationResult {
    /// Migration ID.
    pub migration_id: [u8; 16],
    /// Final status.
    pub status: MigrationStatus,
    /// Steps executed.
    pub steps_executed: usize,
    /// Total steps.
    pub total_steps: usize,
    /// Backfill progress reports.
    pub backfill_progress: Vec<BackfillProgress>,
    /// Any warnings generated.
    pub warnings: Vec<String>,
}

/// Migration executor - orchestrates the migration workflow.
pub struct MigrationExecutor {
    engine: Arc<StorageEngine>,
    catalog: Arc<Catalog>,
    state_store: MigrationStateStore,
    backfill_executor: BackfillExecutor,
    config: MigrationConfig,
}

impl MigrationExecutor {
    /// Create a new migration executor.
    pub fn new(
        engine: Arc<StorageEngine>,
        catalog: Arc<Catalog>,
        db: &sled::Db,
        config: MigrationConfig,
    ) -> Result<Self, MigrationError> {
        let state_store = MigrationStateStore::open(db)?;
        let backfill_executor = BackfillExecutor::new(
            Arc::clone(&engine),
            Arc::clone(&catalog),
            config.backfill.clone(),
        );

        Ok(Self {
            engine,
            catalog,
            state_store,
            backfill_executor,
            config,
        })
    }

    /// Plan a migration between two schema versions.
    pub fn plan(&self, from: &SchemaBundle, to: &SchemaBundle) -> Result<MigrationPlan, MigrationError> {
        let diff = SchemaDiff::compute(from, to);

        if diff.is_empty() {
            return Err(MigrationError::NoChanges {
                from_version: from.version,
                to_version: to.version,
            });
        }

        let grade = SafetyGrader::grade(&diff);
        let plan = MigrationPlan::from_diff(&diff, grade);

        Ok(plan)
    }

    /// Validate that a migration plan can be executed.
    pub fn validate_plan(&self, plan: &MigrationPlan) -> Result<(), MigrationError> {
        // Check if there's already an active migration
        if let Some(active) = self.state_store.get_active()? {
            return Err(MigrationError::MigrationInProgress {
                migration_id: active.migration_id,
            });
        }

        // Check safety grade
        if plan.grade.overall_grade == SafetyGrade::D && !self.config.allow_destructive {
            return Err(MigrationError::UnsafeOperation {
                operation: "migration".to_string(),
                grade: SafetyGrade::D,
                requirement: "set allow_destructive=true to proceed".to_string(),
            });
        }

        Ok(())
    }

    /// Execute a migration plan.
    pub fn execute(&self, plan: &MigrationPlan) -> Result<MigrationResult, MigrationError> {
        // Validate before execution
        self.validate_plan(plan)?;

        // Create migration state
        let mut state = MigrationState::new(
            plan.id,
            plan.from_version,
            plan.to_version,
            plan.steps.len(),
        );

        // Start the migration
        state.start();
        self.state_store.save(&state)?;

        let mut result = MigrationResult {
            migration_id: plan.id,
            status: MigrationStatus::Expanding,
            steps_executed: 0,
            total_steps: plan.steps.len(),
            backfill_progress: Vec::new(),
            warnings: plan.grade.warnings.clone(),
        };

        // Execute each step
        for (idx, step) in plan.steps.iter().enumerate() {
            state.current_step_index = idx;

            // Update phase based on step type
            match step {
                MigrationStep::Backfill(_) => {
                    if state.status != MigrationStatus::Backfilling {
                        state.start_backfilling();
                        self.state_store.save(&state)?;
                    }
                }
                MigrationStep::Contract(_) => {
                    if state.status != MigrationStatus::Contracting {
                        state.start_contracting();
                        self.state_store.save(&state)?;
                    }
                }
                _ => {}
            }

            // Mark step as started
            if let Some(step_progress) = state.step_progress.get_mut(idx) {
                step_progress.start();
            }
            self.state_store.save(&state)?;

            // Execute the step
            match self.execute_step(step, &mut state, &mut result) {
                Ok(()) => {
                    if let Some(step_progress) = state.step_progress.get_mut(idx) {
                        step_progress.complete();
                    }
                    result.steps_executed += 1;
                }
                Err(e) => {
                    if let Some(step_progress) = state.step_progress.get_mut(idx) {
                        step_progress.fail(&e.to_string());
                    }
                    state.fail(&e.to_string());
                    self.state_store.save(&state)?;
                    result.status = MigrationStatus::Failed;
                    return Err(e);
                }
            }

            self.state_store.save(&state)?;
        }

        // Mark migration as complete
        state.complete();
        self.state_store.save(&state)?;

        result.status = MigrationStatus::Complete;

        Ok(result)
    }

    /// Execute a single migration step.
    fn execute_step(
        &self,
        step: &MigrationStep,
        state: &mut MigrationState,
        result: &mut MigrationResult,
    ) -> Result<(), MigrationError> {
        if self.config.dry_run {
            // In dry-run mode, just log and return
            return Ok(());
        }

        match step {
            MigrationStep::Expand(expand) => self.execute_expand(expand),
            MigrationStep::Backfill(backfill) => {
                let progress = self.execute_backfill(backfill, state)?;
                result.backfill_progress.push(progress);
                Ok(())
            }
            MigrationStep::Validate(validate) => self.execute_validate(validate),
            MigrationStep::Contract(contract) => self.execute_contract(contract),
        }
    }

    /// Execute an expand step.
    fn execute_expand(&self, step: &ExpandStep) -> Result<(), MigrationError> {
        match step {
            ExpandStep::AddEntity { entity } => {
                // Entity addition is handled at schema level
                // The schema bundle should already contain the new entity
                Ok(())
            }
            ExpandStep::AddField { entity_name, field } => {
                // Field addition is handled at schema level
                // Existing records will have the field as null/default
                Ok(())
            }
            ExpandStep::AddRelation { relation } => {
                // Relation addition is handled at schema level
                Ok(())
            }
            ExpandStep::AddConstraint { constraint, deferred } => {
                // If not deferred, we would enforce immediately
                // For deferred constraints, we wait until contract phase
                if !deferred {
                    // TODO: Validate existing data against constraint
                }
                Ok(())
            }
            ExpandStep::CreateIndex { entity_name, field_name } => {
                // Index creation would be handled by the index subsystem
                Ok(())
            }
            ExpandStep::AddShadowField { .. } => {
                // Shadow fields for rename operations
                Ok(())
            }
        }
    }

    /// Execute a backfill step.
    fn execute_backfill(
        &self,
        step: &BackfillStep,
        state: &mut MigrationState,
    ) -> Result<BackfillProgress, MigrationError> {
        let job_id = generate_migration_id();
        let mut job_state = BackfillJobState::new(
            job_id,
            state.migration_id,
            step.entity_name(),
            step.field_name().unwrap_or(""),
            self.config.backfill.batch_size,
        );

        let progress = self.backfill_executor.execute(step, &mut job_state, Some(&self.state_store))?;

        // Clean up job state after successful completion
        self.state_store.delete_backfill_job(&job_id)?;

        Ok(progress)
    }

    /// Execute a validate step.
    fn execute_validate(&self, step: &ValidateStep) -> Result<(), MigrationError> {
        match step {
            ValidateStep::CheckConstraint { constraint_name } => {
                // TODO: Validate constraint against all existing data
                Ok(())
            }
            ValidateStep::CheckDataIntegrity { entity_name } => {
                // TODO: Run integrity checks on entity data
                Ok(())
            }
            ValidateStep::CheckBackfillComplete { entity_name, field_name } => {
                // TODO: Verify all records have the field populated
                Ok(())
            }
        }
    }

    /// Execute a contract step.
    fn execute_contract(&self, step: &ContractStep) -> Result<(), MigrationError> {
        match step {
            ContractStep::RemoveField { entity_name, field_name } => {
                // Field removal is handled at schema level
                // Existing data remains but field is no longer accessible
                Ok(())
            }
            ContractStep::RemoveShadowField { .. } => {
                // Shadow field cleanup
                Ok(())
            }
            ContractStep::RemoveEntity { entity_name } => {
                // Entity removal at schema level
                // Data cleanup could happen here or as a separate background job
                Ok(())
            }
            ContractStep::RemoveRelation { relation_name } => {
                // Relation removal at schema level
                Ok(())
            }
            ContractStep::RemoveConstraint { constraint_name } => {
                // Constraint removal at schema level
                Ok(())
            }
            ContractStep::RemoveIndex { entity_name, field_name } => {
                // Index removal
                Ok(())
            }
            ContractStep::EnforceConstraint { constraint_name } => {
                // Enforce a previously deferred constraint
                // TODO: Validate all existing data
                Ok(())
            }
            ContractStep::RenameField { entity_name, from_name, to_name } => {
                // Field rename at schema level
                Ok(())
            }
        }
    }

    /// Rollback a migration.
    pub fn rollback(&self, migration_id: &[u8; 16]) -> Result<(), MigrationError> {
        let mut state = self
            .state_store
            .load(migration_id)?
            .ok_or(MigrationError::MigrationNotFound {
                migration_id: *migration_id,
            })?;

        if state.is_terminal() && state.status != MigrationStatus::Failed {
            return Err(MigrationError::RollbackFailed {
                reason: format!("Cannot rollback migration in {} state", state.status),
            });
        }

        // For now, just mark as rolled back
        // A full rollback would need to reverse each executed step
        state.rollback();
        self.state_store.save(&state)?;

        Ok(())
    }

    /// Get the status of a migration.
    pub fn status(&self, migration_id: &[u8; 16]) -> Result<MigrationState, MigrationError> {
        self.state_store
            .load(migration_id)?
            .ok_or(MigrationError::MigrationNotFound {
                migration_id: *migration_id,
            })
    }

    /// Resume a paused or crashed migration.
    pub fn resume(&self, migration_id: &[u8; 16], plan: &MigrationPlan) -> Result<MigrationResult, MigrationError> {
        let mut state = self
            .state_store
            .load(migration_id)?
            .ok_or(MigrationError::MigrationNotFound {
                migration_id: *migration_id,
            })?;

        if !state.can_resume() {
            return Err(MigrationError::RollbackFailed {
                reason: format!("Cannot resume migration in {} state", state.status),
            });
        }

        let mut result = MigrationResult {
            migration_id: *migration_id,
            status: state.status,
            steps_executed: state.current_step_index,
            total_steps: plan.steps.len(),
            backfill_progress: Vec::new(),
            warnings: plan.grade.warnings.clone(),
        };

        // Resume from current step
        for (idx, step) in plan.steps.iter().enumerate().skip(state.current_step_index) {
            state.current_step_index = idx;

            // Check if step was already completed
            if let Some(step_progress) = state.step_progress.get(idx) {
                if step_progress.status == super::state::StepStatus::Complete {
                    result.steps_executed += 1;
                    continue;
                }
            }

            // Execute the step
            if let Some(step_progress) = state.step_progress.get_mut(idx) {
                step_progress.start();
            }
            self.state_store.save(&state)?;

            match self.execute_step(step, &mut state, &mut result) {
                Ok(()) => {
                    if let Some(step_progress) = state.step_progress.get_mut(idx) {
                        step_progress.complete();
                    }
                    result.steps_executed += 1;
                }
                Err(e) => {
                    if let Some(step_progress) = state.step_progress.get_mut(idx) {
                        step_progress.fail(&e.to_string());
                    }
                    state.fail(&e.to_string());
                    self.state_store.save(&state)?;
                    result.status = MigrationStatus::Failed;
                    return Err(e);
                }
            }

            self.state_store.save(&state)?;
        }

        state.complete();
        self.state_store.save(&state)?;

        result.status = MigrationStatus::Complete;

        Ok(result)
    }

    /// List all migrations.
    pub fn list_migrations(&self) -> Result<Vec<MigrationState>, MigrationError> {
        self.state_store.list()
    }

    /// Get the currently active migration (if any).
    pub fn get_active_migration(&self) -> Result<Option<MigrationState>, MigrationError> {
        self.state_store.get_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
    use crate::storage::StorageConfig;
    use tempfile::tempdir;

    fn setup_test_env() -> (Arc<StorageEngine>, Arc<Catalog>, sled::Db) {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("storage");
        let catalog_path = temp_dir.path().join("catalog");

        let engine = Arc::new(
            StorageEngine::open(StorageConfig::new(&storage_path)).unwrap(),
        );

        let catalog_db = sled::open(&catalog_path).unwrap();
        let catalog = Arc::new(Catalog::open(&catalog_db).unwrap());

        let state_db = sled::open(temp_dir.path().join("state")).unwrap();

        // Keep temp_dir alive
        std::mem::forget(temp_dir);

        (engine, catalog, state_db)
    }

    fn create_user_entity() -> EntityDef {
        EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
    }

    #[test]
    fn test_migration_executor_creation() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(),
        );
        assert!(executor.is_ok());
    }

    #[test]
    fn test_plan_empty_migration() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(),
        )
        .unwrap();

        let schema = SchemaBundle::new(1).with_entity(create_user_entity());
        let result = executor.plan(&schema, &schema);

        assert!(matches!(result, Err(MigrationError::NoChanges { .. })));
    }

    #[test]
    fn test_plan_add_entity() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(),
        )
        .unwrap();

        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::scalar(ScalarType::String)));

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(post);

        let plan = executor.plan(&from, &to).unwrap();

        assert!(!plan.is_empty());
        assert_eq!(plan.from_version, 1);
        assert_eq!(plan.to_version, 2);
    }

    #[test]
    fn test_validate_destructive_migration() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(), // allow_destructive = false
        )
        .unwrap();

        let from = SchemaBundle::new(1).with_entity(create_user_entity());
        let to = SchemaBundle::new(2); // Remove all entities

        let plan = executor.plan(&from, &to).unwrap();
        let result = executor.validate_plan(&plan);

        assert!(matches!(result, Err(MigrationError::UnsafeOperation { .. })));
    }

    #[test]
    fn test_execute_simple_migration() {
        let (engine, catalog, db) = setup_test_env();

        let mut config = MigrationConfig::default();
        config.dry_run = true; // Don't actually execute

        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            config,
        )
        .unwrap();

        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_email = create_user_entity();
        user_with_email.fields.push(FieldDef::optional(
            "email",
            FieldType::scalar(ScalarType::String),
        ));

        let to = SchemaBundle::new(2).with_entity(user_with_email);

        let plan = executor.plan(&from, &to).unwrap();
        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.status, MigrationStatus::Complete);
    }

    #[test]
    fn test_list_migrations() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(),
        )
        .unwrap();

        let migrations = executor.list_migrations().unwrap();
        assert!(migrations.is_empty());
    }

    #[test]
    fn test_migration_not_found() {
        let (engine, catalog, db) = setup_test_env();
        let executor = MigrationExecutor::new(
            engine,
            catalog,
            &db,
            MigrationConfig::default(),
        )
        .unwrap();

        let fake_id = [0u8; 16];
        let result = executor.status(&fake_id);

        assert!(matches!(result, Err(MigrationError::MigrationNotFound { .. })));
    }
}
