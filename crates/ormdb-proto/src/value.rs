//! Runtime value types for protocol messages.

use rkyv::{Archive, Deserialize, Serialize};

/// A runtime value that can be serialized over the wire.
///
/// This enum represents all possible values that can be passed in queries,
/// mutations, and results. It maps to the scalar types defined in the catalog.
///
/// Note: Arrays are typed (e.g., BoolArray, Int32Array) to avoid recursive
/// type issues with rkyv serialization.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub enum Value {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// 32-bit signed integer.
    Int32(i32),
    /// 64-bit signed integer.
    Int64(i64),
    /// 32-bit floating point.
    Float32(f32),
    /// 64-bit floating point.
    Float64(f64),
    /// UTF-8 string.
    String(String),
    /// Binary data.
    Bytes(Vec<u8>),
    /// Timestamp as microseconds since Unix epoch.
    Timestamp(i64),
    /// UUID as 16 bytes.
    Uuid([u8; 16]),
    /// Array of booleans.
    BoolArray(Vec<bool>),
    /// Array of 32-bit integers.
    Int32Array(Vec<i32>),
    /// Array of 64-bit integers.
    Int64Array(Vec<i64>),
    /// Array of 32-bit floats.
    Float32Array(Vec<f32>),
    /// Array of 64-bit floats.
    Float64Array(Vec<f64>),
    /// Array of strings.
    StringArray(Vec<String>),
    /// Array of UUIDs.
    UuidArray(Vec<[u8; 16]>),
}

impl Value {
    /// Check if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if this value is an array type.
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            Value::BoolArray(_)
                | Value::Int32Array(_)
                | Value::Int64Array(_)
                | Value::Float32Array(_)
                | Value::Float64Array(_)
                | Value::StringArray(_)
                | Value::UuidArray(_)
        )
    }

    /// Try to get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i32.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::Int32(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int64(i) => Some(*i),
            Value::Int32(i) => Some(*i as i64),
            _ => None,
        }
    }

    /// Try to get as f32.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Float32(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to get as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float64(f) => Some(*f),
            Value::Float32(f) => Some(*f as f64),
            _ => None,
        }
    }

    /// Try to get as string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as bytes reference.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Try to get as timestamp.
    pub fn as_timestamp(&self) -> Option<i64> {
        match self {
            Value::Timestamp(t) => Some(*t),
            _ => None,
        }
    }

    /// Try to get as UUID.
    pub fn as_uuid(&self) -> Option<&[u8; 16]> {
        match self {
            Value::Uuid(u) => Some(u),
            _ => None,
        }
    }
}

// Conversion implementations
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int32(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int64(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float32(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float64(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}

impl From<[u8; 16]> for Value {
    fn from(v: [u8; 16]) -> Self {
        Value::Uuid(v)
    }
}

impl From<Vec<bool>> for Value {
    fn from(v: Vec<bool>) -> Self {
        Value::BoolArray(v)
    }
}

impl From<Vec<i32>> for Value {
    fn from(v: Vec<i32>) -> Self {
        Value::Int32Array(v)
    }
}

impl From<Vec<i64>> for Value {
    fn from(v: Vec<i64>) -> Self {
        Value::Int64Array(v)
    }
}

impl From<Vec<String>> for Value {
    fn from(v: Vec<String>) -> Self {
        Value::StringArray(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_accessors() {
        assert!(Value::Null.is_null());
        assert!(!Value::Bool(true).is_null());

        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Int32(42).as_i32(), Some(42));
        assert_eq!(Value::Int64(100).as_i64(), Some(100));
        assert_eq!(Value::Int32(42).as_i64(), Some(42)); // Widening conversion

        assert_eq!(Value::String("hello".into()).as_str(), Some("hello"));
        assert_eq!(Value::Bytes(vec![1, 2, 3]).as_bytes(), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn test_value_conversions() {
        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));

        let v: Value = 42i32.into();
        assert_eq!(v, Value::Int32(42));

        let v: Value = "hello".into();
        assert_eq!(v, Value::String("hello".into()));

        let v: Value = None::<i32>.into();
        assert_eq!(v, Value::Null);

        let v: Value = Some(42i32).into();
        assert_eq!(v, Value::Int32(42));
    }

    #[test]
    fn test_array_values() {
        let v: Value = vec![1i32, 2, 3].into();
        assert!(v.is_array());
        assert_eq!(v, Value::Int32Array(vec![1, 2, 3]));

        let v: Value = vec!["a".to_string(), "b".to_string()].into();
        assert!(v.is_array());
    }

    #[test]
    fn test_value_serialization_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int32(-42),
            Value::Int64(i64::MAX),
            Value::Float32(3.14),
            Value::Float64(std::f64::consts::PI),
            Value::String("hello world".into()),
            Value::Bytes(vec![0, 1, 2, 255]),
            Value::Timestamp(1704067200_000_000), // 2024-01-01 00:00:00 UTC
            Value::Uuid([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            Value::Int32Array(vec![1, 2, 3]),
            Value::StringArray(vec!["a".into(), "b".into()]),
        ];

        for value in values {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();
            let archived = rkyv::access::<ArchivedValue, rkyv::rancor::Error>(&bytes).unwrap();
            let deserialized: Value =
                rkyv::deserialize::<Value, rkyv::rancor::Error>(archived).unwrap();
            assert_eq!(value, deserialized);
        }
    }
}
