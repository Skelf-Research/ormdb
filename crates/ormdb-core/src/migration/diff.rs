//! Schema diffing algorithm.
//!
//! Compares two SchemaBundle versions and produces a structured diff
//! of all changes between them.

use crate::catalog::{
    Cardinality, ConstraintDef, DeleteBehavior, EntityDef, FieldDef, FieldType, LifecycleRules,
    RelationDef, SchemaBundle,
};
use std::collections::{HashMap, HashSet};

/// Complete diff between two schema bundles.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    /// Source schema version.
    pub from_version: u64,
    /// Target schema version.
    pub to_version: u64,
    /// Changes to entities.
    pub entity_changes: Vec<EntityChange>,
    /// Changes to relations.
    pub relation_changes: Vec<RelationChange>,
    /// Changes to constraints.
    pub constraint_changes: Vec<ConstraintChange>,
}

impl SchemaDiff {
    /// Compute the diff between two schema bundles.
    pub fn compute(from: &SchemaBundle, to: &SchemaBundle) -> Self {
        let entity_changes = Self::diff_entities(&from.entities, &to.entities);
        let relation_changes = Self::diff_relations(&from.relations, &to.relations);
        let constraint_changes = Self::diff_constraints(&from.constraints, &to.constraints);

        SchemaDiff {
            from_version: from.version,
            to_version: to.version,
            entity_changes,
            relation_changes,
            constraint_changes,
        }
    }

    /// Check if there are any changes.
    pub fn is_empty(&self) -> bool {
        self.entity_changes.is_empty()
            && self.relation_changes.is_empty()
            && self.constraint_changes.is_empty()
    }

    /// Get the total number of changes.
    pub fn change_count(&self) -> usize {
        self.entity_changes.len() + self.relation_changes.len() + self.constraint_changes.len()
    }

    fn diff_entities(
        from: &HashMap<String, EntityDef>,
        to: &HashMap<String, EntityDef>,
    ) -> Vec<EntityChange> {
        let mut changes = Vec::new();

        let from_names: HashSet<_> = from.keys().collect();
        let to_names: HashSet<_> = to.keys().collect();

        // Added entities
        for name in to_names.difference(&from_names) {
            changes.push(EntityChange::Added(to[*name].clone()));
        }

        // Removed entities
        for name in from_names.difference(&to_names) {
            changes.push(EntityChange::Removed(from[*name].clone()));
        }

        // Modified entities
        for name in from_names.intersection(&to_names) {
            let from_entity = &from[*name];
            let to_entity = &to[*name];

            if from_entity != to_entity {
                let field_changes = Self::diff_fields(&from_entity.fields, &to_entity.fields);
                let identity_changed =
                    if from_entity.identity_field != to_entity.identity_field {
                        Some(IdentityChange {
                            from_field: from_entity.identity_field.clone(),
                            to_field: to_entity.identity_field.clone(),
                        })
                    } else {
                        None
                    };
                let lifecycle_changed =
                    Self::diff_lifecycle(&from_entity.lifecycle, &to_entity.lifecycle);

                if !field_changes.is_empty()
                    || identity_changed.is_some()
                    || lifecycle_changed.is_some()
                {
                    changes.push(EntityChange::Modified {
                        entity_name: (*name).to_string(),
                        field_changes,
                        identity_changed,
                        lifecycle_changed,
                    });
                }
            }
        }

        changes
    }

    fn diff_fields(from: &[FieldDef], to: &[FieldDef]) -> Vec<FieldChange> {
        let mut changes = Vec::new();

        let from_map: HashMap<_, _> = from.iter().map(|f| (&f.name, f)).collect();
        let to_map: HashMap<_, _> = to.iter().map(|f| (&f.name, f)).collect();

        let from_names: HashSet<_> = from_map.keys().collect();
        let to_names: HashSet<_> = to_map.keys().collect();

        // Added fields
        for name in to_names.difference(&from_names) {
            changes.push(FieldChange::Added(to_map[*name].clone()));
        }

        // Removed fields
        for name in from_names.difference(&to_names) {
            changes.push(FieldChange::Removed(from_map[*name].clone()));
        }

        // Modified fields
        for name in from_names.intersection(&to_names) {
            let from_field = from_map[*name];
            let to_field = to_map[*name];

            // Type change
            if from_field.field_type != to_field.field_type {
                changes.push(FieldChange::TypeChanged {
                    field_name: (*name).to_string(),
                    from_type: from_field.field_type.clone(),
                    to_type: to_field.field_type.clone(),
                });
            }

            // Required change
            if from_field.required != to_field.required {
                changes.push(FieldChange::RequiredChanged {
                    field_name: (*name).to_string(),
                    from_required: from_field.required,
                    to_required: to_field.required,
                    has_default: to_field.default.is_some(),
                });
            }

            // Default change
            if from_field.default != to_field.default {
                changes.push(FieldChange::DefaultChanged {
                    field_name: (*name).to_string(),
                    from_default: from_field.default.clone(),
                    to_default: to_field.default.clone(),
                });
            }

            // Index change
            if from_field.indexed != to_field.indexed {
                changes.push(FieldChange::IndexChanged {
                    field_name: (*name).to_string(),
                    from_indexed: from_field.indexed,
                    to_indexed: to_field.indexed,
                });
            }

            // Computed change
            if from_field.computed != to_field.computed {
                changes.push(FieldChange::ComputedChanged {
                    field_name: (*name).to_string(),
                    from_computed: from_field.computed.clone(),
                    to_computed: to_field.computed.clone(),
                });
            }
        }

        changes
    }

    fn diff_lifecycle(from: &LifecycleRules, to: &LifecycleRules) -> Option<LifecycleChange> {
        let soft_delete_changed = if from.soft_delete != to.soft_delete {
            Some((from.soft_delete, to.soft_delete))
        } else {
            None
        };

        let default_order_changed = from.default_order != to.default_order;

        if soft_delete_changed.is_some() || default_order_changed {
            Some(LifecycleChange {
                soft_delete_changed,
                default_order_changed,
            })
        } else {
            None
        }
    }

    fn diff_relations(
        from: &HashMap<String, RelationDef>,
        to: &HashMap<String, RelationDef>,
    ) -> Vec<RelationChange> {
        let mut changes = Vec::new();

        let from_names: HashSet<_> = from.keys().collect();
        let to_names: HashSet<_> = to.keys().collect();

        // Added relations
        for name in to_names.difference(&from_names) {
            changes.push(RelationChange::Added(to[*name].clone()));
        }

        // Removed relations
        for name in from_names.difference(&to_names) {
            changes.push(RelationChange::Removed(from[*name].clone()));
        }

        // Modified relations
        for name in from_names.intersection(&to_names) {
            let from_rel = &from[*name];
            let to_rel = &to[*name];

            if from_rel != to_rel {
                let cardinality_changed = if from_rel.cardinality != to_rel.cardinality {
                    Some((from_rel.cardinality.clone(), to_rel.cardinality.clone()))
                } else {
                    None
                };

                let delete_behavior_changed = if from_rel.on_delete != to_rel.on_delete {
                    Some((from_rel.on_delete.clone(), to_rel.on_delete.clone()))
                } else {
                    None
                };

                let from_field_changed = if from_rel.from_field != to_rel.from_field {
                    Some((from_rel.from_field.clone(), to_rel.from_field.clone()))
                } else {
                    None
                };

                let to_field_changed = if from_rel.to_field != to_rel.to_field {
                    Some((from_rel.to_field.clone(), to_rel.to_field.clone()))
                } else {
                    None
                };

                let fields_changed = if from_field_changed.is_some() || to_field_changed.is_some() {
                    Some(RelationFieldChange {
                        from_field_changed,
                        to_field_changed,
                    })
                } else {
                    None
                };

                let entities_changed =
                    if from_rel.from_entity != to_rel.from_entity
                        || from_rel.to_entity != to_rel.to_entity
                    {
                        Some(RelationEntityChange {
                            from_entity_changed: if from_rel.from_entity != to_rel.from_entity {
                                Some((from_rel.from_entity.clone(), to_rel.from_entity.clone()))
                            } else {
                                None
                            },
                            to_entity_changed: if from_rel.to_entity != to_rel.to_entity {
                                Some((from_rel.to_entity.clone(), to_rel.to_entity.clone()))
                            } else {
                                None
                            },
                        })
                    } else {
                        None
                    };

                if cardinality_changed.is_some()
                    || delete_behavior_changed.is_some()
                    || fields_changed.is_some()
                    || entities_changed.is_some()
                {
                    changes.push(RelationChange::Modified {
                        relation_name: (*name).to_string(),
                        cardinality_changed,
                        delete_behavior_changed,
                        fields_changed,
                        entities_changed,
                    });
                }
            }
        }

        changes
    }

    fn diff_constraints(from: &[ConstraintDef], to: &[ConstraintDef]) -> Vec<ConstraintChange> {
        let mut changes = Vec::new();

        let from_map: HashMap<_, _> = from.iter().map(|c| (c.name(), c)).collect();
        let to_map: HashMap<_, _> = to.iter().map(|c| (c.name(), c)).collect();

        let from_names: HashSet<_> = from_map.keys().cloned().collect();
        let to_names: HashSet<_> = to_map.keys().cloned().collect();

        // Added constraints
        for name in to_names.difference(&from_names) {
            changes.push(ConstraintChange::Added(to_map[name].clone()));
        }

        // Removed constraints
        for name in from_names.difference(&to_names) {
            changes.push(ConstraintChange::Removed(from_map[name].clone()));
        }

        // Modified constraints
        for name in from_names.intersection(&to_names) {
            let from_constraint = from_map[name];
            let to_constraint = to_map[name];

            if from_constraint != to_constraint {
                changes.push(ConstraintChange::Modified {
                    constraint_name: (*name).to_string(),
                    from: from_constraint.clone(),
                    to: to_constraint.clone(),
                });
            }
        }

        changes
    }
}

/// Change to an entity definition.
#[derive(Debug, Clone)]
pub enum EntityChange {
    /// Entity was added.
    Added(EntityDef),
    /// Entity was removed.
    Removed(EntityDef),
    /// Entity was modified.
    Modified {
        /// Name of the entity.
        entity_name: String,
        /// Changes to fields.
        field_changes: Vec<FieldChange>,
        /// Change to identity field (if any).
        identity_changed: Option<IdentityChange>,
        /// Change to lifecycle rules (if any).
        lifecycle_changed: Option<LifecycleChange>,
    },
}

impl EntityChange {
    /// Get the entity name for this change.
    pub fn entity_name(&self) -> &str {
        match self {
            EntityChange::Added(e) => &e.name,
            EntityChange::Removed(e) => &e.name,
            EntityChange::Modified { entity_name, .. } => entity_name,
        }
    }
}

/// Change to a field within an entity.
#[derive(Debug, Clone)]
pub enum FieldChange {
    /// Field was added.
    Added(FieldDef),
    /// Field was removed.
    Removed(FieldDef),
    /// Field type was changed.
    TypeChanged {
        /// Name of the field.
        field_name: String,
        /// Original type.
        from_type: FieldType,
        /// New type.
        to_type: FieldType,
    },
    /// Field required status was changed.
    RequiredChanged {
        /// Name of the field.
        field_name: String,
        /// Was required before.
        from_required: bool,
        /// Is required now.
        to_required: bool,
        /// Whether the field has a default value.
        has_default: bool,
    },
    /// Field default value was changed.
    DefaultChanged {
        /// Name of the field.
        field_name: String,
        /// Original default value.
        from_default: Option<crate::catalog::DefaultValue>,
        /// New default value.
        to_default: Option<crate::catalog::DefaultValue>,
    },
    /// Field index status was changed.
    IndexChanged {
        /// Name of the field.
        field_name: String,
        /// Was indexed before.
        from_indexed: bool,
        /// Is indexed now.
        to_indexed: bool,
    },
    /// Field computed status was changed.
    ComputedChanged {
        /// Name of the field.
        field_name: String,
        /// Original computed definition.
        from_computed: Option<crate::catalog::ComputedField>,
        /// New computed definition.
        to_computed: Option<crate::catalog::ComputedField>,
    },
}

impl FieldChange {
    /// Get the field name for this change.
    pub fn field_name(&self) -> &str {
        match self {
            FieldChange::Added(f) => &f.name,
            FieldChange::Removed(f) => &f.name,
            FieldChange::TypeChanged { field_name, .. } => field_name,
            FieldChange::RequiredChanged { field_name, .. } => field_name,
            FieldChange::DefaultChanged { field_name, .. } => field_name,
            FieldChange::IndexChanged { field_name, .. } => field_name,
            FieldChange::ComputedChanged { field_name, .. } => field_name,
        }
    }
}

/// Change to identity field.
#[derive(Debug, Clone)]
pub struct IdentityChange {
    /// Original identity field.
    pub from_field: String,
    /// New identity field.
    pub to_field: String,
}

/// Change to lifecycle rules.
#[derive(Debug, Clone)]
pub struct LifecycleChange {
    /// Soft delete change (old, new).
    pub soft_delete_changed: Option<(bool, bool)>,
    /// Whether default order changed.
    pub default_order_changed: bool,
}

/// Change to a relation definition.
#[derive(Debug, Clone)]
pub enum RelationChange {
    /// Relation was added.
    Added(RelationDef),
    /// Relation was removed.
    Removed(RelationDef),
    /// Relation was modified.
    Modified {
        /// Name of the relation.
        relation_name: String,
        /// Cardinality change (if any).
        cardinality_changed: Option<(Cardinality, Cardinality)>,
        /// Delete behavior change (if any).
        delete_behavior_changed: Option<(DeleteBehavior, DeleteBehavior)>,
        /// Field changes (if any).
        fields_changed: Option<RelationFieldChange>,
        /// Entity changes (if any).
        entities_changed: Option<RelationEntityChange>,
    },
}

impl RelationChange {
    /// Get the relation name for this change.
    pub fn relation_name(&self) -> &str {
        match self {
            RelationChange::Added(r) => &r.name,
            RelationChange::Removed(r) => &r.name,
            RelationChange::Modified { relation_name, .. } => relation_name,
        }
    }
}

/// Change to relation fields.
#[derive(Debug, Clone)]
pub struct RelationFieldChange {
    /// From field change (old, new).
    pub from_field_changed: Option<(String, String)>,
    /// To field change (old, new).
    pub to_field_changed: Option<(String, String)>,
}

/// Change to relation entities.
#[derive(Debug, Clone)]
pub struct RelationEntityChange {
    /// From entity change (old, new).
    pub from_entity_changed: Option<(String, String)>,
    /// To entity change (old, new).
    pub to_entity_changed: Option<(String, String)>,
}

/// Change to a constraint definition.
#[derive(Debug, Clone)]
pub enum ConstraintChange {
    /// Constraint was added.
    Added(ConstraintDef),
    /// Constraint was removed.
    Removed(ConstraintDef),
    /// Constraint was modified.
    Modified {
        /// Name of the constraint.
        constraint_name: String,
        /// Original constraint.
        from: ConstraintDef,
        /// New constraint.
        to: ConstraintDef,
    },
}

impl ConstraintChange {
    /// Get the constraint name for this change.
    pub fn constraint_name(&self) -> &str {
        match self {
            ConstraintChange::Added(c) => c.name(),
            ConstraintChange::Removed(c) => c.name(),
            ConstraintChange::Modified {
                constraint_name, ..
            } => constraint_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{DefaultValue, FieldType, ScalarType};

    fn create_user_entity() -> EntityDef {
        EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
            .with_field(FieldDef::new(
                "email",
                FieldType::scalar(ScalarType::String),
            ))
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
    fn test_diff_add_entity() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.from_version, 1);
        assert_eq!(diff.to_version, 2);
        assert_eq!(diff.entity_changes.len(), 1);

        match &diff.entity_changes[0] {
            EntityChange::Added(e) => assert_eq!(e.name, "Post"),
            _ => panic!("Expected Added entity"),
        }
    }

    #[test]
    fn test_diff_remove_entity() {
        let from = SchemaBundle::new(1)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let to = SchemaBundle::new(2).with_entity(create_user_entity());

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.entity_changes.len(), 1);

        match &diff.entity_changes[0] {
            EntityChange::Removed(e) => assert_eq!(e.name, "Post"),
            _ => panic!("Expected Removed entity"),
        }
    }

    #[test]
    fn test_diff_add_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let mut user_with_age = create_user_entity();
        user_with_age.fields.push(FieldDef::optional(
            "age",
            FieldType::scalar(ScalarType::Int32),
        ));

        let to = SchemaBundle::new(2).with_entity(user_with_age);

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.entity_changes.len(), 1);

        match &diff.entity_changes[0] {
            EntityChange::Modified {
                entity_name,
                field_changes,
                ..
            } => {
                assert_eq!(entity_name, "User");
                assert_eq!(field_changes.len(), 1);
                match &field_changes[0] {
                    FieldChange::Added(f) => assert_eq!(f.name, "age"),
                    _ => panic!("Expected Added field"),
                }
            }
            _ => panic!("Expected Modified entity"),
        }
    }

    #[test]
    fn test_diff_remove_field() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let user_without_email = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)));

        let to = SchemaBundle::new(2).with_entity(user_without_email);

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.entity_changes.len(), 1);

        match &diff.entity_changes[0] {
            EntityChange::Modified { field_changes, .. } => {
                assert_eq!(field_changes.len(), 1);
                match &field_changes[0] {
                    FieldChange::Removed(f) => assert_eq!(f.name, "email"),
                    _ => panic!("Expected Removed field"),
                }
            }
            _ => panic!("Expected Modified entity"),
        }
    }

    #[test]
    fn test_diff_change_field_type() {
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

        assert_eq!(diff.entity_changes.len(), 1);

        match &diff.entity_changes[0] {
            EntityChange::Modified { field_changes, .. } => {
                assert_eq!(field_changes.len(), 1);
                match &field_changes[0] {
                    FieldChange::TypeChanged {
                        field_name,
                        from_type,
                        to_type,
                    } => {
                        assert_eq!(field_name, "age");
                        assert_eq!(*from_type, FieldType::scalar(ScalarType::Int32));
                        assert_eq!(*to_type, FieldType::scalar(ScalarType::Int64));
                    }
                    _ => panic!("Expected TypeChanged"),
                }
            }
            _ => panic!("Expected Modified entity"),
        }
    }

    #[test]
    fn test_diff_change_required() {
        let from = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(FieldDef::optional(
                    "email",
                    FieldType::scalar(ScalarType::String),
                )),
        );

        let to = SchemaBundle::new(2).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
                .with_field(
                    FieldDef::new("email", FieldType::scalar(ScalarType::String))
                        .with_default(DefaultValue::String("default@example.com".into())),
                ),
        );

        let diff = SchemaDiff::compute(&from, &to);

        let field_changes = match &diff.entity_changes[0] {
            EntityChange::Modified { field_changes, .. } => field_changes,
            _ => panic!("Expected Modified entity"),
        };

        let required_change = field_changes
            .iter()
            .find(|fc| matches!(fc, FieldChange::RequiredChanged { .. }));
        assert!(required_change.is_some());

        if let Some(FieldChange::RequiredChanged {
            from_required,
            to_required,
            has_default,
            ..
        }) = required_change
        {
            assert!(!from_required);
            assert!(to_required);
            assert!(has_default);
        }
    }

    #[test]
    fn test_diff_add_relation() {
        let from = SchemaBundle::new(1)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity());

        let relation = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_entity(create_post_entity())
            .with_relation(relation);

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.relation_changes.len(), 1);

        match &diff.relation_changes[0] {
            RelationChange::Added(r) => assert_eq!(r.name, "user_posts"),
            _ => panic!("Expected Added relation"),
        }
    }

    #[test]
    fn test_diff_add_constraint() {
        let from = SchemaBundle::new(1).with_entity(create_user_entity());

        let unique = ConstraintDef::unique("user_email_unique", "User", "email");

        let to = SchemaBundle::new(2)
            .with_entity(create_user_entity())
            .with_constraint(unique);

        let diff = SchemaDiff::compute(&from, &to);

        assert_eq!(diff.constraint_changes.len(), 1);

        match &diff.constraint_changes[0] {
            ConstraintChange::Added(c) => assert_eq!(c.name(), "user_email_unique"),
            _ => panic!("Expected Added constraint"),
        }
    }

    #[test]
    fn test_diff_no_changes() {
        let schema = SchemaBundle::new(1).with_entity(create_user_entity());

        let diff = SchemaDiff::compute(&schema, &schema);

        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_diff_complex_migration() {
        // V1: User entity only
        let v1 = SchemaBundle::new(1).with_entity(create_user_entity());

        // V2: Add Post entity, add age field to User, add relation, add constraint
        let user_with_age = EntityDef::new("User", "id")
            .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
            .with_field(FieldDef::new(
                "email",
                FieldType::scalar(ScalarType::String),
            ))
            .with_field(FieldDef::optional(
                "age",
                FieldType::scalar(ScalarType::Int32),
            ));

        let relation = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");
        let unique = ConstraintDef::unique("user_email_unique", "User", "email");

        let v2 = SchemaBundle::new(2)
            .with_entity(user_with_age)
            .with_entity(create_post_entity())
            .with_relation(relation)
            .with_constraint(unique);

        let diff = SchemaDiff::compute(&v1, &v2);

        assert_eq!(diff.from_version, 1);
        assert_eq!(diff.to_version, 2);

        // Should have: 1 modified entity (User), 1 added entity (Post)
        assert_eq!(diff.entity_changes.len(), 2);
        // Should have: 1 added relation
        assert_eq!(diff.relation_changes.len(), 1);
        // Should have: 1 added constraint
        assert_eq!(diff.constraint_changes.len(), 1);
    }
}
