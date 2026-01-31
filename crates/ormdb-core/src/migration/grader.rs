//! Safety grading for schema migrations.
//!
//! Analyzes a schema diff and assigns safety grades (A/B/C/D)
//! based on the impact of each change.

use super::diff::{
    ConstraintChange, EntityChange, FieldChange, IdentityChange, LifecycleChange, RelationChange,
    SchemaDiff,
};
use super::error::SafetyGrade;
use crate::catalog::{FieldType, ScalarType};
use std::collections::HashSet;

/// Detailed grading result for a single change.
#[derive(Debug, Clone)]
pub struct ChangeGrade {
    /// The safety grade for this change.
    pub grade: SafetyGrade,
    /// Description of the change.
    pub change_description: String,
    /// Reasoning for the grade.
    pub reasoning: String,
    /// Whether this change requires a backfill.
    pub requires_backfill: bool,
    /// Whether this change requires data migration.
    pub requires_data_migration: bool,
}

impl ChangeGrade {
    fn new(
        grade: SafetyGrade,
        description: impl Into<String>,
        reasoning: impl Into<String>,
    ) -> Self {
        Self {
            grade,
            change_description: description.into(),
            reasoning: reasoning.into(),
            requires_backfill: false,
            requires_data_migration: false,
        }
    }

    fn with_backfill(mut self) -> Self {
        self.requires_backfill = true;
        self
    }

    fn with_data_migration(mut self) -> Self {
        self.requires_data_migration = true;
        self
    }
}

/// Complete grading result for a migration.
#[derive(Debug, Clone)]
pub struct MigrationGrade {
    /// The overall safety grade (highest/worst grade among all changes).
    pub overall_grade: SafetyGrade,
    /// Individual grades for each change.
    pub change_grades: Vec<ChangeGrade>,
    /// Changes that are blocking (grade C or D).
    pub blocking_changes: Vec<ChangeGrade>,
    /// Warnings about the migration.
    pub warnings: Vec<String>,
}

impl MigrationGrade {
    /// Check if any backfill is required.
    pub fn requires_backfill(&self) -> bool {
        self.change_grades.iter().any(|g| g.requires_backfill)
    }

    /// Check if any data migration is required.
    pub fn requires_data_migration(&self) -> bool {
        self.change_grades.iter().any(|g| g.requires_data_migration)
    }

    /// Check if the migration can be performed online.
    pub fn can_run_online(&self) -> bool {
        self.overall_grade <= SafetyGrade::B
    }
}

/// Grades schema diffs for safety.
pub struct SafetyGrader;

impl SafetyGrader {
    /// Grade a schema diff.
    pub fn grade(diff: &SchemaDiff) -> MigrationGrade {
        let mut change_grades = Vec::new();
        let mut warnings = Vec::new();
        let mut overall_grade = SafetyGrade::A;

        // Grade entity changes
        for change in &diff.entity_changes {
            let grade = Self::grade_entity_change(change);
            overall_grade = overall_grade.max(grade.grade);
            change_grades.push(grade);
        }

        // Grade relation changes
        for change in &diff.relation_changes {
            let grade = Self::grade_relation_change(change);
            overall_grade = overall_grade.max(grade.grade);
            change_grades.push(grade);
        }

        // Grade constraint changes
        for change in &diff.constraint_changes {
            let grade = Self::grade_constraint_change(change);
            overall_grade = overall_grade.max(grade.grade);
            change_grades.push(grade);
        }

        // Collect blocking changes
        let blocking_changes: Vec<_> = change_grades
            .iter()
            .filter(|g| g.grade >= SafetyGrade::C)
            .cloned()
            .collect();

        // Generate warnings
        warnings.extend(Self::generate_warnings(&change_grades, diff));

        MigrationGrade {
            overall_grade,
            change_grades,
            blocking_changes,
            warnings,
        }
    }

    fn grade_entity_change(change: &EntityChange) -> ChangeGrade {
        match change {
            EntityChange::Added(entity) => ChangeGrade::new(
                SafetyGrade::A,
                format!("Add entity '{}'", entity.name),
                "Adding new entities is non-breaking",
            ),

            EntityChange::Removed(entity) => ChangeGrade::new(
                SafetyGrade::D,
                format!("Remove entity '{}'", entity.name),
                "Removing entities destroys data and breaks clients",
            )
            .with_data_migration(),

            EntityChange::Modified {
                entity_name,
                field_changes,
                identity_changed,
                lifecycle_changed,
            } => {
                // Identity change is always Grade D
                if let Some(id_change) = identity_changed {
                    return ChangeGrade::new(
                        SafetyGrade::D,
                        format!(
                            "Change identity field of '{}' from '{}' to '{}'",
                            entity_name, id_change.from_field, id_change.to_field
                        ),
                        "Changing identity field breaks referential integrity",
                    )
                    .with_data_migration();
                }

                // Grade lifecycle changes
                if let Some(lc_change) = lifecycle_changed {
                    let lc_grade = Self::grade_lifecycle_change(entity_name, lc_change);
                    if lc_grade.grade >= SafetyGrade::C {
                        return lc_grade;
                    }
                }

                // Grade based on worst field change
                let mut max_grade = SafetyGrade::A;
                let mut max_change = None;

                for fc in field_changes {
                    let fg = Self::grade_field_change(entity_name, fc);
                    if fg.grade > max_grade {
                        max_grade = fg.grade;
                        max_change = Some(fg);
                    }
                }

                if let Some(change) = max_change {
                    change
                } else {
                    ChangeGrade::new(
                        SafetyGrade::A,
                        format!("Modify entity '{}'", entity_name),
                        "Minor changes to entity",
                    )
                }
            }
        }
    }

    fn grade_field_change(entity_name: &str, change: &FieldChange) -> ChangeGrade {
        match change {
            FieldChange::Added(field) => {
                if !field.required {
                    // Optional field: Grade A
                    ChangeGrade::new(
                        SafetyGrade::A,
                        format!("Add optional field '{}.{}'", entity_name, field.name),
                        "Optional fields don't affect existing data",
                    )
                } else if field.default.is_some() {
                    // Required with default: Grade B
                    ChangeGrade::new(
                        SafetyGrade::B,
                        format!(
                            "Add required field '{}.{}' with default",
                            entity_name, field.name
                        ),
                        "Requires background backfill to populate defaults",
                    )
                    .with_backfill()
                } else {
                    // Required without default: Grade D
                    ChangeGrade::new(
                        SafetyGrade::D,
                        format!(
                            "Add required field '{}.{}' without default",
                            entity_name, field.name
                        ),
                        "Cannot add required field without default to existing data",
                    )
                    .with_data_migration()
                }
            }

            FieldChange::Removed(field) => ChangeGrade::new(
                SafetyGrade::D,
                format!("Remove field '{}.{}'", entity_name, field.name),
                "Removing fields destroys data",
            )
            .with_data_migration(),

            FieldChange::TypeChanged {
                field_name,
                from_type,
                to_type,
            } => {
                let grade = Self::grade_type_change(from_type, to_type);
                let mut change_grade = ChangeGrade::new(
                    grade,
                    format!(
                        "Change type of '{}.{}' from {:?} to {:?}",
                        entity_name, field_name, from_type, to_type
                    ),
                    match grade {
                        SafetyGrade::A => "Type is unchanged or equivalent",
                        SafetyGrade::B => "Type widening is safe with background conversion",
                        SafetyGrade::C => "Type change requires data migration",
                        SafetyGrade::D => "Incompatible type change",
                    },
                );

                if grade >= SafetyGrade::B {
                    change_grade = change_grade.with_backfill();
                }
                if grade >= SafetyGrade::C {
                    change_grade = change_grade.with_data_migration();
                }

                change_grade
            }

            FieldChange::RequiredChanged {
                field_name,
                from_required,
                to_required,
                has_default,
            } => {
                if *from_required && !*to_required {
                    // Making optional: Grade A
                    ChangeGrade::new(
                        SafetyGrade::A,
                        format!("Make '{}.{}' optional", entity_name, field_name),
                        "Making fields optional is non-breaking",
                    )
                } else if *has_default {
                    // Making required with default: Grade B
                    ChangeGrade::new(
                        SafetyGrade::B,
                        format!(
                            "Make '{}.{}' required (has default)",
                            entity_name, field_name
                        ),
                        "Requires backfill of default for NULL values",
                    )
                    .with_backfill()
                } else {
                    // Making required without default: Grade D
                    ChangeGrade::new(
                        SafetyGrade::D,
                        format!(
                            "Make '{}.{}' required (no default)",
                            entity_name, field_name
                        ),
                        "Cannot enforce NOT NULL on existing NULL values without default",
                    )
                    .with_data_migration()
                }
            }

            FieldChange::DefaultChanged { field_name, .. } => ChangeGrade::new(
                SafetyGrade::A,
                format!("Change default for '{}.{}'", entity_name, field_name),
                "Default changes only affect new records",
            ),

            FieldChange::IndexChanged {
                field_name,
                from_indexed,
                to_indexed,
            } => {
                if *to_indexed && !*from_indexed {
                    // Adding index: Grade B (needs background index build)
                    ChangeGrade::new(
                        SafetyGrade::B,
                        format!("Add index on '{}.{}'", entity_name, field_name),
                        "Index build runs in background",
                    )
                    .with_backfill()
                } else {
                    // Removing index: Grade A
                    ChangeGrade::new(
                        SafetyGrade::A,
                        format!("Remove index from '{}.{}'", entity_name, field_name),
                        "Removing indexes is safe",
                    )
                }
            }

            FieldChange::ComputedChanged { field_name, .. } => {
                // Changing computed fields may require recomputation
                ChangeGrade::new(
                    SafetyGrade::B,
                    format!("Change computed field '{}.{}'", entity_name, field_name),
                    "Computed field changes may require recomputation",
                )
                .with_backfill()
            }
        }
    }

    fn grade_lifecycle_change(entity_name: &str, change: &LifecycleChange) -> ChangeGrade {
        if let Some((from_soft, to_soft)) = change.soft_delete_changed {
            if !from_soft && to_soft {
                // Enabling soft delete: Grade A
                return ChangeGrade::new(
                    SafetyGrade::A,
                    format!("Enable soft delete for '{}'", entity_name),
                    "Enabling soft delete is non-breaking",
                );
            } else if from_soft && !to_soft {
                // Disabling soft delete: Grade C
                return ChangeGrade::new(
                    SafetyGrade::C,
                    format!("Disable soft delete for '{}'", entity_name),
                    "Disabling soft delete changes deletion semantics",
                );
            }
        }

        ChangeGrade::new(
            SafetyGrade::A,
            format!("Change lifecycle rules for '{}'", entity_name),
            "Minor lifecycle changes",
        )
    }

    fn grade_relation_change(change: &RelationChange) -> ChangeGrade {
        match change {
            RelationChange::Added(relation) => ChangeGrade::new(
                SafetyGrade::A,
                format!("Add relation '{}'", relation.name),
                "Adding relations is non-breaking",
            ),

            RelationChange::Removed(relation) => ChangeGrade::new(
                SafetyGrade::D,
                format!("Remove relation '{}'", relation.name),
                "Removing relations breaks referential integrity",
            ),

            RelationChange::Modified {
                relation_name,
                cardinality_changed,
                delete_behavior_changed,
                entities_changed,
                ..
            } => {
                // Entity changes are Grade D
                if entities_changed.is_some() {
                    return ChangeGrade::new(
                        SafetyGrade::D,
                        format!("Change entities in relation '{}'", relation_name),
                        "Changing relation entities breaks referential integrity",
                    )
                    .with_data_migration();
                }

                // Cardinality changes are Grade D
                if cardinality_changed.is_some() {
                    return ChangeGrade::new(
                        SafetyGrade::D,
                        format!("Change cardinality of relation '{}'", relation_name),
                        "Changing cardinality may violate existing data",
                    )
                    .with_data_migration();
                }

                // Delete behavior changes
                if let Some((from, to)) = delete_behavior_changed {
                    return ChangeGrade::new(
                        SafetyGrade::B,
                        format!(
                            "Change delete behavior of '{}' from {:?} to {:?}",
                            relation_name, from, to
                        ),
                        "Delete behavior changes affect future deletes only",
                    );
                }

                ChangeGrade::new(
                    SafetyGrade::A,
                    format!("Modify relation '{}'", relation_name),
                    "Minor relation changes",
                )
            }
        }
    }

    fn grade_constraint_change(change: &ConstraintChange) -> ChangeGrade {
        match change {
            ConstraintChange::Added(constraint) => {
                let name = constraint.name();
                match constraint {
                    crate::catalog::ConstraintDef::Unique { entity, fields, .. } => {
                        ChangeGrade::new(
                            SafetyGrade::B,
                            format!(
                                "Add unique constraint '{}' on {}.{:?}",
                                name, entity, fields
                            ),
                            "Existing data must be checked for uniqueness violations",
                        )
                        .with_backfill()
                    }
                    crate::catalog::ConstraintDef::ForeignKey {
                        entity,
                        field,
                        references_entity,
                        ..
                    } => ChangeGrade::new(
                        SafetyGrade::B,
                        format!(
                            "Add foreign key constraint '{}' on {}.{} -> {}",
                            name, entity, field, references_entity
                        ),
                        "Existing data must be checked for referential integrity",
                    )
                    .with_backfill(),
                    crate::catalog::ConstraintDef::Check {
                        entity, expression, ..
                    } => ChangeGrade::new(
                        SafetyGrade::B,
                        format!("Add check constraint '{}' on {} ({})", name, entity, expression),
                        "Existing data must be validated against the check expression",
                    )
                    .with_backfill(),
                }
            }

            ConstraintChange::Removed(constraint) => ChangeGrade::new(
                SafetyGrade::A,
                format!("Remove constraint '{}'", constraint.name()),
                "Removing constraints is safe",
            ),

            ConstraintChange::Modified {
                constraint_name, ..
            } => ChangeGrade::new(
                SafetyGrade::B,
                format!("Modify constraint '{}'", constraint_name),
                "Constraint modifications require validation of existing data",
            )
            .with_backfill(),
        }
    }

    /// Grade a type change based on compatibility.
    fn grade_type_change(from: &FieldType, to: &FieldType) -> SafetyGrade {
        // Same type: Grade A
        if from == to {
            return SafetyGrade::A;
        }

        match (from, to) {
            // Making scalar optional: Grade A
            (FieldType::Scalar(s1), FieldType::OptionalScalar(s2)) if s1 == s2 => SafetyGrade::A,

            // Making optional scalar required (must have default handled elsewhere)
            (FieldType::OptionalScalar(s1), FieldType::Scalar(s2)) if s1 == s2 => SafetyGrade::C,

            // Widening numeric types: Grade B
            (FieldType::Scalar(ScalarType::Int32), FieldType::Scalar(ScalarType::Int64)) => {
                SafetyGrade::B
            }
            (FieldType::Scalar(ScalarType::Float32), FieldType::Scalar(ScalarType::Float64)) => {
                SafetyGrade::B
            }
            (FieldType::Scalar(ScalarType::Int32), FieldType::Scalar(ScalarType::Float64)) => {
                SafetyGrade::B
            }
            (FieldType::Scalar(ScalarType::Int64), FieldType::Scalar(ScalarType::Float64)) => {
                SafetyGrade::B
            }

            // Optional widening
            (
                FieldType::OptionalScalar(ScalarType::Int32),
                FieldType::OptionalScalar(ScalarType::Int64),
            ) => SafetyGrade::B,
            (
                FieldType::OptionalScalar(ScalarType::Float32),
                FieldType::OptionalScalar(ScalarType::Float64),
            ) => SafetyGrade::B,

            // Narrowing numeric types: Grade C (may lose precision)
            (FieldType::Scalar(ScalarType::Int64), FieldType::Scalar(ScalarType::Int32)) => {
                SafetyGrade::C
            }
            (FieldType::Scalar(ScalarType::Float64), FieldType::Scalar(ScalarType::Float32)) => {
                SafetyGrade::C
            }

            // String to larger container: Grade B
            (FieldType::Scalar(ScalarType::String), FieldType::Scalar(ScalarType::Bytes)) => {
                SafetyGrade::B
            }

            // Enum modifications
            (
                FieldType::Enum {
                    variants: v1,
                    name: n1,
                },
                FieldType::Enum {
                    variants: v2,
                    name: n2,
                },
            ) if n1 == n2 => {
                let v1_set: HashSet<_> = v1.iter().collect();
                let v2_set: HashSet<_> = v2.iter().collect();

                if v1_set.is_subset(&v2_set) {
                    // Adding variants only: Grade A
                    SafetyGrade::A
                } else if v2_set.is_subset(&v1_set) {
                    // Removing variants: Grade D
                    SafetyGrade::D
                } else {
                    // Mixed changes: Grade D
                    SafetyGrade::D
                }
            }

            // Array changes
            (FieldType::ArrayScalar(s1), FieldType::ArrayScalar(s2)) => {
                // Use scalar grading
                Self::grade_type_change(
                    &FieldType::Scalar(s1.clone()),
                    &FieldType::Scalar(s2.clone()),
                )
            }

            // All other changes: Grade D (incompatible)
            _ => SafetyGrade::D,
        }
    }

    fn generate_warnings(grades: &[ChangeGrade], diff: &SchemaDiff) -> Vec<String> {
        let mut warnings = Vec::new();

        // Warn about multiple destructive changes
        let destructive_count = grades.iter().filter(|g| g.grade == SafetyGrade::D).count();
        if destructive_count > 1 {
            warnings.push(format!(
                "Multiple destructive changes ({}) detected - consider breaking into smaller migrations",
                destructive_count
            ));
        }

        // Warn about large backfill requirements
        let backfill_count = grades.iter().filter(|g| g.requires_backfill).count();
        if backfill_count > 3 {
            warnings.push(format!(
                "Multiple backfill operations ({}) may take significant time",
                backfill_count
            ));
        }

        // Warn about removing entities with relations
        for change in &diff.entity_changes {
            if let EntityChange::Removed(entity) = change {
                let related = diff
                    .relation_changes
                    .iter()
                    .filter(|r| match r {
                        RelationChange::Removed(rel) => {
                            rel.from_entity == entity.name || rel.to_entity == entity.name
                        }
                        _ => false,
                    })
                    .count();
                if related > 0 {
                    warnings.push(format!(
                        "Removing entity '{}' also removes {} relation(s)",
                        entity.name, related
                    ));
                }
            }
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{
        ConstraintDef, DefaultValue, EntityDef, FieldDef, FieldType, RelationDef, ScalarType,
        SchemaBundle,
    };

    fn create_user_entity() -> EntityDef {
        EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
    }

    #[test]
    fn test_grade_a_add_optional_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_age = create_user_entity();
        user_with_age.fields.push(FieldDef::optional(
            "age",
            FieldType::scalar(ScalarType::Int32),
        ));

        let to = SchemaBundle::new(2).with_entity(user_with_age);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::A);
        assert!(grade.can_run_online());
        assert!(!grade.requires_backfill());
    }

    #[test]
    fn test_grade_a_add_entity() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new(
                "title",
                FieldType::scalar(ScalarType::String),
            ));

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(post);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::A);
    }

    #[test]
    fn test_grade_b_add_required_with_default() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_status = create_user_entity();
        user_with_status.fields.push(
            FieldDef::new("status", FieldType::scalar(ScalarType::String))
                .with_default(DefaultValue::String("active".into())),
        );

        let to = SchemaBundle::new(2).with_entity(user_with_status);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::B);
        assert!(grade.can_run_online());
        assert!(grade.requires_backfill());
    }

    #[test]
    fn test_grade_b_add_index() {
        let from =
            SchemaBundle::new(1).with_entity(
                EntityDef::new("User", "id")
                    .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                    .with_field(FieldDef::new("email", FieldType::scalar(ScalarType::String))),
            );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("email", FieldType::scalar(ScalarType::String)).with_index()),
        );

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::B);
        assert!(grade.requires_backfill());
    }

    #[test]
    fn test_grade_b_widen_type() {
        let from = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("age", FieldType::scalar(ScalarType::Int32))),
        );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("age", FieldType::scalar(ScalarType::Int64))),
        );

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::B);
    }

    #[test]
    fn test_grade_c_narrow_type() {
        let from = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("age", FieldType::scalar(ScalarType::Int64))),
        );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("age", FieldType::scalar(ScalarType::Int32))),
        );

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::C);
        assert!(!grade.can_run_online());
    }

    #[test]
    fn test_grade_d_remove_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let user_without_name = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)));

        let to = SchemaBundle::new(2).with_entity(user_without_name);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::D);
        assert!(!grade.can_run_online());
        assert!(grade.requires_data_migration());
    }

    #[test]
    fn test_grade_d_remove_entity() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());
        let to = SchemaBundle::new(2);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::D);
        assert!(!grade.blocking_changes.is_empty());
    }

    #[test]
    fn test_grade_d_required_without_default() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_required = create_user_entity();
        user_with_required.fields.push(FieldDef::new(
            "email",
            FieldType::scalar(ScalarType::String),
        ));

        let to = SchemaBundle::new(2).with_entity(user_with_required);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::D);
    }

    #[test]
    fn test_grade_add_constraint() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let constraint = ConstraintDef::unique("user_name_unique", "User", "name");

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_constraint(constraint);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::B);
        assert!(grade.requires_backfill());
    }

    #[test]
    fn test_grade_add_relation() {
        let user = create_user_entity();
        let post = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new(
                "author_id",
                FieldType::scalar(ScalarType::Uuid),
            ));

        let from = SchemaBundle::new(1).with_entity(user.clone()).with_entity(post.clone());

        let relation = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");

        let to = SchemaBundle::new(2)
            .with_entity(user)
            .with_entity(post)
            .with_relation(relation);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::A);
    }

    #[test]
    fn test_warnings_generated() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        // Remove User entity (destructive)
        let to = SchemaBundle::new(2);

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        assert_eq!(grade.overall_grade, SafetyGrade::D);
    }

    #[test]
    fn test_enum_variant_addition() {
        let from = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new(
                    "status",
                    FieldType::Enum {
                        name: "Status".into(),
                        variants: vec!["active".into(), "inactive".into()],
                    },
                )),
        );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new(
                    "status",
                    FieldType::Enum {
                        name: "Status".into(),
                        variants: vec!["active".into(), "inactive".into(), "pending".into()],
                    },
                )),
        );

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        // Adding enum variants is safe
        assert_eq!(grade.overall_grade, SafetyGrade::A);
    }

    #[test]
    fn test_enum_variant_removal() {
        let from = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new(
                    "status",
                    FieldType::Enum {
                        name: "Status".into(),
                        variants: vec!["active".into(), "inactive".into(), "pending".into()],
                    },
                )),
        );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new(
                    "status",
                    FieldType::Enum {
                        name: "Status".into(),
                        variants: vec!["active".into(), "inactive".into()],
                    },
                )),
        );

        let diff = SchemaDiff::compute(&from, &to);
        let grade = SafetyGrader::grade(&diff);

        // Removing enum variants is destructive
        assert_eq!(grade.overall_grade, SafetyGrade::D);
    }
}
