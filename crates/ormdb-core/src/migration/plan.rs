//! Migration plan generation.
//!
//! Generates a sequence of migration steps from a schema diff.

use super::diff::{ConstraintChange, EntityChange, FieldChange, RelationChange, SchemaDiff};
use super::grader::{MigrationGrade, SafetyGrader};
use crate::catalog::{
    ConstraintDef, DefaultValue, EntityDef, FieldDef, FieldType, RelationDef, SchemaBundle,
};
use crate::storage::key::current_timestamp;

/// Generate a unique migration ID.
pub fn generate_migration_id() -> [u8; 16] {
    // Use timestamp + random bytes
    let ts = current_timestamp();
    let mut id = [0u8; 16];
    id[0..8].copy_from_slice(&ts.to_be_bytes());
    // Fill remaining bytes with pseudo-random based on timestamp
    let hash = ts.wrapping_mul(0x517cc1b727220a95);
    id[8..16].copy_from_slice(&hash.to_be_bytes());
    id
}

/// A complete migration plan.
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    /// Unique migration ID.
    pub id: [u8; 16],
    /// Source schema version.
    pub from_version: u64,
    /// Target schema version.
    pub to_version: u64,
    /// Safety grade for this migration.
    pub grade: MigrationGrade,
    /// Ordered list of migration steps.
    pub steps: Vec<MigrationStep>,
    /// When the plan was created.
    pub created_at: u64,
}

impl MigrationPlan {
    /// Generate a migration plan from two schema bundles.
    pub fn generate(from: &SchemaBundle, to: &SchemaBundle) -> Self {
        let diff = SchemaDiff::compute(from, to);
        let grade = SafetyGrader::grade(&diff);
        Self::from_diff(&diff, grade)
    }

    /// Generate a migration plan from a diff and grade.
    pub fn from_diff(diff: &SchemaDiff, grade: MigrationGrade) -> Self {
        let mut steps = Vec::new();

        // Phase 1: Expand (add new structures)
        steps.extend(Self::generate_expand_steps(diff));

        // Phase 2: Backfill (populate data)
        steps.extend(Self::generate_backfill_steps(diff, &grade));

        // Phase 3: Validate (check constraints)
        steps.extend(Self::generate_validation_steps(diff));

        // Phase 4: Contract (remove old structures)
        steps.extend(Self::generate_contract_steps(diff));

        MigrationPlan {
            id: generate_migration_id(),
            from_version: diff.from_version,
            to_version: diff.to_version,
            grade,
            steps,
            created_at: current_timestamp(),
        }
    }

    /// Get the number of steps in the plan.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Check if this plan is empty (no steps).
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get steps of a specific phase.
    pub fn steps_in_phase(&self, phase: MigrationPhase) -> Vec<&MigrationStep> {
        self.steps
            .iter()
            .filter(|s| s.phase() == phase)
            .collect()
    }

    fn generate_expand_steps(diff: &SchemaDiff) -> Vec<MigrationStep> {
        let mut steps = Vec::new();

        // Add new entities first (so fields/relations can reference them)
        for change in &diff.entity_changes {
            if let EntityChange::Added(entity) = change {
                steps.push(MigrationStep::Expand(ExpandStep::AddEntity {
                    entity: entity.clone(),
                }));
            }
        }

        // Add new fields to existing entities
        for change in &diff.entity_changes {
            if let EntityChange::Modified {
                entity_name,
                field_changes,
                ..
            } = change
            {
                for fc in field_changes {
                    if let FieldChange::Added(field) = fc {
                        steps.push(MigrationStep::Expand(ExpandStep::AddField {
                            entity_name: entity_name.clone(),
                            field: field.clone(),
                        }));
                    }

                    // Add index if field became indexed
                    if let FieldChange::IndexChanged {
                        field_name,
                        to_indexed: true,
                        ..
                    } = fc
                    {
                        steps.push(MigrationStep::Expand(ExpandStep::CreateIndex {
                            entity_name: entity_name.clone(),
                            field_name: field_name.clone(),
                        }));
                    }
                }
            }
        }

        // Add new relations
        for change in &diff.relation_changes {
            if let RelationChange::Added(relation) = change {
                steps.push(MigrationStep::Expand(ExpandStep::AddRelation {
                    relation: relation.clone(),
                }));
            }
        }

        // Add new constraints (deferred if backfill needed)
        for change in &diff.constraint_changes {
            if let ConstraintChange::Added(constraint) = change {
                let deferred = Self::constraint_needs_backfill(constraint, diff);
                steps.push(MigrationStep::Expand(ExpandStep::AddConstraint {
                    constraint: constraint.clone(),
                    deferred,
                }));
            }
        }

        steps
    }

    fn generate_backfill_steps(diff: &SchemaDiff, grade: &MigrationGrade) -> Vec<MigrationStep> {
        let mut steps = Vec::new();

        // Backfill default values for new required fields
        for change in &diff.entity_changes {
            if let EntityChange::Modified {
                entity_name,
                field_changes,
                ..
            } = change
            {
                for fc in field_changes {
                    // New required field with default
                    if let FieldChange::Added(field) = fc {
                        if field.required {
                            if let Some(default) = &field.default {
                                steps.push(MigrationStep::Backfill(BackfillStep::PopulateDefault {
                                    entity_name: entity_name.clone(),
                                    field_name: field.name.clone(),
                                    default_value: default.clone(),
                                }));
                            }
                        }
                    }

                    // Field becoming required with default
                    if let FieldChange::RequiredChanged {
                        field_name,
                        to_required: true,
                        has_default: true,
                        ..
                    } = fc
                    {
                        // Need to find the default value from the target schema
                        // For now, we'll use a placeholder - the executor will resolve this
                        steps.push(MigrationStep::Backfill(
                            BackfillStep::PopulateNullsWithDefault {
                                entity_name: entity_name.clone(),
                                field_name: field_name.clone(),
                            },
                        ));
                    }

                    // Type change that requires conversion
                    if let FieldChange::TypeChanged {
                        field_name,
                        from_type,
                        to_type,
                    } = fc
                    {
                        if let Some(transform) = Self::get_type_transform(from_type, to_type) {
                            steps.push(MigrationStep::Backfill(BackfillStep::TransformField {
                                entity_name: entity_name.clone(),
                                field_name: field_name.clone(),
                                transform,
                            }));
                        }
                    }
                }
            }
        }

        // Build indexes for new constraints
        for change in &diff.constraint_changes {
            if let ConstraintChange::Added(constraint) = change {
                if Self::constraint_needs_backfill(constraint, diff) {
                    steps.push(MigrationStep::Backfill(BackfillStep::BuildIndex {
                        entity_name: constraint.entity().to_string(),
                        constraint_name: constraint.name().to_string(),
                    }));
                }
            }
        }

        steps
    }

    fn generate_validation_steps(diff: &SchemaDiff) -> Vec<MigrationStep> {
        let mut steps = Vec::new();

        // Validate new constraints
        for change in &diff.constraint_changes {
            if let ConstraintChange::Added(constraint) = change {
                steps.push(MigrationStep::Validate(ValidateStep::CheckConstraint {
                    constraint_name: constraint.name().to_string(),
                }));
            }
        }

        // Validate data integrity for modified entities
        for change in &diff.entity_changes {
            if let EntityChange::Modified { entity_name, .. } = change {
                steps.push(MigrationStep::Validate(ValidateStep::CheckDataIntegrity {
                    entity_name: entity_name.clone(),
                }));
            }
        }

        steps
    }

    fn generate_contract_steps(diff: &SchemaDiff) -> Vec<MigrationStep> {
        let mut steps = Vec::new();

        // Remove constraints first (before removing entities/fields)
        for change in &diff.constraint_changes {
            if let ConstraintChange::Removed(constraint) = change {
                steps.push(MigrationStep::Contract(ContractStep::RemoveConstraint {
                    constraint_name: constraint.name().to_string(),
                }));
            }
        }

        // Remove relations
        for change in &diff.relation_changes {
            if let RelationChange::Removed(relation) = change {
                steps.push(MigrationStep::Contract(ContractStep::RemoveRelation {
                    relation_name: relation.name.clone(),
                }));
            }
        }

        // Remove fields from entities
        for change in &diff.entity_changes {
            if let EntityChange::Modified {
                entity_name,
                field_changes,
                ..
            } = change
            {
                for fc in field_changes {
                    if let FieldChange::Removed(field) = fc {
                        steps.push(MigrationStep::Contract(ContractStep::RemoveField {
                            entity_name: entity_name.clone(),
                            field_name: field.name.clone(),
                        }));
                    }

                    // Remove index if field became non-indexed
                    if let FieldChange::IndexChanged {
                        field_name,
                        to_indexed: false,
                        ..
                    } = fc
                    {
                        steps.push(MigrationStep::Contract(ContractStep::RemoveIndex {
                            entity_name: entity_name.clone(),
                            field_name: field_name.clone(),
                        }));
                    }
                }
            }
        }

        // Remove entities last
        for change in &diff.entity_changes {
            if let EntityChange::Removed(entity) = change {
                steps.push(MigrationStep::Contract(ContractStep::RemoveEntity {
                    entity_name: entity.name.clone(),
                }));
            }
        }

        // Enforce deferred constraints after backfill
        for change in &diff.constraint_changes {
            if let ConstraintChange::Added(constraint) = change {
                if Self::constraint_needs_backfill(constraint, diff) {
                    steps.push(MigrationStep::Contract(ContractStep::EnforceConstraint {
                        constraint_name: constraint.name().to_string(),
                    }));
                }
            }
        }

        steps
    }

    fn constraint_needs_backfill(constraint: &ConstraintDef, _diff: &SchemaDiff) -> bool {
        // Unique and FK constraints on existing data need validation
        matches!(
            constraint,
            ConstraintDef::Unique { .. } | ConstraintDef::ForeignKey { .. }
        )
    }

    fn get_type_transform(from: &FieldType, to: &FieldType) -> Option<FieldTransform> {
        match (from, to) {
            // Numeric widening
            (FieldType::Scalar(from_s), FieldType::Scalar(to_s)) => {
                use crate::catalog::ScalarType::*;
                match (from_s, to_s) {
                    (Int32, Int64) => Some(FieldTransform::TypeCast {
                        from_type: from.clone(),
                        to_type: to.clone(),
                    }),
                    (Float32, Float64) => Some(FieldTransform::TypeCast {
                        from_type: from.clone(),
                        to_type: to.clone(),
                    }),
                    (Int32, Float64) | (Int64, Float64) => Some(FieldTransform::TypeCast {
                        from_type: from.clone(),
                        to_type: to.clone(),
                    }),
                    _ => None,
                }
            }
            // Optional widening
            (FieldType::OptionalScalar(from_s), FieldType::OptionalScalar(to_s)) => {
                use crate::catalog::ScalarType::*;
                match (from_s, to_s) {
                    (Int32, Int64) | (Float32, Float64) => Some(FieldTransform::TypeCast {
                        from_type: from.clone(),
                        to_type: to.clone(),
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Phase of a migration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPhase {
    /// Expand phase: add new structures.
    Expand,
    /// Backfill phase: populate data.
    Backfill,
    /// Validate phase: check constraints.
    Validate,
    /// Contract phase: remove old structures.
    Contract,
}

impl std::fmt::Display for MigrationPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationPhase::Expand => write!(f, "expand"),
            MigrationPhase::Backfill => write!(f, "backfill"),
            MigrationPhase::Validate => write!(f, "validate"),
            MigrationPhase::Contract => write!(f, "contract"),
        }
    }
}

/// A single step in the migration.
#[derive(Debug, Clone)]
pub enum MigrationStep {
    /// Expand phase step.
    Expand(ExpandStep),
    /// Backfill phase step.
    Backfill(BackfillStep),
    /// Validate phase step.
    Validate(ValidateStep),
    /// Contract phase step.
    Contract(ContractStep),
}

impl MigrationStep {
    /// Get the phase of this step.
    pub fn phase(&self) -> MigrationPhase {
        match self {
            MigrationStep::Expand(_) => MigrationPhase::Expand,
            MigrationStep::Backfill(_) => MigrationPhase::Backfill,
            MigrationStep::Validate(_) => MigrationPhase::Validate,
            MigrationStep::Contract(_) => MigrationPhase::Contract,
        }
    }

    /// Get a description of this step.
    pub fn description(&self) -> String {
        match self {
            MigrationStep::Expand(e) => e.description(),
            MigrationStep::Backfill(b) => b.description(),
            MigrationStep::Validate(v) => v.description(),
            MigrationStep::Contract(c) => c.description(),
        }
    }
}

/// Expand phase step.
#[derive(Debug, Clone)]
pub enum ExpandStep {
    /// Add a new entity.
    AddEntity { entity: EntityDef },
    /// Add a new field to an entity.
    AddField { entity_name: String, field: FieldDef },
    /// Add a new relation.
    AddRelation { relation: RelationDef },
    /// Add a new constraint.
    AddConstraint {
        constraint: ConstraintDef,
        /// If true, enforcement is deferred until after backfill.
        deferred: bool,
    },
    /// Create an index.
    CreateIndex {
        entity_name: String,
        field_name: String,
    },
    /// Add a shadow field (for rename operations).
    AddShadowField {
        entity_name: String,
        field: FieldDef,
        original_field: String,
    },
}

impl ExpandStep {
    fn description(&self) -> String {
        match self {
            ExpandStep::AddEntity { entity } => format!("Add entity '{}'", entity.name),
            ExpandStep::AddField { entity_name, field } => {
                format!("Add field '{}.{}'", entity_name, field.name)
            }
            ExpandStep::AddRelation { relation } => format!("Add relation '{}'", relation.name),
            ExpandStep::AddConstraint { constraint, .. } => {
                format!("Add constraint '{}'", constraint.name())
            }
            ExpandStep::CreateIndex {
                entity_name,
                field_name,
            } => {
                format!("Create index on '{}.{}'", entity_name, field_name)
            }
            ExpandStep::AddShadowField {
                entity_name,
                field,
                ..
            } => {
                format!("Add shadow field '{}.{}'", entity_name, field.name)
            }
        }
    }
}

/// Backfill phase step.
#[derive(Debug, Clone)]
pub enum BackfillStep {
    /// Populate a field with its default value.
    PopulateDefault {
        entity_name: String,
        field_name: String,
        default_value: DefaultValue,
    },
    /// Populate nulls with default (for making field required).
    PopulateNullsWithDefault {
        entity_name: String,
        field_name: String,
    },
    /// Copy data from one field to another.
    CopyField {
        entity_name: String,
        from_field: String,
        to_field: String,
        transform: Option<FieldTransform>,
    },
    /// Transform field data in place.
    TransformField {
        entity_name: String,
        field_name: String,
        transform: FieldTransform,
    },
    /// Build an index for a constraint.
    BuildIndex {
        entity_name: String,
        constraint_name: String,
    },
    /// Compute a computed field's values.
    ComputeField {
        entity_name: String,
        field_name: String,
        expression: String,
    },
}

impl BackfillStep {
    fn description(&self) -> String {
        match self {
            BackfillStep::PopulateDefault {
                entity_name,
                field_name,
                ..
            } => {
                format!("Populate default for '{}.{}'", entity_name, field_name)
            }
            BackfillStep::PopulateNullsWithDefault {
                entity_name,
                field_name,
            } => {
                format!(
                    "Populate null values with default for '{}.{}'",
                    entity_name, field_name
                )
            }
            BackfillStep::CopyField {
                entity_name,
                from_field,
                to_field,
                ..
            } => {
                format!(
                    "Copy '{}.{}' to '{}.{}'",
                    entity_name, from_field, entity_name, to_field
                )
            }
            BackfillStep::TransformField {
                entity_name,
                field_name,
                ..
            } => {
                format!("Transform '{}.{}'", entity_name, field_name)
            }
            BackfillStep::BuildIndex {
                entity_name,
                constraint_name,
            } => {
                format!("Build index for '{}' on '{}'", constraint_name, entity_name)
            }
            BackfillStep::ComputeField {
                entity_name,
                field_name,
                ..
            } => {
                format!("Compute '{}.{}'", entity_name, field_name)
            }
        }
    }

    /// Get the entity name for this step.
    pub fn entity_name(&self) -> &str {
        match self {
            BackfillStep::PopulateDefault { entity_name, .. } => entity_name,
            BackfillStep::PopulateNullsWithDefault { entity_name, .. } => entity_name,
            BackfillStep::CopyField { entity_name, .. } => entity_name,
            BackfillStep::TransformField { entity_name, .. } => entity_name,
            BackfillStep::BuildIndex { entity_name, .. } => entity_name,
            BackfillStep::ComputeField { entity_name, .. } => entity_name,
        }
    }

    /// Get the field name for this step (if applicable).
    pub fn field_name(&self) -> Option<&str> {
        match self {
            BackfillStep::PopulateDefault { field_name, .. } => Some(field_name),
            BackfillStep::PopulateNullsWithDefault { field_name, .. } => Some(field_name),
            BackfillStep::CopyField { to_field, .. } => Some(to_field),
            BackfillStep::TransformField { field_name, .. } => Some(field_name),
            BackfillStep::BuildIndex { .. } => None,
            BackfillStep::ComputeField { field_name, .. } => Some(field_name),
        }
    }
}

/// Field transformation.
#[derive(Debug, Clone)]
pub enum FieldTransform {
    /// Type cast between compatible types.
    TypeCast {
        from_type: FieldType,
        to_type: FieldType,
    },
    /// Custom expression transformation.
    Expression(String),
}

/// Validate phase step.
#[derive(Debug, Clone)]
pub enum ValidateStep {
    /// Check that a constraint is satisfied.
    CheckConstraint { constraint_name: String },
    /// Check data integrity for an entity.
    CheckDataIntegrity { entity_name: String },
    /// Check that a backfill is complete.
    CheckBackfillComplete {
        entity_name: String,
        field_name: String,
    },
}

impl ValidateStep {
    fn description(&self) -> String {
        match self {
            ValidateStep::CheckConstraint { constraint_name } => {
                format!("Check constraint '{}'", constraint_name)
            }
            ValidateStep::CheckDataIntegrity { entity_name } => {
                format!("Check data integrity for '{}'", entity_name)
            }
            ValidateStep::CheckBackfillComplete {
                entity_name,
                field_name,
            } => {
                format!("Check backfill complete for '{}.{}'", entity_name, field_name)
            }
        }
    }
}

/// Contract phase step.
#[derive(Debug, Clone)]
pub enum ContractStep {
    /// Remove a field.
    RemoveField {
        entity_name: String,
        field_name: String,
    },
    /// Remove a shadow field.
    RemoveShadowField {
        entity_name: String,
        field_name: String,
    },
    /// Remove an entity.
    RemoveEntity { entity_name: String },
    /// Remove a relation.
    RemoveRelation { relation_name: String },
    /// Remove a constraint.
    RemoveConstraint { constraint_name: String },
    /// Remove an index.
    RemoveIndex {
        entity_name: String,
        field_name: String,
    },
    /// Enforce a deferred constraint.
    EnforceConstraint { constraint_name: String },
    /// Rename a field.
    RenameField {
        entity_name: String,
        from_name: String,
        to_name: String,
    },
}

impl ContractStep {
    fn description(&self) -> String {
        match self {
            ContractStep::RemoveField {
                entity_name,
                field_name,
            } => {
                format!("Remove field '{}.{}'", entity_name, field_name)
            }
            ContractStep::RemoveShadowField {
                entity_name,
                field_name,
            } => {
                format!("Remove shadow field '{}.{}'", entity_name, field_name)
            }
            ContractStep::RemoveEntity { entity_name } => {
                format!("Remove entity '{}'", entity_name)
            }
            ContractStep::RemoveRelation { relation_name } => {
                format!("Remove relation '{}'", relation_name)
            }
            ContractStep::RemoveConstraint { constraint_name } => {
                format!("Remove constraint '{}'", constraint_name)
            }
            ContractStep::RemoveIndex {
                entity_name,
                field_name,
            } => {
                format!("Remove index on '{}.{}'", entity_name, field_name)
            }
            ContractStep::EnforceConstraint { constraint_name } => {
                format!("Enforce constraint '{}'", constraint_name)
            }
            ContractStep::RenameField {
                entity_name,
                from_name,
                to_name,
            } => {
                format!(
                    "Rename '{}.{}' to '{}.{}'",
                    entity_name, from_name, entity_name, to_name
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ConstraintDef, FieldType, RelationDef, ScalarType};

    fn create_user_entity() -> EntityDef {
        EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
    }

    fn create_post_entity() -> EntityDef {
        EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new(
                "title",
                FieldType::scalar(ScalarType::String),
            ))
            .with_field(FieldDef::new(
                "author_id",
                FieldType::scalar(ScalarType::Uuid),
            ))
    }

    #[test]
    fn test_generate_plan_add_entity() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());
        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let plan = MigrationPlan::generate(&from, &to);

        assert_eq!(plan.from_version, 1);
        assert_eq!(plan.to_version, 2);
        assert!(!plan.is_empty());

        // Should have an expand step for adding the entity
        let expand_steps = plan.steps_in_phase(MigrationPhase::Expand);
        assert!(!expand_steps.is_empty());

        let has_add_entity = expand_steps.iter().any(|s| {
            matches!(s, MigrationStep::Expand(ExpandStep::AddEntity { entity }) if entity.name == "Post")
        });
        assert!(has_add_entity);
    }

    #[test]
    fn test_generate_plan_add_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_email = create_user_entity();
        user_with_email.fields.push(FieldDef::optional(
            "email",
            FieldType::scalar(ScalarType::String),
        ));

        let to = SchemaBundle::new(2).with_entity(user_with_email);

        let plan = MigrationPlan::generate(&from, &to);

        let expand_steps = plan.steps_in_phase(MigrationPhase::Expand);
        let has_add_field = expand_steps.iter().any(|s| {
            matches!(s, MigrationStep::Expand(ExpandStep::AddField { entity_name, field })
                if entity_name == "User" && field.name == "email")
        });
        assert!(has_add_field);
    }

    #[test]
    fn test_generate_plan_add_required_field_with_default() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_status = create_user_entity();
        user_with_status.fields.push(
            FieldDef::new("status", FieldType::scalar(ScalarType::String))
                .with_default(DefaultValue::String("active".into())),
        );

        let to = SchemaBundle::new(2).with_entity(user_with_status);

        let plan = MigrationPlan::generate(&from, &to);

        // Should have backfill step
        let backfill_steps = plan.steps_in_phase(MigrationPhase::Backfill);
        let has_populate_default = backfill_steps.iter().any(|s| {
            matches!(s, MigrationStep::Backfill(BackfillStep::PopulateDefault { entity_name, field_name, .. })
                if entity_name == "User" && field_name == "status")
        });
        assert!(has_populate_default);
    }

    #[test]
    fn test_generate_plan_add_relation() {
        let from = SchemaBundle::new(1)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let relation = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity())
            .with_relation(relation);

        let plan = MigrationPlan::generate(&from, &to);

        let expand_steps = plan.steps_in_phase(MigrationPhase::Expand);
        let has_add_relation = expand_steps.iter().any(|s| {
            matches!(s, MigrationStep::Expand(ExpandStep::AddRelation { relation })
                if relation.name == "user_posts")
        });
        assert!(has_add_relation);
    }

    #[test]
    fn test_generate_plan_add_constraint() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let constraint = ConstraintDef::unique("user_name_unique", "User", "name");

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_constraint(constraint);

        let plan = MigrationPlan::generate(&from, &to);

        // Should have expand step for constraint (deferred)
        let expand_steps = plan.steps_in_phase(MigrationPhase::Expand);
        let has_add_constraint = expand_steps.iter().any(|s| {
            matches!(s, MigrationStep::Expand(ExpandStep::AddConstraint { constraint, deferred: true })
                if constraint.name() == "user_name_unique")
        });
        assert!(has_add_constraint);

        // Should have backfill step for building index
        let backfill_steps = plan.steps_in_phase(MigrationPhase::Backfill);
        let has_build_index = backfill_steps.iter().any(|s| {
            matches!(s, MigrationStep::Backfill(BackfillStep::BuildIndex { constraint_name, .. })
                if constraint_name == "user_name_unique")
        });
        assert!(has_build_index);

        // Should have contract step for enforcing constraint
        let contract_steps = plan.steps_in_phase(MigrationPhase::Contract);
        let has_enforce = contract_steps.iter().any(|s| {
            matches!(s, MigrationStep::Contract(ContractStep::EnforceConstraint { constraint_name })
                if constraint_name == "user_name_unique")
        });
        assert!(has_enforce);
    }

    #[test]
    fn test_generate_plan_remove_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let user_without_name = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)));

        let to = SchemaBundle::new(2).with_entity(user_without_name);

        let plan = MigrationPlan::generate(&from, &to);

        let contract_steps = plan.steps_in_phase(MigrationPhase::Contract);
        let has_remove_field = contract_steps.iter().any(|s| {
            matches!(s, MigrationStep::Contract(ContractStep::RemoveField { entity_name, field_name })
                if entity_name == "User" && field_name == "name")
        });
        assert!(has_remove_field);
    }

    #[test]
    fn test_generate_plan_remove_entity() {
        let from = SchemaBundle::new(1)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let to = SchemaBundle::new(2).with_entity(create_user_entity());

        let plan = MigrationPlan::generate(&from, &to);

        let contract_steps = plan.steps_in_phase(MigrationPhase::Contract);
        let has_remove_entity = contract_steps.iter().any(|s| {
            matches!(s, MigrationStep::Contract(ContractStep::RemoveEntity { entity_name })
                if entity_name == "Post")
        });
        assert!(has_remove_entity);
    }

    #[test]
    fn test_step_description() {
        let step = MigrationStep::Expand(ExpandStep::AddEntity {
            entity: create_user_entity(),
        });
        assert!(step.description().contains("Add entity"));

        let step = MigrationStep::Backfill(BackfillStep::PopulateDefault {
            entity_name: "User".into(),
            field_name: "status".into(),
            default_value: DefaultValue::String("active".into()),
        });
        assert!(step.description().contains("Populate default"));
    }

    #[test]
    fn test_migration_phases() {
        assert_eq!(MigrationPhase::Expand.to_string(), "expand");
        assert_eq!(MigrationPhase::Backfill.to_string(), "backfill");
        assert_eq!(MigrationPhase::Validate.to_string(), "validate");
        assert_eq!(MigrationPhase::Contract.to_string(), "contract");
    }

    #[test]
    fn test_empty_migration() {
        let schema = SchemaBundle::new(1).with_entity(create_user_entity());

        let diff = SchemaDiff::compute(&schema, &schema);
        let grade = SafetyGrader::grade(&diff);
        let plan = MigrationPlan::from_diff(&diff, grade);

        // Should be empty or only have validation steps
        let expand_steps = plan.steps_in_phase(MigrationPhase::Expand);
        let backfill_steps = plan.steps_in_phase(MigrationPhase::Backfill);
        let contract_steps = plan.steps_in_phase(MigrationPhase::Contract);

        assert!(expand_steps.is_empty());
        assert!(backfill_steps.is_empty());
        assert!(contract_steps.is_empty());
    }
}
