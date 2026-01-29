//! Versioned key encoding for MVCC.

use std::fmt;

/// Size of entity ID in bytes (UUID).
pub const ENTITY_ID_SIZE: usize = 16;

/// Size of version timestamp in bytes.
pub const VERSION_TS_SIZE: usize = 8;

/// Total key size.
pub const KEY_SIZE: usize = ENTITY_ID_SIZE + VERSION_TS_SIZE;

/// A versioned key combining entity ID and version timestamp.
///
/// Key format: `[entity_id (16 bytes)][version_ts (8 bytes, big-endian)]`
///
/// Big-endian encoding ensures lexicographic ordering matches numeric ordering,
/// so range scans return versions in chronological order.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VersionedKey {
    /// Entity identifier (UUID bytes).
    pub entity_id: [u8; ENTITY_ID_SIZE],

    /// Version timestamp in microseconds since Unix epoch.
    pub version_ts: u64,
}

impl VersionedKey {
    /// Create a new versioned key.
    pub fn new(entity_id: [u8; ENTITY_ID_SIZE], version_ts: u64) -> Self {
        Self {
            entity_id,
            version_ts,
        }
    }

    /// Create a key with the current timestamp.
    pub fn now(entity_id: [u8; ENTITY_ID_SIZE]) -> Self {
        Self::new(entity_id, current_timestamp())
    }

    /// Encode the key to bytes.
    pub fn encode(&self) -> [u8; KEY_SIZE] {
        let mut buf = [0u8; KEY_SIZE];
        buf[..ENTITY_ID_SIZE].copy_from_slice(&self.entity_id);
        buf[ENTITY_ID_SIZE..].copy_from_slice(&self.version_ts.to_be_bytes());
        buf
    }

    /// Decode a key from bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != KEY_SIZE {
            return None;
        }

        let mut entity_id = [0u8; ENTITY_ID_SIZE];
        entity_id.copy_from_slice(&bytes[..ENTITY_ID_SIZE]);

        let mut ts_bytes = [0u8; VERSION_TS_SIZE];
        ts_bytes.copy_from_slice(&bytes[ENTITY_ID_SIZE..]);
        let version_ts = u64::from_be_bytes(ts_bytes);

        Some(Self {
            entity_id,
            version_ts,
        })
    }

    /// Get the prefix for scanning all versions of an entity.
    pub fn entity_prefix(entity_id: &[u8; ENTITY_ID_SIZE]) -> [u8; ENTITY_ID_SIZE] {
        *entity_id
    }

    /// Create the minimum key for an entity (version 0).
    pub fn min_for_entity(entity_id: [u8; ENTITY_ID_SIZE]) -> Self {
        Self::new(entity_id, 0)
    }

    /// Create the maximum key for an entity (max version).
    pub fn max_for_entity(entity_id: [u8; ENTITY_ID_SIZE]) -> Self {
        Self::new(entity_id, u64::MAX)
    }
}

impl fmt::Debug for VersionedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format entity_id as hex for readability
        let hex: String = self
            .entity_id
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        f.debug_struct("VersionedKey")
            .field("entity_id", &hex)
            .field("version_ts", &self.version_ts)
            .finish()
    }
}

/// Get current timestamp in microseconds since Unix epoch.
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let entity_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let version_ts = 1234567890123456u64;

        let key = VersionedKey::new(entity_id, version_ts);
        let encoded = key.encode();
        let decoded = VersionedKey::decode(&encoded).unwrap();

        assert_eq!(key, decoded);
    }

    #[test]
    fn test_lexicographic_ordering() {
        let entity_id = [0u8; 16];

        let key1 = VersionedKey::new(entity_id, 100);
        let key2 = VersionedKey::new(entity_id, 200);
        let key3 = VersionedKey::new(entity_id, 300);

        let enc1 = key1.encode();
        let enc2 = key2.encode();
        let enc3 = key3.encode();

        // Lexicographic ordering should match numeric ordering
        assert!(enc1 < enc2);
        assert!(enc2 < enc3);
    }

    #[test]
    fn test_decode_invalid_length() {
        let short = [0u8; 10];
        assert!(VersionedKey::decode(&short).is_none());

        let long = [0u8; 30];
        assert!(VersionedKey::decode(&long).is_none());
    }
}
