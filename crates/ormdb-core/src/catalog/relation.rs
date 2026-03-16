//! Relation definitions between entities.

use rkyv::{Archive, Deserialize, Serialize};

/// Cardinality of a relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum Cardinality {
    /// One-to-one relation (unique foreign key).
    OneToOne,
    /// One-to-many relation (foreign key on many side).
    OneToMany,
    /// Many-to-many relation (requires edge/join entity).
    ManyToMany,
}

/// Behavior when a referenced entity is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum DeleteBehavior {
    /// Delete related entities.
    Cascade,
    /// Prevent deletion if related entities exist.
    Restrict,
    /// Set foreign key to null.
    SetNull,
}

/// A relation definition between two entities.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct RelationDef {
    /// Relation name (unique within schema).
    pub name: String,
    /// Source entity name.
    pub from_entity: String,
    /// Target entity name.
    pub to_entity: String,
    /// Relation cardinality.
    pub cardinality: Cardinality,
    /// Field on the source entity (foreign key).
    pub from_field: String,
    /// Field on the target entity (usually identity).
    pub to_field: String,
    /// Delete behavior.
    pub on_delete: DeleteBehavior,
    /// Edge entity for many-to-many relations.
    pub edge_entity: Option<String>,
}

impl RelationDef {
    /// Create a one-to-one relation.
    pub fn one_to_one(
        name: impl Into<String>,
        from_entity: impl Into<String>,
        from_field: impl Into<String>,
        to_entity: impl Into<String>,
        to_field: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            from_entity: from_entity.into(),
            to_entity: to_entity.into(),
            cardinality: Cardinality::OneToOne,
            from_field: from_field.into(),
            to_field: to_field.into(),
            on_delete: DeleteBehavior::Restrict,
            edge_entity: None,
        }
    }

    /// Create a one-to-many relation.
    pub fn one_to_many(
        name: impl Into<String>,
        from_entity: impl Into<String>,
        from_field: impl Into<String>,
        to_entity: impl Into<String>,
        to_field: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            from_entity: from_entity.into(),
            to_entity: to_entity.into(),
            cardinality: Cardinality::OneToMany,
            from_field: from_field.into(),
            to_field: to_field.into(),
            on_delete: DeleteBehavior::Restrict,
            edge_entity: None,
        }
    }

    /// Create a many-to-many relation.
    pub fn many_to_many(
        name: impl Into<String>,
        from_entity: impl Into<String>,
        from_field: impl Into<String>,
        to_entity: impl Into<String>,
        to_field: impl Into<String>,
        edge_entity: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            from_entity: from_entity.into(),
            to_entity: to_entity.into(),
            cardinality: Cardinality::ManyToMany,
            from_field: from_field.into(),
            to_field: to_field.into(),
            on_delete: DeleteBehavior::Cascade,
            edge_entity: Some(edge_entity.into()),
        }
    }

    /// Set delete behavior.
    pub fn with_on_delete(mut self, on_delete: DeleteBehavior) -> Self {
        self.on_delete = on_delete;
        self
    }

    /// Check if this is a many-to-many relation.
    pub fn is_many_to_many(&self) -> bool {
        self.cardinality == Cardinality::ManyToMany
    }

    /// Get the inverse relation (swapping from/to).
    ///
    /// Note: For one-to-many relations, the inverse is conceptually many-to-one,
    /// but we keep the same cardinality enum value since it represents the same relationship.
    pub fn inverse(&self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            from_entity: self.to_entity.clone(),
            to_entity: self.from_entity.clone(),
            cardinality: self.cardinality,
            from_field: self.to_field.clone(),
            to_field: self.from_field.clone(),
            on_delete: self.on_delete,
            edge_entity: self.edge_entity.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_to_one_relation() {
        let rel = RelationDef::one_to_one("user_profile", "Profile", "user_id", "User", "id");

        assert_eq!(rel.cardinality, Cardinality::OneToOne);
        assert_eq!(rel.from_entity, "Profile");
        assert_eq!(rel.to_entity, "User");
        assert!(rel.edge_entity.is_none());
    }

    #[test]
    fn test_one_to_many_relation() {
        let rel = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id")
            .with_on_delete(DeleteBehavior::Cascade);

        assert_eq!(rel.cardinality, Cardinality::OneToMany);
        assert_eq!(rel.on_delete, DeleteBehavior::Cascade);
    }

    #[test]
    fn test_many_to_many_relation() {
        let rel =
            RelationDef::many_to_many("user_tags", "User", "id", "Tag", "id", "UserTagEdge");

        assert!(rel.is_many_to_many());
        assert_eq!(rel.edge_entity, Some("UserTagEdge".to_string()));
    }

    #[test]
    fn test_inverse_relation() {
        let rel = RelationDef::one_to_many("user_posts", "Post", "author_id", "User", "id");
        let inverse = rel.inverse("posts_user");

        assert_eq!(inverse.from_entity, "User");
        assert_eq!(inverse.to_entity, "Post");
    }
}
