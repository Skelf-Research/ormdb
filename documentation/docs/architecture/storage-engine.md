# Storage Engine

The storage engine manages data persistence using a hybrid row/columnar architecture with MVCC versioning.

---

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                       Storage Engine                             │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                     sled Database                        │    │
│  │  ┌───────────────┐  ┌───────────────┐  ┌─────────────┐  │    │
│  │  │   Data Tree   │  │   Meta Tree   │  │ Type Index  │  │    │
│  │  │  (versions)   │  │  (pointers)   │  │   Tree      │  │    │
│  │  └───────────────┘  └───────────────┘  └─────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Secondary Storage                      │    │
│  │  ┌───────────────┐  ┌───────────────┐  ┌─────────────┐  │    │
│  │  │   Columnar    │  │  Hash Index   │  │ B-tree Index│  │    │
│  │  │    Store      │  │  (equality)   │  │   (range)   │  │    │
│  │  └───────────────┘  └───────────────┘  └─────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Data Structures

### Versioned Key

Every entity record is keyed by entity ID + version timestamp:

```rust
pub struct VersionedKey {
    /// Entity's unique identifier (16-byte UUID)
    pub entity_id: [u8; 16],
    /// Version timestamp (Unix microseconds, big-endian)
    pub version_ts: u64,
}

impl VersionedKey {
    pub fn encode(&self) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[..16].copy_from_slice(&self.entity_id);
        bytes[16..].copy_from_slice(&self.version_ts.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 24 { return None; }
        let mut entity_id = [0u8; 16];
        entity_id.copy_from_slice(&bytes[..16]);
        let version_ts = u64::from_be_bytes(bytes[16..24].try_into().ok()?);
        Some(Self { entity_id, version_ts })
    }
}
```

Big-endian encoding ensures keys sort chronologically when scanned.

### Record

Entity data with deletion flag:

```rust
pub struct Record {
    /// True if this version is a tombstone
    pub deleted: bool,
    /// Serialized entity data
    pub data: Vec<u8>,
}

impl Record {
    pub fn new(data: Vec<u8>) -> Self {
        Self { deleted: false, data }
    }

    pub fn tombstone() -> Self {
        Self { deleted: true, data: Vec::new() }
    }
}
```

---

## Tree Organization

### Data Tree

Stores versioned entity records:

```
Tree: "data"
─────────────────────────────────────────────────────
Key: [entity_id:16][version_ts:8]
Value: Record { deleted, data }

Example entries:
[abc...][100] → Record { deleted: false, data: {...} }
[abc...][200] → Record { deleted: false, data: {...} }
[abc...][300] → Record { deleted: true, data: [] }  ← tombstone
[def...][150] → Record { deleted: false, data: {...} }
```

### Meta Tree

Stores latest version pointers for fast lookups:

```
Tree: "meta"
─────────────────────────────────────────────────────
Key: "latest:" + entity_id
Value: version_ts (8 bytes, big-endian)

Example entries:
latest:abc... → 300  (current version)
latest:def... → 150  (current version)
```

### Type Index Tree

Enables efficient entity type scans:

```
Tree: "index:entity_type"
─────────────────────────────────────────────────────
Key: entity_type + "\0" + entity_id
Value: (empty)

Example entries:
User\0abc... →
User\0def... →
Post\0ghi... →
```

---

## Read Operations

### Get Latest

```rust
pub fn get_latest(&self, entity_id: &[u8; 16]) -> Result<Option<(u64, Record)>> {
    // 1. Look up latest version from meta tree
    let latest_key = format!("latest:{}", hex::encode(entity_id));
    let version_ts = match self.meta_tree.get(&latest_key)? {
        Some(bytes) => u64::from_be_bytes(bytes[..8].try_into()?),
        None => return Ok(None),
    };

    // 2. Fetch record at that version
    let key = VersionedKey::new(*entity_id, version_ts);
    match self.data_tree.get(key.encode())? {
        Some(bytes) => {
            let record = Record::from_bytes(&bytes)?;
            if record.deleted {
                Ok(None)  // Tombstone = deleted
            } else {
                Ok(Some((version_ts, record)))
            }
        }
        None => Ok(None),
    }
}
```

### Get At Timestamp

Point-in-time query:

```rust
pub fn get_at(&self, entity_id: &[u8; 16], at_ts: u64) -> Result<Option<(u64, Record)>> {
    // Scan backwards from requested timestamp
    let max_key = VersionedKey::new(*entity_id, at_ts);
    let min_key = VersionedKey::min_for_entity(*entity_id);

    for result in self.data_tree.range(min_key.encode()..=max_key.encode()).rev() {
        let (key_bytes, value_bytes) = result?;
        let key = VersionedKey::decode(&key_bytes)?;

        if key.entity_id != *entity_id {
            continue;
        }

        let record = Record::from_bytes(&value_bytes)?;
        if !record.deleted {
            return Ok(Some((key.version_ts, record)));
        }
    }

    Ok(None)
}
```

### Batch Get

Optimized multi-entity fetch:

```rust
pub fn get_latest_batch(&self, entity_ids: &[[u8; 16]]) -> Result<Vec<Option<(u64, Record)>>> {
    // Phase 1: Batch fetch version timestamps
    let mut versions = Vec::with_capacity(entity_ids.len());
    for id in entity_ids {
        let latest_key = format!("latest:{}", hex::encode(id));
        let version = self.meta_tree.get(&latest_key)?
            .map(|b| u64::from_be_bytes(b[..8].try_into().unwrap()));
        versions.push(version);
    }

    // Phase 2: Batch fetch records
    let mut results = Vec::with_capacity(entity_ids.len());
    for (id, version) in entity_ids.iter().zip(versions.iter()) {
        let result = match version {
            Some(v) => self.get(id, *v)?.map(|r| (*v, r)),
            None => None,
        };
        results.push(result);
    }

    Ok(results)
}
```

---

## Write Operations

### Put (Insert/Update)

```rust
pub fn put(&self, key: VersionedKey, record: Record) -> Result<()> {
    let key_bytes = key.encode();
    let value_bytes = record.to_bytes()?;

    // 1. Insert versioned record
    self.data_tree.insert(key_bytes, value_bytes)?;

    // 2. Update latest pointer
    self.update_latest(&key.entity_id, key.version_ts)?;

    Ok(())
}

fn update_latest(&self, entity_id: &[u8; 16], version_ts: u64) -> Result<()> {
    let latest_key = format!("latest:{}", hex::encode(entity_id));
    self.meta_tree.insert(&latest_key, &version_ts.to_be_bytes())?;
    Ok(())
}
```

### Put Typed (With Index Updates)

```rust
pub fn put_typed(&self, entity_type: &str, key: VersionedKey, record: Record) -> Result<()> {
    // 1. Write to primary storage
    self.put(key, record.clone())?;

    // 2. Update type index
    let index_key = format!("{}\0{}", entity_type, hex::encode(key.entity_id));
    self.type_index_tree.insert(&index_key, &[])?;

    // 3. Update columnar store
    if let Ok(fields) = decode_entity(&record.data) {
        let projection = self.columnar.projection(entity_type)?;
        projection.update_row(&key.entity_id, &fields)?;
    }

    Ok(())
}
```

### Delete (Soft Delete)

```rust
pub fn delete(&self, entity_id: &[u8; 16]) -> Result<u64> {
    let key = VersionedKey::now(*entity_id);
    let tombstone = Record::tombstone();

    self.put(key, tombstone)?;
    Ok(key.version_ts)
}
```

---

## Scan Operations

### Scan Entity Type

```rust
pub fn scan_entity_type(&self, entity_type: &str)
    -> impl Iterator<Item = Result<([u8; 16], u64, Record)>> + '_
{
    let prefix = format!("{}\0", entity_type);

    self.type_index_tree
        .scan_prefix(&prefix)
        .filter_map(move |result| {
            match result {
                Ok((key, _)) => {
                    // Extract entity_id from key
                    let entity_id = extract_entity_id(&key, prefix.len())?;

                    // Get latest version
                    match self.get_latest(&entity_id) {
                        Ok(Some((version, record))) => {
                            Some(Ok((entity_id, version, record)))
                        }
                        Ok(None) => None,  // Deleted
                        Err(e) => Some(Err(e)),
                    }
                }
                Err(e) => Some(Err(e.into())),
            }
        })
}
```

### Scan Versions

Iterate through entity history:

```rust
pub fn scan_versions(&self, entity_id: &[u8; 16])
    -> impl Iterator<Item = Result<(u64, Record)>> + '_
{
    let min_key = VersionedKey::min_for_entity(*entity_id);
    let max_key = VersionedKey::max_for_entity(*entity_id);

    self.data_tree
        .range(min_key.encode()..=max_key.encode())
        .map(move |result| {
            let (key_bytes, value_bytes) = result?;
            let key = VersionedKey::decode(&key_bytes)?;
            let record = Record::from_bytes(&value_bytes)?;
            Ok((key.version_ts, record))
        })
}
```

---

## Columnar Store

### Purpose

The columnar store organizes data by column for:
- Efficient aggregations
- Analytics queries
- Queries accessing few columns from many rows

### Structure

```rust
pub struct ColumnarStore {
    db: sled::Db,
    projections: DashMap<String, ColumnProjection>,
}

pub struct ColumnProjection {
    entity_type: String,
    columns: DashMap<String, Column>,
}

pub struct Column {
    entity_ids: Vec<[u8; 16]>,
    values: ColumnData,
}

pub enum ColumnData {
    Int32(Vec<Option<i32>>),
    Int64(Vec<Option<i64>>),
    Float64(Vec<Option<f64>>),
    String(StringColumn),
    // ...
}
```

### String Dictionary Compression

```rust
pub struct StringColumn {
    dictionary: StringDictionary,
    encoded: Vec<Option<u32>>,
}

pub struct StringDictionary {
    strings: Vec<String>,
    lookup: HashMap<String, u32>,
}
```

Example:

```
Dictionary:
  0 → "active"
  1 → "inactive"
  2 → "pending"

Encoded values: [0, 0, 1, 2, 0, 0, 1, ...]
```

### Column Operations

```rust
impl Column {
    pub fn sum(&self) -> Option<f64> {
        match &self.values {
            ColumnData::Int32(v) => Some(v.iter().flatten().map(|x| *x as f64).sum()),
            ColumnData::Int64(v) => Some(v.iter().flatten().map(|x| *x as f64).sum()),
            ColumnData::Float64(v) => Some(v.iter().flatten().sum()),
            _ => None,
        }
    }

    pub fn avg(&self) -> Option<f64> {
        match &self.values {
            ColumnData::Int32(v) => {
                let values: Vec<_> = v.iter().flatten().collect();
                if values.is_empty() { return None; }
                Some(values.iter().map(|x| **x as f64).sum::<f64>() / values.len() as f64)
            }
            // ...
        }
    }
}
```

---

## Configuration

```rust
pub struct StorageConfig {
    /// Path to database directory
    path: Option<PathBuf>,

    /// sled cache size in bytes
    cache_size: u64,

    /// Compression enabled
    compression: bool,

    /// Flush interval
    flush_every_ms: Option<u64>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: None,  // In-memory
            cache_size: 1024 * 1024 * 1024,  // 1GB
            compression: true,
            flush_every_ms: Some(1000),
        }
    }
}
```

### Opening Storage

```rust
pub fn open(config: StorageConfig) -> Result<StorageEngine> {
    let sled_config = config.to_sled_config();
    let db = sled_config.open()?;

    let data_tree = db.open_tree("data")?;
    let meta_tree = db.open_tree("meta")?;
    let type_index_tree = db.open_tree("index:entity_type")?;

    let columnar = ColumnarStore::open(&db)?;
    let hash_index = HashIndex::open(&db)?;

    let btree_index = if let Some(path) = config.path() {
        Some(BTreeIndex::open(&path.join("btree_index"))?)
    } else {
        None
    };

    Ok(StorageEngine {
        db,
        data_tree,
        meta_tree,
        type_index_tree,
        columnar,
        hash_index,
        btree_index,
        btree_indexed_columns: RwLock::new(HashSet::new()),
    })
}
```

---

## Durability

### Write-Ahead Logging

sled handles durability through WAL:

1. Changes written to WAL first
2. WAL fsync'd to disk
3. Changes applied to in-memory structures
4. Background flush to data files

### Explicit Flush

```rust
pub fn flush(&self) -> Result<()> {
    self.db.flush()?;
    Ok(())
}
```

### Recovery

```rust
pub fn was_recovered(&self) -> bool {
    self.db.was_recovered()
}
```

On startup, sled replays uncommitted WAL entries.

---

## Performance Characteristics

| Operation | Complexity | Typical Latency |
|-----------|------------|-----------------|
| Get by ID | O(1) | ~10µs |
| Get latest | O(1) + O(1) | ~15µs |
| Put | O(log N) | ~50µs |
| Scan type | O(N) | ~5µs/entity |
| Batch get | O(K) | ~10µs/entity |

### Memory Usage

| Component | Memory |
|-----------|--------|
| sled cache | Configurable (default 1GB) |
| Type index | ~24 bytes/entity |
| Columnar | Varies by data |
| B-tree | ~60 bytes/entry |

---

## Next Steps

- **[Index Internals](index-internals.md)** - Hash and B-tree implementation
- **[Query Engine](query-engine.md)** - How queries use storage
- **[MVCC Concepts](../concepts/mvcc.md)** - Version management
