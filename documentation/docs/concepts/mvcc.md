# MVCC Versioning

ORMDB uses Multi-Version Concurrency Control (MVCC) to provide consistent reads, change history, and point-in-time queries. Every write creates a new version rather than overwriting existing data.

---

## What is MVCC?

MVCC allows multiple versions of data to coexist. When you update an entity, the old version remains accessible while a new version is created:

```
Time    Version     Data
─────   ─────────   ──────────────────
100     v1          {name: "Alice"}
200     v2          {name: "Alice Smith"}    ← Update
300     v3          {name: "Alice Johnson"}  ← Update
```

This enables:

- **Non-blocking reads**: Readers never block writers
- **Point-in-time queries**: Query data as it existed at any timestamp
- **Change history**: Track what changed and when
- **Soft deletes**: "Deleted" data remains recoverable

---

## Versioned Keys

Every entity is stored with a versioned key combining the entity ID and a timestamp:

```rust
pub struct VersionedKey {
    /// The entity's unique identifier (16-byte UUID)
    pub entity_id: [u8; 16],
    /// Version timestamp (Unix micros, big-endian for sort order)
    pub version_ts: u64,
}
```

### Key Format

```
┌────────────────┬────────────────┐
│  Entity ID     │  Version TS    │
│  (16 bytes)    │  (8 bytes)     │
└────────────────┴────────────────┘
```

The timestamp is stored in big-endian format so keys sort chronologically when scanned.

### Creating Keys

```rust
// Create a key for "now"
let key = VersionedKey::now(entity_id);

// Create a key with specific timestamp
let key = VersionedKey::new(entity_id, 1704067200_000_000);  // 2024-01-01 00:00:00

// Keys for scanning all versions of an entity
let min_key = VersionedKey::min_for_entity(entity_id);  // timestamp = 0
let max_key = VersionedKey::max_for_entity(entity_id);  // timestamp = MAX
```

---

## Records and Tombstones

### Record Structure

```rust
pub struct Record {
    /// Whether this version is a deletion tombstone
    pub deleted: bool,
    /// The serialized entity data
    pub data: Vec<u8>,
}
```

### Regular Records

When you insert or update an entity:

```rust
let record = Record::new(serialized_data);
storage.put(key, record)?;
```

### Tombstone Records

When you delete an entity, a tombstone is written:

```rust
let tombstone = Record::tombstone();  // deleted=true, data=empty
storage.put(key, tombstone)?;
```

The tombstone marks the entity as deleted at that timestamp, but previous versions remain accessible.

---

## Reading Data

### Get Latest Version

The most common operation - get the current state:

```rust
let (version_ts, record) = storage.get_latest(&entity_id)?;
```

This looks up the latest version timestamp from the metadata tree, then fetches that version's record.

### Get Specific Version

Retrieve data as it existed at a specific version:

```rust
let record = storage.get(&entity_id, version_ts)?;
```

### Point-in-Time Queries

Get the version that was current at a specific timestamp:

```rust
// What did this entity look like at timestamp 150?
let (version_ts, record) = storage.get_at(&entity_id, 150)?;
// Returns version 100 (the version active at time 150)
```

This scans backwards from the requested timestamp to find the newest version that existed before or at that time.

### Scan All Versions

Iterate through an entity's complete history:

```rust
for result in storage.scan_versions(&entity_id) {
    let (version_ts, record) = result?;
    if record.deleted {
        println!("Deleted at {}", version_ts);
    } else {
        println!("Version {} data: {:?}", version_ts, record.data);
    }
}
```

---

## Latest Version Tracking

For efficient "get latest" operations, ORMDB maintains a metadata tree with latest version pointers:

```
Meta Tree
─────────────────────────────────
latest:<entity_id_1> → 300
latest:<entity_id_2> → 450
latest:<entity_id_3> → 200
```

When you write a new version:

```rust
fn put(&self, key: VersionedKey, record: Record) -> Result<()> {
    // 1. Insert the versioned record
    self.data_tree.insert(key.encode(), record.to_bytes()?)?;

    // 2. Update the latest version pointer
    self.update_latest(&key.entity_id, key.version_ts)?;

    Ok(())
}
```

---

## Soft Deletes

By default, deletes are "soft" - they write a tombstone rather than removing data:

```rust
// Soft delete (default)
storage.delete(&entity_id)?;

// What happens internally:
// 1. Create tombstone record (deleted=true)
// 2. Write with new timestamp
// 3. Update latest pointer
```

### Benefits

1. **Recovery**: Accidentally deleted data can be recovered
2. **Audit**: You can see when entities were deleted
3. **References**: Foreign keys can still resolve the deleted entity's history

### Checking Deletion Status

```rust
let (version_ts, record) = storage.get_latest(&entity_id)?;
if record.deleted {
    println!("Entity was deleted at {}", version_ts);
}
```

### Reading Deleted Entity's History

```rust
// Even after deletion, you can read the pre-deletion versions
for result in storage.scan_versions(&entity_id) {
    let (ts, record) = result?;
    if !record.deleted {
        println!("Active version at {}: {:?}", ts, record.data);
    }
}
```

---

## Concurrency Model

### Writers

- Writers always create new versions (no conflicts possible)
- Multiple writers can write different entities concurrently
- Same-entity writes are serialized by timestamp

### Readers

- Readers always see a consistent snapshot
- Reading doesn't block writing
- No locks required for read operations

### Example

```
Time    Writer A                Writer B                Reader
────    ─────────────────────   ─────────────────────   ─────────────────
100     Write User:1 @ 100
150                             Write User:2 @ 150      Read User:1 → v100
200     Write User:1 @ 200
250                                                     Read User:1 → v200
300                             Write User:1 @ 300
350     Read User:1 → v300
```

Both writers and readers can proceed without blocking each other.

---

## Storage Layout

### Data Tree

Stores versioned records:

```
Key: [entity_id:16][version_ts:8]
Value: Record { deleted, data }
```

Sorted by entity_id then version_ts, enabling efficient:
- Get specific version: point lookup
- Get latest: metadata lookup + point lookup
- Scan versions: range scan within entity prefix

### Metadata Tree

Stores latest version pointers and other metadata:

```
Key: "latest:" + entity_id
Value: version_ts (8 bytes, big-endian)
```

### Type Index Tree

Stores entity type membership for efficient scans:

```
Key: entity_type + "\0" + entity_id
Value: (empty)
```

---

## Version Retention

By default, all versions are retained indefinitely. For production systems, you may want to implement version cleanup:

### Retention Strategies

1. **Time-based**: Keep versions from the last N days
2. **Count-based**: Keep the last N versions per entity
3. **Hybrid**: Keep last N versions OR versions from last N days

### Cleanup Considerations

- Don't delete the latest version (even if it's a tombstone)
- Consider audit/compliance requirements
- Run cleanup during low-traffic periods
- Backup before cleanup

---

## Use Cases

### Audit Logging

Track who changed what and when:

```rust
// Get the change history
let history: Vec<_> = storage.scan_versions(&entity_id).collect();

for (i, (ts, record)) in history.iter().enumerate() {
    if i > 0 {
        let prev = &history[i - 1].1;
        let changes = diff(&prev.data, &record.data);
        println!("At {}: Changed {:?}", ts, changes);
    }
}
```

### Undo/Rollback

Restore a previous version:

```rust
// Get the previous version
let (_, old_record) = storage.get_at(&entity_id, before_bad_change)?;

// Write it as a new version (effectively "undo")
let new_key = VersionedKey::now(entity_id);
storage.put(new_key, old_record)?;
```

### Time-Travel Queries

Query data as it existed in the past:

```rust
// Report data as of end of last month
let report_time = end_of_last_month();

for entity_id in entity_ids {
    if let Some((_, record)) = storage.get_at(&entity_id, report_time)? {
        process_for_report(&record)?;
    }
}
```

### Conflict-Free Sync

Each version has a unique timestamp, making merge conflicts impossible:

```rust
// Client A and B both update the same entity
// They each create a new version with their current timestamp
// The higher timestamp "wins" as the latest version
// Both versions are preserved in history
```

---

## Performance Considerations

### Write Amplification

Each update writes a full new record, not a delta. For frequently-updated entities with large records, consider:

- Normalizing frequently-changing fields into separate entities
- Using the columnar store for read-heavy analytics

### Version Scan Performance

Scanning all versions is O(N) where N is the number of versions. For entities with many versions:

- Use pagination when displaying history
- Consider version cleanup for old data
- Index frequently-queried timestamp ranges

### Latest Version Optimization

Getting the latest version is optimized with metadata pointers:

```
Without metadata: O(log N) to find max timestamp in range
With metadata: O(1) lookup + O(log N) record fetch
```

---

## Next Steps

- **[Storage Architecture](storage-architecture.md)** - How data is organized
- **[CDC Guide](../guides/cdc.md)** - Change Data Capture using versions
- **[Transactions Tutorial](../tutorials/transactions.md)** - Atomic multi-entity writes
