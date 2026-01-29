//! Field definitions for entities.

use super::types::FieldType;
use rkyv::{Archive, Deserialize, Serialize};

/// A field definition within an entity.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct FieldDef {
    /// Field name.
    pub name: String,
    /// Field data type.
    pub field_type: FieldType,
    /// Whether the field is required (non-nullable at the application level).
    pub required: bool,
    /// Default value if not provided.
    pub default: Option<DefaultValue>,
    /// Computed field definition if this is a derived field.
    pub computed: Option<ComputedField>,
    /// Whether this field should be indexed.
    pub indexed: bool,
}

/// Default value for a field.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum DefaultValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Floating point value.
    Float(f64),
    /// String value.
    String(String),
    /// Binary data.
    Bytes(Vec<u8>),
    /// Current timestamp (evaluated at insert time).
    CurrentTimestamp,
    /// Auto-generated UUID.
    AutoUuid,
    /// Custom expression (evaluated at insert time).
    Expression(String),
}

/// Computed field definition.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum ComputedField {
    /// Materialized: computed and stored on write.
    Materialized {
        /// Expression to compute the value.
        expression: String,
    },
    /// Virtual: computed on read, not stored.
    Virtual {
        /// Expression to compute the value.
        expression: String,
    },
}

impl FieldDef {
    /// Create a new required field.
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            required: true,
            default: None,
            computed: None,
            indexed: false,
        }
    }

    /// Create an optional field (required = false).
    pub fn optional(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            required: false,
            default: None,
            computed: None,
            indexed: false,
        }
    }

    /// Create an optional scalar field.
    pub fn optional_scalar(
        name: impl Into<String>,
        scalar: crate::catalog::ScalarType,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::OptionalScalar(scalar),
            required: false,
            default: None,
            computed: None,
            indexed: false,
        }
    }

    /// Set the default value.
    pub fn with_default(mut self, default: DefaultValue) -> Self {
        self.default = Some(default);
        self
    }

    /// Mark as indexed.
    pub fn with_index(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Set as a computed field.
    pub fn computed(mut self, computed: ComputedField) -> Self {
        self.computed = Some(computed);
        self
    }

    /// Check if this is a computed field.
    pub fn is_computed(&self) -> bool {
        self.computed.is_some()
    }

    /// Check if this field has a default value.
    pub fn has_default(&self) -> bool {
        self.default.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::types::ScalarType;

    #[test]
    fn test_field_def_builder() {
        let field = FieldDef::new("id", FieldType::scalar(ScalarType::Uuid))
            .with_default(DefaultValue::AutoUuid)
            .with_index();

        assert_eq!(field.name, "id");
        assert!(field.required);
        assert!(field.indexed);
        assert!(field.has_default());
    }

    #[test]
    fn test_optional_field() {
        let field = FieldDef::optional("description", FieldType::scalar(ScalarType::String));

        assert!(!field.required);
        assert!(!field.indexed);
        assert!(!field.has_default());
    }

    #[test]
    fn test_computed_field() {
        let field = FieldDef::new("full_name", FieldType::scalar(ScalarType::String)).computed(
            ComputedField::Virtual {
                expression: "first_name || ' ' || last_name".into(),
            },
        );

        assert!(field.is_computed());
    }
}
