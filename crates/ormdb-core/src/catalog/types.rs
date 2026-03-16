//! Core type definitions for the catalog.

use rkyv::{Archive, Deserialize, Serialize};

/// Scalar data types supported by ORMDB.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum ScalarType {
    /// Boolean value.
    Bool,
    /// 32-bit signed integer.
    Int32,
    /// 64-bit signed integer.
    Int64,
    /// 32-bit floating point.
    Float32,
    /// 64-bit floating point.
    Float64,
    /// Fixed-precision decimal.
    Decimal {
        /// Total number of digits.
        precision: u8,
        /// Number of digits after decimal point.
        scale: u8,
    },
    /// UTF-8 string.
    String,
    /// Binary data.
    Bytes,
    /// Timestamp (microseconds since Unix epoch).
    Timestamp,
    /// UUID (128-bit identifier).
    Uuid,
}

/// Field types - flat representation without recursion.
///
/// Note: Nested optional/array types are not supported to avoid recursive type issues.
/// Use multiple fields or separate entities for complex nested structures.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum FieldType {
    /// A scalar value.
    Scalar(ScalarType),
    /// An optional scalar value (nullable).
    OptionalScalar(ScalarType),
    /// An array of scalar values.
    ArrayScalar(ScalarType),
    /// An enumeration type.
    Enum {
        /// Name of the enum type.
        name: String,
        /// Allowed variant values.
        variants: Vec<String>,
    },
    /// An optional enumeration.
    OptionalEnum {
        /// Name of the enum type.
        name: String,
        /// Allowed variant values.
        variants: Vec<String>,
    },
    /// An embedded entity (nested object).
    Embedded {
        /// Name of the embedded entity type.
        entity: String,
    },
    /// An optional embedded entity.
    OptionalEmbedded {
        /// Name of the embedded entity type.
        entity: String,
    },
    /// An array of embedded entities.
    ArrayEmbedded {
        /// Name of the embedded entity type.
        entity: String,
    },
}

impl ScalarType {
    /// Check if this type is numeric.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Float32
                | ScalarType::Float64
                | ScalarType::Decimal { .. }
        )
    }

    /// Check if this type is a string-like type.
    pub fn is_string_like(&self) -> bool {
        matches!(self, ScalarType::String | ScalarType::Bytes)
    }
}

impl FieldType {
    /// Create a scalar field type.
    pub fn scalar(scalar: ScalarType) -> Self {
        FieldType::Scalar(scalar)
    }

    /// Create an optional scalar field type.
    pub fn optional_scalar(scalar: ScalarType) -> Self {
        FieldType::OptionalScalar(scalar)
    }

    /// Create an array of scalars field type.
    pub fn array_scalar(scalar: ScalarType) -> Self {
        FieldType::ArrayScalar(scalar)
    }

    /// Create an enum field type.
    pub fn enum_type(name: impl Into<String>, variants: Vec<String>) -> Self {
        FieldType::Enum {
            name: name.into(),
            variants,
        }
    }

    /// Create an embedded entity field type.
    pub fn embedded(entity: impl Into<String>) -> Self {
        FieldType::Embedded {
            entity: entity.into(),
        }
    }

    /// Check if this type is nullable.
    pub fn is_nullable(&self) -> bool {
        matches!(
            self,
            FieldType::OptionalScalar(_)
                | FieldType::OptionalEnum { .. }
                | FieldType::OptionalEmbedded { .. }
        )
    }

    /// Check if this type is an array.
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            FieldType::ArrayScalar(_) | FieldType::ArrayEmbedded { .. }
        )
    }

    /// Get the inner scalar type if this is a scalar-based type.
    pub fn scalar_type(&self) -> Option<&ScalarType> {
        match self {
            FieldType::Scalar(s) | FieldType::OptionalScalar(s) | FieldType::ArrayScalar(s) => {
                Some(s)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_type_checks() {
        assert!(ScalarType::Int32.is_numeric());
        assert!(ScalarType::Float64.is_numeric());
        assert!(ScalarType::Decimal {
            precision: 10,
            scale: 2
        }
        .is_numeric());
        assert!(!ScalarType::String.is_numeric());
        assert!(!ScalarType::Bool.is_numeric());

        assert!(ScalarType::String.is_string_like());
        assert!(ScalarType::Bytes.is_string_like());
        assert!(!ScalarType::Int32.is_string_like());
    }

    #[test]
    fn test_field_type_builders() {
        let int_type = FieldType::scalar(ScalarType::Int32);
        assert!(!int_type.is_nullable());
        assert!(!int_type.is_array());

        let optional_int = FieldType::optional_scalar(ScalarType::Int32);
        assert!(optional_int.is_nullable());

        let int_array = FieldType::array_scalar(ScalarType::Int32);
        assert!(int_array.is_array());
        assert!(int_array.scalar_type().is_some());
    }

    #[test]
    fn test_enum_type() {
        let status = FieldType::enum_type("Status", vec!["Active".into(), "Inactive".into()]);
        assert!(!status.is_nullable());

        if let FieldType::Enum { name, variants } = status {
            assert_eq!(name, "Status");
            assert_eq!(variants.len(), 2);
        } else {
            panic!("Expected Enum");
        }
    }

    #[test]
    fn test_embedded_type() {
        let address = FieldType::embedded("Address");
        assert!(!address.is_nullable());

        if let FieldType::Embedded { entity } = address {
            assert_eq!(entity, "Address");
        } else {
            panic!("Expected Embedded");
        }
    }
}
