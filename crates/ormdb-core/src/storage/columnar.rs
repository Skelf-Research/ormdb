//! Columnar storage projections for analytical queries.
//!
//! This module provides columnar storage that runs alongside the row store,
//! optimized for aggregate queries and column scans.

use std::sync::atomic::{AtomicU32, Ordering};
use std::collections::HashMap;

use sled::{Db, Tree};
use tracing::debug;

use crate::error::Error;
use ormdb_proto::Value;

/// Columnar projection store for efficient column-oriented access.
pub struct ColumnarStore {
    /// The underlying sled database.
    db: Db,

    /// Dictionary for string compression.
    string_dict: StringDictionary,
}

impl ColumnarStore {
    /// Open or create a columnar store.
    pub fn open(db: &Db) -> Result<Self, Error> {
        let string_dict = StringDictionary::open(db)?;

        Ok(Self {
            db: db.clone(),
            string_dict,
        })
    }

    /// Get the columnar projection for a specific entity type.
    pub fn projection(&self, entity_type: &str) -> Result<ColumnarProjection, Error> {
        ColumnarProjection::open(&self.db, entity_type, &self.string_dict)
    }

    /// Get the string dictionary.
    pub fn string_dict(&self) -> &StringDictionary {
        &self.string_dict
    }
}

/// Columnar projection for a single entity type.
///
/// Key format: `[column_name_len:1][column_name][entity_id:16]`
/// Value format: encoded column value
pub struct ColumnarProjection<'a> {
    tree: Tree,
    entity_type: String,
    string_dict: &'a StringDictionary,
}

impl<'a> ColumnarProjection<'a> {
    /// Open or create a columnar projection for an entity type.
    pub fn open(db: &Db, entity_type: &str, string_dict: &'a StringDictionary) -> Result<Self, Error> {
        let tree_name = format!("columnar:{}", entity_type);
        let tree = db.open_tree(tree_name)?;

        Ok(Self {
            tree,
            entity_type: entity_type.to_string(),
            string_dict,
        })
    }

    /// Update a row in the columnar store.
    ///
    /// This extracts each field and stores it in columnar format.
    pub fn update_row(&self, entity_id: &[u8; 16], fields: &[(String, Value)]) -> Result<(), Error> {
        for (field_name, value) in fields {
            let key = self.column_key(field_name, entity_id);
            let encoded = self.encode_value(value)?;
            self.tree.insert(key, encoded)?;
        }

        debug!(
            entity_type = %self.entity_type,
            entity_id = ?entity_id,
            field_count = fields.len(),
            "Updated columnar row"
        );

        Ok(())
    }

    /// Delete a row from the columnar store.
    pub fn delete_row(&self, entity_id: &[u8; 16], column_names: &[&str]) -> Result<(), Error> {
        for column_name in column_names {
            let key = self.column_key(column_name, entity_id);
            self.tree.remove(key)?;
        }
        Ok(())
    }

    /// Scan a single column and return all values.
    pub fn scan_column(&self, column_name: &str) -> impl Iterator<Item = Result<([u8; 16], Value), Error>> + '_ {
        let prefix = self.column_prefix(column_name);
        let prefix_len = prefix.len();

        self.tree.scan_prefix(&prefix).map(move |result| {
            let (key, value) = result?;

            // Extract entity_id from key
            if key.len() != prefix_len + 16 {
                return Err(Error::InvalidKey);
            }
            let mut entity_id = [0u8; 16];
            entity_id.copy_from_slice(&key[prefix_len..]);

            // Decode value
            let decoded = self.decode_value(&value)?;

            Ok((entity_id, decoded))
        })
    }

    /// Scan multiple columns for all entities.
    ///
    /// Returns a map from entity_id to field values.
    pub fn scan_columns(&self, column_names: &[&str]) -> Result<HashMap<[u8; 16], HashMap<String, Value>>, Error> {
        let mut result: HashMap<[u8; 16], HashMap<String, Value>> = HashMap::new();

        for &column_name in column_names {
            for item in self.scan_column(column_name) {
                let (entity_id, value) = item?;
                result
                    .entry(entity_id)
                    .or_default()
                    .insert(column_name.to_string(), value);
            }
        }

        Ok(result)
    }

    /// Get a specific column value for an entity.
    pub fn get_column(&self, entity_id: &[u8; 16], column_name: &str) -> Result<Option<Value>, Error> {
        let key = self.column_key(column_name, entity_id);
        match self.tree.get(key)? {
            Some(bytes) => {
                let value = self.decode_value(&bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Count non-null values in a column.
    pub fn count_column(&self, column_name: &str) -> Result<u64, Error> {
        let prefix = self.column_prefix(column_name);
        let mut count = 0u64;

        for result in self.tree.scan_prefix(&prefix) {
            let (_, value) = result?;
            // Skip null values
            if !value.is_empty() && value[0] != ValueTag::Null as u8 {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Sum values in a numeric column.
    pub fn sum_column(&self, column_name: &str) -> Result<f64, Error> {
        let mut sum = 0.0f64;

        for item in self.scan_column(column_name) {
            let (_, value) = item?;
            sum += value_to_f64(&value);
        }

        Ok(sum)
    }

    /// Get minimum value in a column.
    pub fn min_column(&self, column_name: &str) -> Result<Option<Value>, Error> {
        let mut min: Option<Value> = None;

        for item in self.scan_column(column_name) {
            let (_, value) = item?;
            if matches!(value, Value::Null) {
                continue;
            }

            min = Some(match min {
                None => value,
                Some(current) => {
                    if compare_values(&value, &current).is_lt() {
                        value
                    } else {
                        current
                    }
                }
            });
        }

        Ok(min)
    }

    /// Get maximum value in a column.
    pub fn max_column(&self, column_name: &str) -> Result<Option<Value>, Error> {
        let mut max: Option<Value> = None;

        for item in self.scan_column(column_name) {
            let (_, value) = item?;
            if matches!(value, Value::Null) {
                continue;
            }

            max = Some(match max {
                None => value,
                Some(current) => {
                    if compare_values(&value, &current).is_gt() {
                        value
                    } else {
                        current
                    }
                }
            });
        }

        Ok(max)
    }

    /// Build column key: [column_name_len][column_name][entity_id]
    fn column_key(&self, column_name: &str, entity_id: &[u8; 16]) -> Vec<u8> {
        let name_bytes = column_name.as_bytes();
        let mut key = Vec::with_capacity(1 + name_bytes.len() + 16);
        key.push(name_bytes.len() as u8);
        key.extend_from_slice(name_bytes);
        key.extend_from_slice(entity_id);
        key
    }

    /// Build column prefix for scanning: [column_name_len][column_name]
    fn column_prefix(&self, column_name: &str) -> Vec<u8> {
        let name_bytes = column_name.as_bytes();
        let mut prefix = Vec::with_capacity(1 + name_bytes.len());
        prefix.push(name_bytes.len() as u8);
        prefix.extend_from_slice(name_bytes);
        prefix
    }

    /// Encode a value for columnar storage.
    fn encode_value(&self, value: &Value) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::new();

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
            Value::Float32(n) => {
                buf.push(ValueTag::Float32 as u8);
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::Float64(n) => {
                buf.push(ValueTag::Float64 as u8);
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Value::String(s) => {
                // Use dictionary encoding for strings
                let dict_id = self.string_dict.get_or_insert(s)?;
                buf.push(ValueTag::StringDict as u8);
                buf.extend_from_slice(&dict_id.to_le_bytes());
            }
            Value::Bytes(b) => {
                buf.push(ValueTag::Bytes as u8);
                buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
                buf.extend_from_slice(b);
            }
            Value::Uuid(id) => {
                buf.push(ValueTag::Uuid as u8);
                buf.extend_from_slice(id);
            }
            Value::Timestamp(ts) => {
                buf.push(ValueTag::Timestamp as u8);
                buf.extend_from_slice(&ts.to_le_bytes());
            }
            // Typed arrays - encode length followed by elements
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
                for n in arr {
                    buf.extend_from_slice(&n.to_le_bytes());
                }
            }
            Value::Float64Array(arr) => {
                buf.push(ValueTag::Float64Array as u8);
                buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
                for n in arr {
                    buf.extend_from_slice(&n.to_le_bytes());
                }
            }
            Value::StringArray(arr) => {
                buf.push(ValueTag::StringArray as u8);
                buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
                for s in arr {
                    let dict_id = self.string_dict.get_or_insert(s)?;
                    buf.extend_from_slice(&dict_id.to_le_bytes());
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

        Ok(buf)
    }

    /// Decode a value from columnar storage.
    fn decode_value(&self, bytes: &[u8]) -> Result<Value, Error> {
        if bytes.is_empty() {
            return Err(Error::InvalidData("empty value".to_string()));
        }

        let tag = bytes[0];
        let data = &bytes[1..];

        match tag {
            t if t == ValueTag::Null as u8 => Ok(Value::Null),
            t if t == ValueTag::Bool as u8 => {
                if data.is_empty() {
                    return Err(Error::InvalidData("missing bool data".to_string()));
                }
                Ok(Value::Bool(data[0] != 0))
            }
            t if t == ValueTag::Int32 as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing int32 data".to_string()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&data[..4]);
                Ok(Value::Int32(i32::from_le_bytes(buf)))
            }
            t if t == ValueTag::Int64 as u8 => {
                if data.len() < 8 {
                    return Err(Error::InvalidData("missing int64 data".to_string()));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data[..8]);
                Ok(Value::Int64(i64::from_le_bytes(buf)))
            }
            t if t == ValueTag::Float32 as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing float32 data".to_string()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&data[..4]);
                Ok(Value::Float32(f32::from_le_bytes(buf)))
            }
            t if t == ValueTag::Float64 as u8 => {
                if data.len() < 8 {
                    return Err(Error::InvalidData("missing float64 data".to_string()));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data[..8]);
                Ok(Value::Float64(f64::from_le_bytes(buf)))
            }
            t if t == ValueTag::StringDict as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing string dict id".to_string()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&data[..4]);
                let dict_id = u32::from_le_bytes(buf);
                let s = self.string_dict.lookup(dict_id)?
                    .ok_or_else(|| Error::InvalidData(format!("unknown dict id: {}", dict_id)))?;
                Ok(Value::String(s))
            }
            t if t == ValueTag::Bytes as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing bytes length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                if data.len() < 4 + len {
                    return Err(Error::InvalidData("truncated bytes data".to_string()));
                }
                Ok(Value::Bytes(data[4..4 + len].to_vec()))
            }
            t if t == ValueTag::Uuid as u8 => {
                if data.len() < 16 {
                    return Err(Error::InvalidData("missing uuid data".to_string()));
                }
                let mut id = [0u8; 16];
                id.copy_from_slice(&data[..16]);
                Ok(Value::Uuid(id))
            }
            t if t == ValueTag::Timestamp as u8 => {
                if data.len() < 8 {
                    return Err(Error::InvalidData("missing timestamp data".to_string()));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data[..8]);
                Ok(Value::Timestamp(i64::from_le_bytes(buf)))
            }
            t if t == ValueTag::BoolArray as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    if data.len() <= 4 + i {
                        return Err(Error::InvalidData("truncated bool array".to_string()));
                    }
                    arr.push(data[4 + i] != 0);
                }
                Ok(Value::BoolArray(arr))
            }
            t if t == ValueTag::Int32Array as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 4;
                    if data.len() < offset + 4 {
                        return Err(Error::InvalidData("truncated int32 array".to_string()));
                    }
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&data[offset..offset + 4]);
                    arr.push(i32::from_le_bytes(buf));
                }
                Ok(Value::Int32Array(arr))
            }
            t if t == ValueTag::Int64Array as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 8;
                    if data.len() < offset + 8 {
                        return Err(Error::InvalidData("truncated int64 array".to_string()));
                    }
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&data[offset..offset + 8]);
                    arr.push(i64::from_le_bytes(buf));
                }
                Ok(Value::Int64Array(arr))
            }
            t if t == ValueTag::Float32Array as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 4;
                    if data.len() < offset + 4 {
                        return Err(Error::InvalidData("truncated float32 array".to_string()));
                    }
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&data[offset..offset + 4]);
                    arr.push(f32::from_le_bytes(buf));
                }
                Ok(Value::Float32Array(arr))
            }
            t if t == ValueTag::Float64Array as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 8;
                    if data.len() < offset + 8 {
                        return Err(Error::InvalidData("truncated float64 array".to_string()));
                    }
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&data[offset..offset + 8]);
                    arr.push(f64::from_le_bytes(buf));
                }
                Ok(Value::Float64Array(arr))
            }
            t if t == ValueTag::StringArray as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 4;
                    if data.len() < offset + 4 {
                        return Err(Error::InvalidData("truncated string array".to_string()));
                    }
                    let mut id_buf = [0u8; 4];
                    id_buf.copy_from_slice(&data[offset..offset + 4]);
                    let dict_id = u32::from_le_bytes(id_buf);
                    let s = self.string_dict.lookup(dict_id)?
                        .ok_or_else(|| Error::InvalidData(format!("unknown dict id: {}", dict_id)))?;
                    arr.push(s);
                }
                Ok(Value::StringArray(arr))
            }
            t if t == ValueTag::UuidArray as u8 => {
                if data.len() < 4 {
                    return Err(Error::InvalidData("missing array length".to_string()));
                }
                let mut len_buf = [0u8; 4];
                len_buf.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = 4 + i * 16;
                    if data.len() < offset + 16 {
                        return Err(Error::InvalidData("truncated uuid array".to_string()));
                    }
                    let mut uuid = [0u8; 16];
                    uuid.copy_from_slice(&data[offset..offset + 16]);
                    arr.push(uuid);
                }
                Ok(Value::UuidArray(arr))
            }
            _ => Err(Error::InvalidData(format!("unknown value tag: {}", tag))),
        }
    }
}

/// Value type tags for columnar encoding.
#[repr(u8)]
enum ValueTag {
    Null = 0,
    Bool = 1,
    Int32 = 2,
    Int64 = 3,
    Float32 = 4,
    Float64 = 5,
    StringDict = 6,  // Dictionary-encoded string
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

/// Dictionary for string compression.
///
/// Maps strings to u32 IDs for efficient storage of repeated string values.
pub struct StringDictionary {
    /// Forward mapping: string -> id
    forward_tree: Tree,
    /// Reverse mapping: id -> string
    reverse_tree: Tree,
    /// Next ID to assign
    next_id: AtomicU32,
}

impl StringDictionary {
    /// Open or create a string dictionary.
    pub fn open(db: &Db) -> Result<Self, Error> {
        let forward_tree = db.open_tree("dict:forward")?;
        let reverse_tree = db.open_tree("dict:reverse")?;

        // Find the highest existing ID
        let max_id = reverse_tree.last()?.map(|(key, _)| {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&key);
            u32::from_be_bytes(buf)
        }).unwrap_or(0);

        Ok(Self {
            forward_tree,
            reverse_tree,
            next_id: AtomicU32::new(max_id + 1),
        })
    }

    /// Get or insert a string, returning its dictionary ID.
    pub fn get_or_insert(&self, s: &str) -> Result<u32, Error> {
        let key = s.as_bytes();

        // Check if already exists
        if let Some(id_bytes) = self.forward_tree.get(key)? {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&id_bytes);
            return Ok(u32::from_le_bytes(buf));
        }

        // Assign new ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let id_bytes = id.to_le_bytes();

        // Insert forward mapping
        self.forward_tree.insert(key, &id_bytes)?;

        // Insert reverse mapping (use big-endian for sorted iteration)
        self.reverse_tree.insert(&id.to_be_bytes(), key)?;

        Ok(id)
    }

    /// Lookup a string by its dictionary ID.
    pub fn lookup(&self, id: u32) -> Result<Option<String>, Error> {
        let key = id.to_be_bytes();
        match self.reverse_tree.get(key)? {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())
                    .map_err(|e| Error::InvalidData(format!("invalid utf8: {}", e)))?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    /// Get the current size of the dictionary.
    pub fn len(&self) -> usize {
        self.forward_tree.len()
    }

    /// Check if the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Convert a Value to f64 for aggregation.
fn value_to_f64(value: &Value) -> f64 {
    match value {
        Value::Int32(n) => *n as f64,
        Value::Int64(n) => *n as f64,
        Value::Float32(n) => *n as f64,
        Value::Float64(n) => *n,
        _ => 0.0,
    }
}

/// Compare two Values for ordering.
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Int32(a), Value::Int32(b)) => a.cmp(b),
        (Value::Int64(a), Value::Int64(b)) => a.cmp(b),
        (Value::Float32(a), Value::Float32(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
        (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Timestamp(a), Value::Timestamp(b)) => a.cmp(b),
        // For mixed numeric types, convert to f64
        (a, b) => {
            let fa = value_to_f64(a);
            let fb = value_to_f64(b);
            fa.partial_cmp(&fb).unwrap_or(Ordering::Equal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::open(dir.path()).unwrap();
        (dir, db)
    }

    #[test]
    fn test_string_dictionary() {
        let (_dir, db) = test_db();
        let dict = StringDictionary::open(&db).unwrap();

        // Insert strings
        let id1 = dict.get_or_insert("hello").unwrap();
        let id2 = dict.get_or_insert("world").unwrap();
        let id3 = dict.get_or_insert("hello").unwrap(); // Duplicate

        assert_ne!(id1, id2);
        assert_eq!(id1, id3); // Same string should get same ID

        // Lookup
        assert_eq!(dict.lookup(id1).unwrap(), Some("hello".to_string()));
        assert_eq!(dict.lookup(id2).unwrap(), Some("world".to_string()));
        assert_eq!(dict.lookup(999).unwrap(), None);
    }

    #[test]
    fn test_columnar_projection_basic() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("User").unwrap();

        let entity_id = [1u8; 16];
        let fields = vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(), Value::Int32(30)),
        ];

        // Update row
        proj.update_row(&entity_id, &fields).unwrap();

        // Get columns
        assert_eq!(
            proj.get_column(&entity_id, "name").unwrap(),
            Some(Value::String("Alice".to_string()))
        );
        assert_eq!(
            proj.get_column(&entity_id, "age").unwrap(),
            Some(Value::Int32(30))
        );
        assert_eq!(
            proj.get_column(&entity_id, "nonexistent").unwrap(),
            None
        );
    }

    #[test]
    fn test_columnar_aggregates() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("User").unwrap();

        // Insert multiple rows
        for i in 0..10 {
            let mut entity_id = [0u8; 16];
            entity_id[0] = i as u8;
            let fields = vec![
                ("name".to_string(), Value::String(format!("User{}", i))),
                ("age".to_string(), Value::Int32((20 + i) as i32)),
            ];
            proj.update_row(&entity_id, &fields).unwrap();
        }

        // Test count
        assert_eq!(proj.count_column("age").unwrap(), 10);

        // Test sum: 20 + 21 + ... + 29 = 245
        assert_eq!(proj.sum_column("age").unwrap(), 245.0);

        // Test min
        assert_eq!(proj.min_column("age").unwrap(), Some(Value::Int32(20)));

        // Test max
        assert_eq!(proj.max_column("age").unwrap(), Some(Value::Int32(29)));
    }

    #[test]
    fn test_columnar_scan() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("Item").unwrap();

        // Insert rows
        let id1 = [1u8; 16];
        let id2 = [2u8; 16];
        proj.update_row(&id1, &[("price".to_string(), Value::Float64(10.5))]).unwrap();
        proj.update_row(&id2, &[("price".to_string(), Value::Float64(20.0))]).unwrap();

        // Scan column
        let items: Vec<_> = proj.scan_column("price").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_columnar_delete() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("User").unwrap();

        let entity_id = [1u8; 16];
        let fields = vec![
            ("name".to_string(), Value::String("Bob".to_string())),
            ("age".to_string(), Value::Int32(25)),
        ];

        // Insert and verify
        proj.update_row(&entity_id, &fields).unwrap();
        assert!(proj.get_column(&entity_id, "name").unwrap().is_some());

        // Delete
        proj.delete_row(&entity_id, &["name", "age"]).unwrap();

        // Verify deletion
        assert!(proj.get_column(&entity_id, "name").unwrap().is_none());
        assert!(proj.get_column(&entity_id, "age").unwrap().is_none());
    }

    #[test]
    fn test_value_encoding_roundtrip() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("Test").unwrap();

        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            Value::Int32(-42),
            Value::Int64(1234567890),
            Value::Float32(3.14),
            Value::Float64(2.718281828),
            Value::String("test string".to_string()),
            Value::Bytes(vec![1, 2, 3, 4]),
            Value::Uuid([0xAB; 16]),
            Value::Timestamp(1234567890),
        ];

        for (i, value) in values.into_iter().enumerate() {
            let mut entity_id = [0u8; 16];
            entity_id[0] = i as u8;
            proj.update_row(&entity_id, &[("val".to_string(), value.clone())]).unwrap();

            let retrieved = proj.get_column(&entity_id, "val").unwrap().unwrap();
            assert_eq!(retrieved, value, "Roundtrip failed for value type {}", i);
        }
    }

    #[test]
    fn test_array_encoding_roundtrip() {
        let (_dir, db) = test_db();
        let store = ColumnarStore::open(&db).unwrap();
        let proj = store.projection("Test").unwrap();

        let entity_id = [1u8; 16];
        let values = vec![
            ("bools".to_string(), Value::BoolArray(vec![true, false, true])),
            ("ints".to_string(), Value::Int32Array(vec![1, 2, 3, 4])),
            ("longs".to_string(), Value::Int64Array(vec![100, 200, 300])),
            ("floats".to_string(), Value::Float32Array(vec![1.0, 2.5, 3.5])),
            ("doubles".to_string(), Value::Float64Array(vec![10.1, 20.2])),
            ("strings".to_string(), Value::StringArray(vec!["a".into(), "b".into(), "c".into()])),
        ];

        proj.update_row(&entity_id, &values).unwrap();

        for (name, expected) in &values {
            let retrieved = proj.get_column(&entity_id, name).unwrap().unwrap();
            assert_eq!(&retrieved, expected, "Roundtrip failed for {}", name);
        }
    }
}
