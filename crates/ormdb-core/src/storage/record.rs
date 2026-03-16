//! Record type for stored values.

use crate::error::Error;
use rkyv::{Archive, Deserialize, Serialize};

/// A stored record with metadata.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
pub struct Record {
    /// Serialized entity data.
    pub data: Vec<u8>,

    /// Creation timestamp in microseconds since Unix epoch.
    pub created_at: u64,

    /// Whether this record is a tombstone (soft delete).
    pub deleted: bool,
}

impl Record {
    /// Create a new record with the current timestamp.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            created_at: super::key::current_timestamp(),
            deleted: false,
        }
    }

    /// Create a tombstone record for soft deletion.
    pub fn tombstone() -> Self {
        Self {
            data: Vec::new(),
            created_at: super::key::current_timestamp(),
            deleted: true,
        }
    }

    /// Create a record with a specific timestamp.
    pub fn with_timestamp(data: Vec<u8>, created_at: u64) -> Self {
        Self {
            data,
            created_at,
            deleted: false,
        }
    }

    /// Serialize the record to bytes using rkyv.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(|e| Error::Serialization(e.to_string()))
    }

    /// Deserialize a record from bytes using rkyv.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| Error::Deserialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_roundtrip() {
        let record = Record::new(vec![1, 2, 3, 4, 5]);
        let bytes = record.to_bytes().unwrap();
        let decoded = Record::from_bytes(&bytes).unwrap();

        assert_eq!(record.data, decoded.data);
        assert_eq!(record.deleted, decoded.deleted);
        assert_eq!(record.created_at, decoded.created_at);
    }

    #[test]
    fn test_tombstone() {
        let tombstone = Record::tombstone();
        assert!(tombstone.deleted);
        assert!(tombstone.data.is_empty());

        let bytes = tombstone.to_bytes().unwrap();
        let decoded = Record::from_bytes(&bytes).unwrap();
        assert!(decoded.deleted);
    }
}
