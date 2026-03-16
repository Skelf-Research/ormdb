//! Constraint definitions for entities.

use rkyv::{Archive, Deserialize, Serialize};

/// A constraint definition.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum ConstraintDef {
    /// Uniqueness constraint (single or composite).
    Unique {
        /// Constraint name.
        name: String,
        /// Entity this constraint applies to.
        entity: String,
        /// Fields that must be unique together.
        fields: Vec<String>,
    },
    /// Foreign key constraint.
    ForeignKey {
        /// Constraint name.
        name: String,
        /// Entity containing the foreign key.
        entity: String,
        /// Foreign key field.
        field: String,
        /// Referenced entity.
        references_entity: String,
        /// Referenced field (usually identity).
        references_field: String,
    },
    /// Check constraint (expression must evaluate to true).
    Check {
        /// Constraint name.
        name: String,
        /// Entity this constraint applies to.
        entity: String,
        /// Boolean expression.
        expression: String,
    },
}

impl ConstraintDef {
    /// Create a unique constraint on a single field.
    pub fn unique(
        name: impl Into<String>,
        entity: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        ConstraintDef::Unique {
            name: name.into(),
            entity: entity.into(),
            fields: vec![field.into()],
        }
    }

    /// Create a composite unique constraint.
    pub fn unique_composite(
        name: impl Into<String>,
        entity: impl Into<String>,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        ConstraintDef::Unique {
            name: name.into(),
            entity: entity.into(),
            fields: fields.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a foreign key constraint.
    pub fn foreign_key(
        name: impl Into<String>,
        entity: impl Into<String>,
        field: impl Into<String>,
        references_entity: impl Into<String>,
        references_field: impl Into<String>,
    ) -> Self {
        ConstraintDef::ForeignKey {
            name: name.into(),
            entity: entity.into(),
            field: field.into(),
            references_entity: references_entity.into(),
            references_field: references_field.into(),
        }
    }

    /// Create a check constraint.
    pub fn check(
        name: impl Into<String>,
        entity: impl Into<String>,
        expression: impl Into<String>,
    ) -> Self {
        ConstraintDef::Check {
            name: name.into(),
            entity: entity.into(),
            expression: expression.into(),
        }
    }

    /// Get the constraint name.
    pub fn name(&self) -> &str {
        match self {
            ConstraintDef::Unique { name, .. } => name,
            ConstraintDef::ForeignKey { name, .. } => name,
            ConstraintDef::Check { name, .. } => name,
        }
    }

    /// Get the entity this constraint applies to.
    pub fn entity(&self) -> &str {
        match self {
            ConstraintDef::Unique { entity, .. } => entity,
            ConstraintDef::ForeignKey { entity, .. } => entity,
            ConstraintDef::Check { entity, .. } => entity,
        }
    }

    /// Check if this is a unique constraint.
    pub fn is_unique(&self) -> bool {
        matches!(self, ConstraintDef::Unique { .. })
    }

    /// Check if this is a foreign key constraint.
    pub fn is_foreign_key(&self) -> bool {
        matches!(self, ConstraintDef::ForeignKey { .. })
    }

    /// Check if this is a check constraint.
    pub fn is_check(&self) -> bool {
        matches!(self, ConstraintDef::Check { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_constraint() {
        let constraint = ConstraintDef::unique("user_email_unique", "User", "email");

        assert!(constraint.is_unique());
        assert_eq!(constraint.name(), "user_email_unique");
        assert_eq!(constraint.entity(), "User");
    }

    #[test]
    fn test_composite_unique() {
        let constraint = ConstraintDef::unique_composite(
            "user_org_unique",
            "UserOrg",
            ["user_id", "org_id"],
        );

        if let ConstraintDef::Unique { fields, .. } = constraint {
            assert_eq!(fields.len(), 2);
        } else {
            panic!("Expected unique constraint");
        }
    }

    #[test]
    fn test_foreign_key_constraint() {
        let constraint =
            ConstraintDef::foreign_key("post_author_fk", "Post", "author_id", "User", "id");

        assert!(constraint.is_foreign_key());
    }

    #[test]
    fn test_check_constraint() {
        let constraint = ConstraintDef::check("positive_amount", "Payment", "amount > 0");

        assert!(constraint.is_check());
        if let ConstraintDef::Check { expression, .. } = constraint {
            assert_eq!(expression, "amount > 0");
        }
    }
}
