//! Value codec for encoding/decoding entity data to/from bytes.
//!
//! This module provides functions to serialize entity fields to bytes for storage
//! and deserialize them back to `Value` types for query results.

use crate::error::Error;
use ormdb_proto::Value;

/// Type tag for encoded values.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueTag {
    Null = 0,
    Bool = 1,
    Int32 = 2,
    Int64 = 3,
    Float32 = 4,
    Float64 = 5,
    String = 6,
    Bytes = 7,
    Uuid = 8,
    Timestamp = 9,
    BoolArray = 10,
    Int32Array = 11,
    Int64Array = 12,
    Float32Array = 13,
    Float64Array = 14,
    StringArray = 15,
    UuidArray = 16,
}

impl TryFrom<u8> for ValueTag {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ValueTag::Null),
            1 => Ok(ValueTag::Bool),
            2 => Ok(ValueTag::Int32),
            3 => Ok(ValueTag::Int64),
            4 => Ok(ValueTag::Float32),
            5 => Ok(ValueTag::Float64),
            6 => Ok(ValueTag::String),
            7 => Ok(ValueTag::Bytes),
            8 => Ok(ValueTag::Uuid),
            9 => Ok(ValueTag::Timestamp),
            10 => Ok(ValueTag::BoolArray),
            11 => Ok(ValueTag::Int32Array),
            12 => Ok(ValueTag::Int64Array),
            13 => Ok(ValueTag::Float32Array),
            14 => Ok(ValueTag::Float64Array),
            15 => Ok(ValueTag::StringArray),
            16 => Ok(ValueTag::UuidArray),
            _ => Err(Error::InvalidData(format!("Unknown value tag: {}", value))),
        }
    }
}

/// Encode a list of field name/value pairs to bytes.
///
/// Format:
/// - Field count (4 bytes, little-endian)
/// - For each field:
///   - Field name length (2 bytes, little-endian)
///   - Field name (UTF-8 bytes)
///   - Value tag (1 byte)
///   - Value data (variable length, depends on type)
pub fn encode_entity(fields: &[(String, Value)]) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();

    // Write field count
    let count = fields.len() as u32;
    buf.extend_from_slice(&count.to_le_bytes());

    for (name, value) in fields {
        // Write field name
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(Error::InvalidData("Field name too long".into()));
        }
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        // Write value
        encode_value(&mut buf, value)?;
    }

    Ok(buf)
}

/// Decode bytes back to field name/value pairs.
pub fn decode_entity(data: &[u8]) -> Result<Vec<(String, Value)>, Error> {
    let mut cursor = 0;

    // Read field count
    if data.len() < 4 {
        return Err(Error::InvalidData("Data too short for field count".into()));
    }
    let count = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4;

    let mut fields = Vec::with_capacity(count);

    for _ in 0..count {
        // Read field name
        if cursor + 2 > data.len() {
            return Err(Error::InvalidData("Data too short for field name length".into()));
        }
        let name_len = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;

        if cursor + name_len > data.len() {
            return Err(Error::InvalidData("Data too short for field name".into()));
        }
        let name = String::from_utf8(data[cursor..cursor + name_len].to_vec())
            .map_err(|_| Error::InvalidData("Invalid UTF-8 in field name".into()))?;
        cursor += name_len;

        // Read value
        let (value, bytes_read) = decode_value(&data[cursor..])?;
        cursor += bytes_read;

        fields.push((name, value));
    }

    Ok(fields)
}

/// Decode only specific fields from entity data (for projections).
///
/// This is more efficient than decoding all fields when only a subset is needed.
pub fn decode_fields(data: &[u8], field_names: &[String]) -> Result<Vec<(String, Value)>, Error> {
    let all_fields = decode_entity(data)?;

    // Filter to requested fields
    let mut result = Vec::with_capacity(field_names.len());
    for name in field_names {
        if let Some((_, value)) = all_fields.iter().find(|(n, _)| n == name) {
            result.push((name.clone(), value.clone()));
        }
    }

    Ok(result)
}

/// Get a single field value by name.
///
/// Uses skip_value() to avoid decoding values for non-matching fields.
pub fn get_field(data: &[u8], field_name: &str) -> Result<Option<Value>, Error> {
    let mut cursor = 0;

    // Read field count
    if data.len() < 4 {
        return Err(Error::InvalidData("Data too short for field count".into()));
    }
    let count = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4;

    for _ in 0..count {
        // Read field name
        if cursor + 2 > data.len() {
            return Err(Error::InvalidData("Data too short for field name length".into()));
        }
        let name_len = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;

        if cursor + name_len > data.len() {
            return Err(Error::InvalidData("Data too short for field name".into()));
        }
        let name = std::str::from_utf8(&data[cursor..cursor + name_len])
            .map_err(|_| Error::InvalidData("Invalid UTF-8 in field name".into()))?;
        cursor += name_len;

        if name == field_name {
            // Decode and return the matching field
            let (value, _) = decode_value(&data[cursor..])?;
            return Ok(Some(value));
        } else {
            // Skip this value without decoding
            let skip_bytes = skip_value(&data[cursor..])?;
            cursor += skip_bytes;
        }
    }

    Ok(None)
}

/// Get multiple fields by name in a single pass.
///
/// More efficient than calling get_field() multiple times because it only
/// makes one pass through the data and uses skip_value() for non-matching fields.
pub fn get_fields_fast(data: &[u8], field_names: &[&str]) -> Result<Vec<(String, Value)>, Error> {
    use std::collections::HashSet;

    let name_set: HashSet<&str> = field_names.iter().copied().collect();
    let mut cursor = 0;
    let mut result = Vec::with_capacity(field_names.len());

    // Read field count
    if data.len() < 4 {
        return Err(Error::InvalidData("Data too short for field count".into()));
    }
    let count = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4;

    for _ in 0..count {
        // Read field name
        if cursor + 2 > data.len() {
            return Err(Error::InvalidData("Data too short for field name length".into()));
        }
        let name_len = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;

        if cursor + name_len > data.len() {
            return Err(Error::InvalidData("Data too short for field name".into()));
        }
        let name = std::str::from_utf8(&data[cursor..cursor + name_len])
            .map_err(|_| Error::InvalidData("Invalid UTF-8 in field name".into()))?;
        cursor += name_len;

        if name_set.contains(name) {
            // Decode the matching field
            let (value, bytes_read) = decode_value(&data[cursor..])?;
            cursor += bytes_read;
            result.push((name.to_string(), value));

            // Early exit if we found all requested fields
            if result.len() == field_names.len() {
                break;
            }
        } else {
            // Skip this value without decoding
            let skip_bytes = skip_value(&data[cursor..])?;
            cursor += skip_bytes;
        }
    }

    Ok(result)
}

/// Skip a value without decoding it.
///
/// Returns the number of bytes to skip (including the tag byte).
/// This is much faster than decode_value() when you don't need the value.
pub fn skip_value(data: &[u8]) -> Result<usize, Error> {
    if data.is_empty() {
        return Err(Error::InvalidData("Empty data for value".into()));
    }

    let tag = ValueTag::try_from(data[0])?;

    let size = match tag {
        ValueTag::Null => 1,
        ValueTag::Bool => 2, // tag + 1 byte
        ValueTag::Int32 | ValueTag::Float32 => 5, // tag + 4 bytes
        ValueTag::Int64 | ValueTag::Float64 | ValueTag::Timestamp => 9, // tag + 8 bytes
        ValueTag::Uuid => 17, // tag + 16 bytes
        ValueTag::String | ValueTag::Bytes => {
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for string/bytes length".into()));
            }
            let len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            5 + len // tag + 4 byte length + content
        }
        ValueTag::BoolArray => {
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for bool array length".into()));
            }
            let len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            5 + len // tag + 4 byte length + len bools
        }
        ValueTag::Int32Array | ValueTag::Float32Array => {
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for i32/f32 array length".into()));
            }
            let len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            5 + len * 4 // tag + 4 byte length + len * 4 bytes
        }
        ValueTag::Int64Array | ValueTag::Float64Array => {
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for i64/f64 array length".into()));
            }
            let len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            5 + len * 8 // tag + 4 byte length + len * 8 bytes
        }
        ValueTag::UuidArray => {
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for uuid array length".into()));
            }
            let len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            5 + len * 16 // tag + 4 byte length + len * 16 bytes
        }
        ValueTag::StringArray => {
            // Need to iterate through strings to compute size
            if data.len() < 5 {
                return Err(Error::InvalidData("Data too short for string array length".into()));
            }
            let array_len = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
            let mut cursor = 5;
            for _ in 0..array_len {
                if cursor + 4 > data.len() {
                    return Err(Error::InvalidData("Data too short for string length in array".into()));
                }
                let str_len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4 + str_len;
            }
            cursor
        }
    };

    Ok(size)
}

/// Encode a single value to the buffer.
fn encode_value(buf: &mut Vec<u8>, value: &Value) -> Result<(), Error> {
    match value {
        Value::Null => {
            buf.push(ValueTag::Null as u8);
        }
        Value::Bool(b) => {
            buf.push(ValueTag::Bool as u8);
            buf.push(if *b { 1 } else { 0 });
        }
        Value::Int32(n) => {
            buf.push(ValueTag::Int32 as u8);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Int64(n) => {
            buf.push(ValueTag::Int64 as u8);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::Float32(f) => {
            buf.push(ValueTag::Float32 as u8);
            buf.extend_from_slice(&f.to_le_bytes());
        }
        Value::Float64(f) => {
            buf.push(ValueTag::Float64 as u8);
            buf.extend_from_slice(&f.to_le_bytes());
        }
        Value::String(s) => {
            buf.push(ValueTag::String as u8);
            let bytes = s.as_bytes();
            if bytes.len() > u32::MAX as usize {
                return Err(Error::InvalidData("String too long".into()));
            }
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::Bytes(b) => {
            buf.push(ValueTag::Bytes as u8);
            if b.len() > u32::MAX as usize {
                return Err(Error::InvalidData("Bytes too long".into()));
            }
            buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
            buf.extend_from_slice(b);
        }
        Value::Uuid(uuid) => {
            buf.push(ValueTag::Uuid as u8);
            buf.extend_from_slice(uuid);
        }
        Value::Timestamp(ts) => {
            buf.push(ValueTag::Timestamp as u8);
            buf.extend_from_slice(&ts.to_le_bytes());
        }
        Value::BoolArray(arr) => {
            buf.push(ValueTag::BoolArray as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for b in arr {
                buf.push(if *b { 1 } else { 0 });
            }
        }
        Value::Int32Array(arr) => {
            buf.push(ValueTag::Int32Array as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for n in arr {
                buf.extend_from_slice(&n.to_le_bytes());
            }
        }
        Value::Int64Array(arr) => {
            buf.push(ValueTag::Int64Array as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for n in arr {
                buf.extend_from_slice(&n.to_le_bytes());
            }
        }
        Value::Float32Array(arr) => {
            buf.push(ValueTag::Float32Array as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for f in arr {
                buf.extend_from_slice(&f.to_le_bytes());
            }
        }
        Value::Float64Array(arr) => {
            buf.push(ValueTag::Float64Array as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for f in arr {
                buf.extend_from_slice(&f.to_le_bytes());
            }
        }
        Value::StringArray(arr) => {
            buf.push(ValueTag::StringArray as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for s in arr {
                let bytes = s.as_bytes();
                if bytes.len() > u32::MAX as usize {
                    return Err(Error::InvalidData("String in array too long".into()));
                }
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                buf.extend_from_slice(bytes);
            }
        }
        Value::UuidArray(arr) => {
            buf.push(ValueTag::UuidArray as u8);
            buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for uuid in arr {
                buf.extend_from_slice(uuid);
            }
        }
    }
    Ok(())
}

/// Decode a single value from the buffer.
/// Returns the value and the number of bytes consumed.
fn decode_value(data: &[u8]) -> Result<(Value, usize), Error> {
    if data.is_empty() {
        return Err(Error::InvalidData("Empty data for value".into()));
    }

    let tag = ValueTag::try_from(data[0])?;
    let mut cursor = 1;

    let value = match tag {
        ValueTag::Null => Value::Null,
        ValueTag::Bool => {
            if cursor >= data.len() {
                return Err(Error::InvalidData("Data too short for bool".into()));
            }
            let v = data[cursor] != 0;
            cursor += 1;
            Value::Bool(v)
        }
        ValueTag::Int32 => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for i32".into()));
            }
            let v = i32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            Value::Int32(v)
        }
        ValueTag::Int64 => {
            if cursor + 8 > data.len() {
                return Err(Error::InvalidData("Data too short for i64".into()));
            }
            let v = i64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            Value::Int64(v)
        }
        ValueTag::Float32 => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for f32".into()));
            }
            let v = f32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            Value::Float32(v)
        }
        ValueTag::Float64 => {
            if cursor + 8 > data.len() {
                return Err(Error::InvalidData("Data too short for f64".into()));
            }
            let v = f64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            Value::Float64(v)
        }
        ValueTag::String => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for string length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len > data.len() {
                return Err(Error::InvalidData("Data too short for string".into()));
            }
            let v = String::from_utf8(data[cursor..cursor + len].to_vec())
                .map_err(|_| Error::InvalidData("Invalid UTF-8 in string".into()))?;
            cursor += len;
            Value::String(v)
        }
        ValueTag::Bytes => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for bytes length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len > data.len() {
                return Err(Error::InvalidData("Data too short for bytes".into()));
            }
            let v = data[cursor..cursor + len].to_vec();
            cursor += len;
            Value::Bytes(v)
        }
        ValueTag::Uuid => {
            if cursor + 16 > data.len() {
                return Err(Error::InvalidData("Data too short for uuid".into()));
            }
            let mut uuid = [0u8; 16];
            uuid.copy_from_slice(&data[cursor..cursor + 16]);
            cursor += 16;
            Value::Uuid(uuid)
        }
        ValueTag::Timestamp => {
            if cursor + 8 > data.len() {
                return Err(Error::InvalidData("Data too short for timestamp".into()));
            }
            let v = i64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            Value::Timestamp(v)
        }
        ValueTag::BoolArray => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for bool array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len > data.len() {
                return Err(Error::InvalidData("Data too short for bool array".into()));
            }
            let arr: Vec<bool> = data[cursor..cursor + len].iter().map(|&b| b != 0).collect();
            cursor += len;
            Value::BoolArray(arr)
        }
        ValueTag::Int32Array => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for i32 array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len * 4 > data.len() {
                return Err(Error::InvalidData("Data too short for i32 array".into()));
            }
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(i32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()));
                cursor += 4;
            }
            Value::Int32Array(arr)
        }
        ValueTag::Int64Array => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for i64 array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len * 8 > data.len() {
                return Err(Error::InvalidData("Data too short for i64 array".into()));
            }
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(i64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap()));
                cursor += 8;
            }
            Value::Int64Array(arr)
        }
        ValueTag::Float32Array => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for f32 array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len * 4 > data.len() {
                return Err(Error::InvalidData("Data too short for f32 array".into()));
            }
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(f32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()));
                cursor += 4;
            }
            Value::Float32Array(arr)
        }
        ValueTag::Float64Array => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for f64 array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len * 8 > data.len() {
                return Err(Error::InvalidData("Data too short for f64 array".into()));
            }
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(f64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap()));
                cursor += 8;
            }
            Value::Float64Array(arr)
        }
        ValueTag::StringArray => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for string array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                if cursor + 4 > data.len() {
                    return Err(Error::InvalidData("Data too short for string length in array".into()));
                }
                let str_len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4;
                if cursor + str_len > data.len() {
                    return Err(Error::InvalidData("Data too short for string in array".into()));
                }
                let s = String::from_utf8(data[cursor..cursor + str_len].to_vec())
                    .map_err(|_| Error::InvalidData("Invalid UTF-8 in string array".into()))?;
                cursor += str_len;
                arr.push(s);
            }
            Value::StringArray(arr)
        }
        ValueTag::UuidArray => {
            if cursor + 4 > data.len() {
                return Err(Error::InvalidData("Data too short for uuid array length".into()));
            }
            let len = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + len * 16 > data.len() {
                return Err(Error::InvalidData("Data too short for uuid array".into()));
            }
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                let mut uuid = [0u8; 16];
                uuid.copy_from_slice(&data[cursor..cursor + 16]);
                cursor += 16;
                arr.push(uuid);
            }
            Value::UuidArray(arr)
        }
    };

    Ok((value, cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_basic_types() {
        let fields = vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(), Value::Int32(30)),
            ("active".to_string(), Value::Bool(true)),
            ("score".to_string(), Value::Float64(95.5)),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_encode_decode_null() {
        let fields = vec![
            ("value".to_string(), Value::Null),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_encode_decode_uuid() {
        let uuid = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let fields = vec![
            ("id".to_string(), Value::Uuid(uuid)),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_encode_decode_timestamp() {
        let fields = vec![
            ("created_at".to_string(), Value::Timestamp(1234567890123)),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_encode_decode_bytes() {
        let fields = vec![
            ("data".to_string(), Value::Bytes(vec![0, 1, 2, 255])),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_encode_decode_arrays() {
        let fields = vec![
            ("flags".to_string(), Value::BoolArray(vec![true, false, true])),
            ("numbers".to_string(), Value::Int32Array(vec![1, 2, 3])),
            ("big_numbers".to_string(), Value::Int64Array(vec![100, 200, 300])),
            ("floats".to_string(), Value::Float32Array(vec![1.0, 2.5, 3.14])),
            ("doubles".to_string(), Value::Float64Array(vec![1.0, 2.5, 3.14159])),
            ("tags".to_string(), Value::StringArray(vec!["a".to_string(), "b".to_string()])),
            ("ids".to_string(), Value::UuidArray(vec![[1u8; 16], [2u8; 16]])),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_decode_fields_subset() {
        let fields = vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(), Value::Int32(30)),
            ("email".to_string(), Value::String("alice@example.com".to_string())),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let subset = decode_fields(&encoded, &["name".to_string(), "email".to_string()]).unwrap();

        assert_eq!(subset.len(), 2);
        assert_eq!(subset[0], ("name".to_string(), Value::String("Alice".to_string())));
        assert_eq!(subset[1], ("email".to_string(), Value::String("alice@example.com".to_string())));
    }

    #[test]
    fn test_get_field() {
        let fields = vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(), Value::Int32(30)),
        ];

        let encoded = encode_entity(&fields).unwrap();

        assert_eq!(
            get_field(&encoded, "name").unwrap(),
            Some(Value::String("Alice".to_string()))
        );
        assert_eq!(
            get_field(&encoded, "age").unwrap(),
            Some(Value::Int32(30))
        );
        assert_eq!(
            get_field(&encoded, "nonexistent").unwrap(),
            None
        );
    }

    #[test]
    fn test_empty_entity() {
        let fields: Vec<(String, Value)> = vec![];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert!(decoded.is_empty());
    }

    #[test]
    fn test_empty_string() {
        let fields = vec![
            ("empty".to_string(), Value::String("".to_string())),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }

    #[test]
    fn test_empty_arrays() {
        let fields = vec![
            ("empty_strs".to_string(), Value::StringArray(vec![])),
            ("empty_ints".to_string(), Value::Int32Array(vec![])),
        ];

        let encoded = encode_entity(&fields).unwrap();
        let decoded = decode_entity(&encoded).unwrap();

        assert_eq!(fields, decoded);
    }
}
