# Index Internals

ORMDB uses two index types: hash indexes for O(1) equality lookups and B-tree indexes for O(log N) range queries.

---

## Index Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Index Layer                              │
│                                                                  │
│  ┌─────────────────────────┐    ┌─────────────────────────┐     │
│  │      Hash Index         │    │     B-tree Index        │     │
│  │                         │    │                         │     │
│  │  O(1) equality          │    │  O(log N) range         │     │
│  │  WHERE x = value        │    │  WHERE x > value        │     │
│  │  WHERE x IN (...)       │    │  WHERE x BETWEEN a,b    │     │
│  │                         │    │  ORDER BY x             │     │
│  └─────────────────────────┘    └─────────────────────────┘     │
│                                                                  │
│                    Backed by sled trees                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Hash Index

### Structure

```rust
pub struct HashIndex {
    tree: sled::Tree,
}
```

### Key Format

```
Key: <entity_type>:<field>:<value_hash>
Value: Compressed list of entity IDs

Example:
User:status:hash("active") → [id1, id2, id3, ...]
User:status:hash("pending") → [id4, id5, ...]
Post:published:hash(true) → [id6, id7, id8, ...]
```

### Value Hashing

```rust
fn hash_value(value: &Value) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();

    match value {
        Value::String(s) => {
            hasher.update(b"string:");
            hasher.update(s.as_bytes());
        }
        Value::Int32(n) => {
            hasher.update(b"i32:");
            hasher.update(&n.to_le_bytes());
        }
        Value::Int64(n) => {
            hasher.update(b"i64:");
            hasher.update(&n.to_le_bytes());
        }
        Value::Bool(b) => {
            hasher.update(b"bool:");
            hasher.update(&[*b as u8]);
        }
        Value::Uuid(bytes) => {
            hasher.update(b"uuid:");
            hasher.update(bytes);
        }
        // ...
    }

    let hash = hasher.finalize();
    hash.as_bytes()[..16].try_into().unwrap()
}
```

### Operations

#### Insert

```rust
pub fn insert(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
    entity_id: [u8; 16],
) -> Result<()> {
    let key = self.make_key(entity_type, field, value);

    // Read current ID list
    let mut ids = match self.tree.get(&key)? {
        Some(bytes) => decompress_ids(&bytes)?,
        None => Vec::new(),
    };

    // Add new ID if not present
    if !ids.contains(&entity_id) {
        ids.push(entity_id);

        // Write back
        let compressed = compress_ids(&ids)?;
        self.tree.insert(&key, compressed)?;
    }

    Ok(())
}
```

#### Remove

```rust
pub fn remove(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
    entity_id: [u8; 16],
) -> Result<()> {
    let key = self.make_key(entity_type, field, value);

    if let Some(bytes) = self.tree.get(&key)? {
        let mut ids = decompress_ids(&bytes)?;

        // Remove ID
        ids.retain(|id| *id != entity_id);

        if ids.is_empty() {
            // Remove entry entirely
            self.tree.remove(&key)?;
        } else {
            // Write back reduced list
            let compressed = compress_ids(&ids)?;
            self.tree.insert(&key, compressed)?;
        }
    }

    Ok(())
}
```

#### Lookup

```rust
pub fn lookup(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
) -> Result<Vec<[u8; 16]>> {
    let key = self.make_key(entity_type, field, value);

    match self.tree.get(&key)? {
        Some(bytes) => decompress_ids(&bytes),
        None => Ok(Vec::new()),
    }
}
```

### ID List Compression

```rust
fn compress_ids(ids: &[[u8; 16]]) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(ids.len() * 16);
    for id in ids {
        data.extend_from_slice(id);
    }
    Ok(lz4::compress(&data))
}

fn decompress_ids(bytes: &[u8]) -> Result<Vec<[u8; 16]>> {
    let data = lz4::decompress(bytes)?;
    let mut ids = Vec::with_capacity(data.len() / 16);
    for chunk in data.chunks_exact(16) {
        let mut id = [0u8; 16];
        id.copy_from_slice(chunk);
        ids.push(id);
    }
    Ok(ids)
}
```

### Batch Insert

For bulk loading, batch insert is more efficient:

```rust
pub fn insert_batch(
    &self,
    entity_type: &str,
    field: &str,
    entries: &[(Value, [u8; 16])],
) -> Result<()> {
    // Group by value hash
    let mut groups: HashMap<Vec<u8>, Vec<[u8; 16]>> = HashMap::new();

    for (value, id) in entries {
        let key = self.make_key(entity_type, field, value);
        groups.entry(key).or_default().push(*id);
    }

    // Write each group
    for (key, new_ids) in groups {
        let mut ids = match self.tree.get(&key)? {
            Some(bytes) => decompress_ids(&bytes)?,
            None => Vec::new(),
        };

        // Add new IDs
        for id in new_ids {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }

        let compressed = compress_ids(&ids)?;
        self.tree.insert(&key, compressed)?;
    }

    Ok(())
}
```

---

## B-tree Index

### Structure

```rust
pub struct BTreeIndex {
    db: sled::Db,
}
```

### Key Format

```
Key: <entity_type>:<field>:<encoded_value>
Value: entity_id

Example (Int32 field "age"):
User:age:\x00\x00\x00\x12 → id1   (age = 18)
User:age:\x00\x00\x00\x13 → id2   (age = 19)
User:age:\x00\x00\x00\x14 → id3   (age = 20)
User:age:\x00\x00\x00\x19 → id4   (age = 25)
```

Values are encoded to preserve sort order.

### Value Encoding

```rust
fn encode_value(value: &Value) -> Vec<u8> {
    match value {
        // Integers: big-endian with sign bit flip
        Value::Int32(n) => {
            let flipped = (*n as u32) ^ 0x80000000;
            flipped.to_be_bytes().to_vec()
        }
        Value::Int64(n) => {
            let flipped = (*n as u64) ^ 0x8000000000000000;
            flipped.to_be_bytes().to_vec()
        }

        // Floats: IEEE 754 with sign handling
        Value::Float64(f) => {
            let bits = f.to_bits();
            let encoded = if *f >= 0.0 {
                bits ^ 0x8000000000000000
            } else {
                !bits
            };
            encoded.to_be_bytes().to_vec()
        }

        // Strings: raw bytes (naturally sorted)
        Value::String(s) => s.as_bytes().to_vec(),

        // Timestamps: i64 microseconds
        Value::Timestamp(ts) => {
            let flipped = (*ts as u64) ^ 0x8000000000000000;
            flipped.to_be_bytes().to_vec()
        }

        // ...
    }
}
```

### Operations

#### Insert

```rust
pub fn insert(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
    entity_id: [u8; 16],
) -> Result<()> {
    let tree = self.tree_for_field(entity_type, field)?;
    let key = encode_value(value);

    // Multiple entities may have same value
    // Use composite key: value + entity_id
    let mut full_key = key;
    full_key.extend_from_slice(&entity_id);

    tree.insert(&full_key, &entity_id)?;
    Ok(())
}
```

#### Remove

```rust
pub fn remove(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
    entity_id: [u8; 16],
) -> Result<()> {
    let tree = self.tree_for_field(entity_type, field)?;
    let mut key = encode_value(value);
    key.extend_from_slice(&entity_id);

    tree.remove(&key)?;
    Ok(())
}
```

#### Range Scan

```rust
pub fn scan_range(
    &self,
    entity_type: &str,
    field: &str,
    min: &Value,
    max: &Value,
) -> Result<Vec<[u8; 16]>> {
    let tree = self.tree_for_field(entity_type, field)?;

    let min_key = encode_value(min);
    let max_key = encode_value(max);

    let ids: Vec<[u8; 16]> = tree
        .range(min_key..=max_key)
        .filter_map(|result| {
            let (_, value) = result.ok()?;
            let mut id = [0u8; 16];
            id.copy_from_slice(&value);
            Some(id)
        })
        .collect();

    Ok(ids)
}
```

#### Greater Than

```rust
pub fn scan_greater_than(
    &self,
    entity_type: &str,
    field: &str,
    value: &Value,
) -> Result<Vec<[u8; 16]>> {
    let tree = self.tree_for_field(entity_type, field)?;
    let min_key = encode_value(value);

    // Add suffix to exclude exact match
    let mut exclusive_min = min_key.clone();
    exclusive_min.push(0xFF);

    let ids: Vec<[u8; 16]> = tree
        .range(exclusive_min..)
        .filter_map(|result| {
            let (_, value) = result.ok()?;
            let mut id = [0u8; 16];
            id.copy_from_slice(&value);
            Some(id)
        })
        .collect();

    Ok(ids)
}
```

### Lazy Index Building

B-tree indexes are built on demand:

```rust
pub fn ensure_btree_index(
    &self,
    entity_type: &str,
    column_name: &str,
) -> Result<bool> {
    // Check if already built
    let key = (entity_type.to_string(), column_name.to_string());
    if self.indexed_columns.read()?.contains(&key) {
        return Ok(true);
    }

    // Build from columnar data
    let projection = self.columnar.projection(entity_type)?;
    let column_iter = projection.scan_column(column_name);

    let tree = self.tree_for_field(entity_type, column_name)?;

    for (entity_id, value) in column_iter {
        let mut key = encode_value(&value);
        key.extend_from_slice(&entity_id);
        tree.insert(&key, &entity_id)?;
    }

    // Mark as built
    self.indexed_columns.write()?.insert(key);

    Ok(true)
}
```

---

## Index Selection

The query executor chooses indexes automatically:

```rust
fn select_index(
    filter: &FilterExpr,
    hash_index: &HashIndex,
    btree_index: Option<&BTreeIndex>,
) -> IndexChoice {
    match filter {
        // Equality: prefer hash index
        FilterExpr::Eq { field, value } => {
            IndexChoice::Hash { field, value }
        }

        // Range: require B-tree
        FilterExpr::Gt { field, value } |
        FilterExpr::Gte { field, value } |
        FilterExpr::Lt { field, value } |
        FilterExpr::Lte { field, value } => {
            if btree_index.is_some() {
                IndexChoice::BTree { field, range: ... }
            } else {
                IndexChoice::Scan  // Fall back
            }
        }

        // IN: multiple hash lookups
        FilterExpr::In { field, values } => {
            IndexChoice::HashMultiple { field, values }
        }

        // LIKE with prefix: B-tree range
        FilterExpr::Like { field, pattern } if !pattern.starts_with('%') => {
            if btree_index.is_some() {
                let prefix = pattern.split('%').next().unwrap();
                IndexChoice::BTreePrefix { field, prefix }
            } else {
                IndexChoice::Scan
            }
        }

        // Complex: evaluate and intersect
        FilterExpr::And { left, right } => {
            let left_choice = select_index(left, ...);
            let right_choice = select_index(right, ...);
            IndexChoice::Intersect(left_choice, right_choice)
        }

        _ => IndexChoice::Scan,
    }
}
```

---

## Index Maintenance

### Automatic Updates

When entities are written, indexes are updated:

```rust
pub fn update_secondary_indexes(
    &self,
    entity_type: &str,
    entity_id: [u8; 16],
    before: Option<&[(String, Value)]>,
    after: Option<&[(String, Value)]>,
) -> Result<()> {
    let before_map = to_map(before);
    let after_map = to_map(after);

    // Find changed fields
    let all_fields: HashSet<_> = before_map.keys()
        .chain(after_map.keys())
        .collect();

    for field in all_fields {
        let before_value = before_map.get(field);
        let after_value = after_map.get(field);

        if before_value == after_value {
            continue;
        }

        // Update hash index
        if let Some(v) = before_value {
            self.hash_index.remove(entity_type, field, v, entity_id)?;
        }
        if let Some(v) = after_value {
            self.hash_index.insert(entity_type, field, v, entity_id)?;
        }

        // Update B-tree index if exists
        if let Some(btree) = &self.btree_index {
            if self.btree_indexed_columns.read()?.contains(&(entity_type, field)) {
                if let Some(v) = before_value {
                    btree.remove(entity_type, field, v, entity_id)?;
                }
                if let Some(v) = after_value {
                    btree.insert(entity_type, field, v, entity_id)?;
                }
            }
        }
    }

    Ok(())
}
```

### Async Index Updates

For high-throughput writes, indexes can be updated asynchronously:

```rust
// Write path
storage.put_typed("User", key, record)?;
changelog.append(ChangelogEntry::new("User", entity_id, fields))?;

// Background worker
loop {
    let entry = changelog.next()?;
    storage.update_secondary_indexes(
        &entry.entity_type,
        entry.entity_id,
        entry.before_fields,
        entry.after_fields,
    )?;
    changelog.mark_processed(entry.lsn)?;
}
```

---

## Performance Characteristics

### Hash Index

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Insert | O(1) amortized | May need to read/write ID list |
| Remove | O(n) | n = IDs with same value |
| Lookup | O(1) | Plus decompression |

Best for:
- High cardinality fields (user_id, email)
- Equality filters

### B-tree Index

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Insert | O(log N) | N = total entries |
| Remove | O(log N) | |
| Point lookup | O(log N) | |
| Range scan | O(log N + K) | K = results |

Best for:
- Range queries (age > 18, date between)
- Sorting (ORDER BY created_at)
- Prefix matching (email LIKE 'admin%')

---

## Best Practices

### When to Index

**Hash index (automatic on indexed fields):**
- Foreign key fields
- Status/enum fields with filters
- Unique constraint fields

**B-tree index (built on demand):**
- Date/time fields for sorting
- Numeric fields for ranges
- Text fields for prefix search

### When Not to Index

- Very low cardinality (boolean with 50/50 split)
- Frequently updated fields
- Fields rarely used in queries

---

## Next Steps

- **[Storage Engine](storage-engine.md)** - Data organization
- **[Query Engine](query-engine.md)** - How indexes are used
- **[Performance Guide](../guides/performance.md)** - Optimization tips
