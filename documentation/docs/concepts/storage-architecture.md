# Storage Architecture

ORMDB uses a hybrid storage architecture combining row-oriented storage for transactional workloads with columnar storage for analytics. Multiple index types accelerate different query patterns.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Storage Engine                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   Data Tree     │  │   Meta Tree     │  │  Type Index     │  │
│  │  (Row Store)    │  │  (Pointers)     │  │   Tree          │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │  Columnar       │  │   Hash Index    │  │  B-tree Index   │  │
│  │  Store          │  │   (Equality)    │  │   (Range)       │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                      sled (Embedded KV Store)                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Row Store (Primary Storage)

The row store is the source of truth for all entity data. It's built on [sled](https://sled.rs), a modern embedded database.

### Data Tree

Stores versioned entity records:

```
Key Format:
┌────────────────────────────┬────────────────────────────┐
│      Entity ID (16 bytes)  │  Version Timestamp (8 bytes) │
└────────────────────────────┴────────────────────────────┘

Value Format:
┌──────────────┬─────────────────────────────────────────┐
│ Deleted Flag │          Serialized Entity Data          │
│  (1 byte)    │          (Variable length)               │
└──────────────┴─────────────────────────────────────────┘
```

### Meta Tree

Stores metadata for fast lookups:

```
latest:<entity_id> → <version_timestamp>
```

This enables O(1) latest version lookups without scanning.

### Type Index Tree

Enables efficient entity type scans:

```
<entity_type>\0<entity_id> → (empty value)
```

Example:
```
User\0abc123... →
User\0def456... →
Post\0ghi789... →
```

Scanning "User" entities becomes a simple prefix scan.

---

## Columnar Store

The columnar store organizes data by column rather than by row, optimized for:

- Aggregation queries (SUM, AVG, COUNT)
- Analytics workloads
- Queries accessing few columns from many rows

### Column Projections

Each entity type has a projection storing columns separately:

```
Projection: User
├── Column: id       → [uuid1, uuid2, uuid3, ...]
├── Column: name     → [alice, bob, charlie, ...]
├── Column: age      → [25, 30, 28, ...]
└── Column: status   → [1, 1, 2, ...]  (dictionary-encoded)
```

### String Dictionary Compression

String columns use dictionary encoding to reduce storage:

```
Dictionary:
0 → "active"
1 → "inactive"
2 → "pending"

Encoded Column:
[0, 0, 1, 0, 2, 0, 0, 1, ...]
```

Benefits:
- Reduced storage for repeated values
- Faster comparisons (integer vs string)
- Better cache utilization

### Synchronization

The columnar store is updated alongside the row store:

```rust
// When writing to row store, also update columnar
fn put_typed(&self, entity_type: &str, key: VersionedKey, record: Record) {
    // 1. Write to row store
    self.put(key, record.clone())?;

    // 2. Update type index
    self.type_index_tree.insert(type_key, &[])?;

    // 3. Update columnar projection
    if let Ok(fields) = decode_entity(&record.data) {
        let projection = self.columnar.projection(entity_type)?;
        projection.update_row(&key.entity_id, &fields)?;
    }
}
```

---

## Hash Index

Hash indexes provide O(1) equality lookups on indexed columns.

### Structure

```
Key: <entity_type>:<field_name>:<value_hash>
Value: [entity_id_1, entity_id_2, ...]
```

### Use Cases

Perfect for:
- `WHERE status = 'active'`
- `WHERE user_id = ?`
- `WHERE email = ?`

Not suitable for:
- Range queries (`age > 25`)
- Pattern matching (`LIKE '%smith'`)
- Sorting

### Operations

```rust
// Insert into index
hash_index.insert("User", "status", &Value::String("active"), entity_id)?;

// Lookup by value
let entity_ids = hash_index.lookup("User", "status", &Value::String("active"))?;

// Remove from index
hash_index.remove("User", "status", &Value::String("active"), entity_id)?;
```

### Automatic Updates

When entity data changes, hash indexes are updated automatically:

```rust
fn update_hash_indexes(
    &self,
    entity_type: &str,
    entity_id: [u8; 16],
    before: Option<&[(String, Value)]>,
    after: Option<&[(String, Value)]>,
) {
    for field in changed_fields {
        if let Some(old_value) = before.get(field) {
            hash_index.remove(entity_type, field, old_value, entity_id)?;
        }
        if let Some(new_value) = after.get(field) {
            hash_index.insert(entity_type, field, new_value, entity_id)?;
        }
    }
}
```

---

## B-tree Index

B-tree indexes provide O(log N) lookups with support for range queries.

### Structure

```
Key: <entity_type>:<column>:<value_bytes>
Value: entity_id
```

Values are encoded to maintain sort order (big-endian for numbers).

### Use Cases

Perfect for:
- Range queries (`age BETWEEN 18 AND 65`)
- Inequality (`price < 100`)
- Ordering (`ORDER BY created_at`)
- Combined range + equality

### Operations

```rust
// Insert
btree.insert("User", "age", &Value::Int32(25), entity_id)?;

// Range scan: age > 18
let ids = btree.scan_greater_than("User", "age", &Value::Int32(18))?;

// Range scan: age < 65
let ids = btree.scan_less_than("User", "age", &Value::Int32(65))?;

// Range scan: age BETWEEN 18 AND 65
let ids = btree.scan_range("User", "age", &Value::Int32(18), &Value::Int32(65))?;
```

### Lazy Index Building

B-tree indexes are built lazily when first needed:

```rust
// Ensure index exists before range query
fn ensure_btree_index(&self, entity_type: &str, column: &str) -> Result<bool> {
    if self.btree_indexed_columns.contains(&(entity_type, column)) {
        return Ok(true);  // Already built
    }

    // Build from columnar data
    let projection = self.columnar.projection(entity_type)?;
    self.btree.build_for_column(
        entity_type,
        column,
        projection.scan_column(column)
    )?;

    self.btree_indexed_columns.insert((entity_type, column));
    Ok(true)
}
```

---

## Index Selection

The query executor automatically selects the best index for each query:

### Decision Matrix

| Query Pattern | Best Index | Complexity |
|--------------|------------|------------|
| `field = value` | Hash | O(1) |
| `field > value` | B-tree | O(log N + K) |
| `field < value` | B-tree | O(log N + K) |
| `field BETWEEN a AND b` | B-tree | O(log N + K) |
| `field LIKE 'prefix%'` | B-tree* | O(log N + K) |
| `field LIKE '%suffix'` | Full scan | O(N) |
| `field IN (a, b, c)` | Hash | O(K) |

*Prefix patterns can use B-tree with range scan

### Example Selection

```rust
// Query: WHERE status = 'active' AND age > 25

// Step 1: Use hash index for equality (fastest filter)
let active_ids = hash_index.lookup("User", "status", "active")?;

// Step 2: Use B-tree for range (if available) or filter in memory
let final_ids = if btree.has_index("User", "age") {
    // Intersect with B-tree range
    let age_ids = btree.scan_greater_than("User", "age", 25)?;
    intersect(active_ids, age_ids)
} else {
    // Filter in memory
    active_ids.into_iter()
        .filter(|id| get_age(id) > 25)
        .collect()
};
```

---

## Changelog and Async Index Updates

For high-throughput writes, indexes can be updated asynchronously using a changelog:

### Changelog Structure

```rust
pub struct ChangelogEntry {
    pub lsn: u64,              // Log sequence number
    pub entity_type: String,
    pub entity_id: [u8; 16],
    pub operation: Operation,   // Insert, Update, Delete
    pub fields: Vec<(String, Value)>,
}
```

### Write Path with Changelog

```rust
// 1. Write to row store (synchronous)
storage.put_typed("User", key, record)?;

// 2. Append to changelog (fast, append-only)
changelog.append(ChangelogEntry { ... })?;

// 3. Index worker processes changelog (background)
// - Updates hash indexes
// - Updates B-tree indexes
// - Updates columnar projections
```

### Query Path with Pending Changes

```rust
// Merge committed index with pending changelog entries
fn lookup_with_changelog(&self, entity_type: &str, field: &str, value: &Value) -> Vec<[u8; 16]> {
    // Get committed results from hash index
    let mut ids = self.hash_index.lookup(entity_type, field, value)?;

    // Add pending entries from changelog
    let pending = changelog.pending_ids_for_value(entity_type, field, value);
    ids.extend(pending);

    ids.dedup();
    ids
}
```

---

## Storage Configuration

### Configuration Options

```rust
pub struct StorageConfig {
    /// Path to database directory
    pub path: PathBuf,

    /// sled cache size in bytes (default: 1GB)
    pub cache_size: u64,

    /// Enable compression (default: true)
    pub compression: bool,

    /// Flush mode (default: automatic)
    pub flush_every_ms: Option<u64>,
}
```

### Opening Storage

```rust
let config = StorageConfig::new("/path/to/data")
    .with_cache_size(2 * 1024 * 1024 * 1024)  // 2GB cache
    .with_compression(true);

let storage = StorageEngine::open(config)?;
```

---

## Performance Characteristics

### Write Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Single insert | ~50µs | Includes type index update |
| Batch insert (100) | ~2ms | Amortized: ~20µs/entity |
| Update with indexes | ~100µs | Hash + columnar update |

### Read Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Get by ID | ~10µs | Metadata + data lookup |
| Hash index lookup | ~15µs | O(1) |
| B-tree range scan (1K results) | ~500µs | O(log N + K) |
| Full entity scan (10K entities) | ~50ms | O(N) |

### Memory Usage

| Component | Memory | Notes |
|-----------|--------|-------|
| sled cache | Configurable | Default 1GB |
| Hash index | ~50 bytes/entry | In-memory during queries |
| B-tree index | ~60 bytes/entry | Memory-mapped |
| Columnar | ~varies | Dictionary compression |

---

## Durability

### Write-Ahead Logging

sled uses write-ahead logging for durability:

1. Write to WAL (fsync)
2. Update in-memory index
3. Background flush to data files

### Recovery

On startup, sled:

1. Reads WAL
2. Replays uncommitted transactions
3. Reports recovery status

```rust
if storage.was_recovered() {
    log::info!("Database recovered from crash");
}
```

### Explicit Flush

Force durability for critical writes:

```rust
storage.put_typed("Order", key, record)?;
storage.flush()?;  // Ensures data is on disk
```

---

## Best Practices

### 1. Choose the Right Index Type

```rust
// High-cardinality equality lookups: Hash
// user_id, order_id, email

// Range queries and sorting: B-tree
// created_at, price, age

// Don't index:
// - Frequently updated columns
// - Very low cardinality (e.g., boolean with 50/50 split)
```

### 2. Monitor Disk Usage

```rust
let size = storage.size_on_disk()?;
println!("Database size: {} MB", size / 1024 / 1024);
```

### 3. Use Batch Operations

```rust
// Bad: Many small writes
for item in items {
    storage.put_typed("Item", VersionedKey::now(id), record)?;
}

// Good: Batch within transaction
let batch = storage.transaction()?;
for item in items {
    batch.put_typed("Item", VersionedKey::now(id), record)?;
}
batch.commit()?;
```

### 4. Configure Cache Appropriately

```rust
// Working set < cache: Excellent performance
// Working set > cache: Frequent disk reads

// Rule of thumb: cache = 2x hot data size
let config = StorageConfig::new(path)
    .with_cache_size(hot_data_estimate * 2);
```

---

## Next Steps

- **[MVCC](mvcc.md)** - How versioning works
- **[Performance Guide](../guides/performance.md)** - Optimization tips
- **[Architecture Overview](../architecture/index.md)** - Full system architecture
