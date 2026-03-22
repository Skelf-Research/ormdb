# Change Data Capture (CDC)

Stream real-time data changes from ORMDB.

## Overview

Change Data Capture (CDC) allows you to:

- Stream changes in real-time
- Build event-driven architectures
- Sync data to external systems
- Maintain caches and search indexes
- Create audit trails

## Change Log

ORMDB maintains a change log with every mutation:

```rust
pub struct ChangeEntry {
    pub lsn: u64,                    // Log Sequence Number
    pub timestamp: i64,              // Microseconds since epoch
    pub entity: String,              // Entity type
    pub entity_id: [u8; 16],         // Entity ID
    pub operation: Operation,        // Insert, Update, Delete
    pub old_data: Option<EntityData>, // Previous values (update/delete)
    pub new_data: Option<EntityData>, // New values (insert/update)
}

pub enum Operation {
    Insert,
    Update,
    Delete,
}
```

## Streaming Changes

### Basic Streaming

=== "Rust"

    ```rust
    use ormdb_client::cdc::ChangeStream;

    // Start streaming from current position
    let mut stream = client.stream_changes().await?;

    while let Some(change) = stream.next().await? {
        match change.operation {
            Operation::Insert => {
                println!("New {}: {:?}", change.entity, change.entity_id);
            }
            Operation::Update => {
                println!("Updated {}: {:?}", change.entity, change.entity_id);
            }
            Operation::Delete => {
                println!("Deleted {}: {:?}", change.entity, change.entity_id);
            }
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const stream = client.streamChanges();

    for await (const change of stream) {
      switch (change.operation) {
        case "insert":
          console.log(`New ${change.entity}: ${change.entityId}`);
          break;
        case "update":
          console.log(`Updated ${change.entity}: ${change.entityId}`);
          break;
        case "delete":
          console.log(`Deleted ${change.entity}: ${change.entityId}`);
          break;
      }
    }
    ```

=== "Python"

    ```python
    for change in client.stream_changes():
        if change.operation == "insert":
            print(f"New {change.entity}: {change.entity_id}")
        elif change.operation == "update":
            print(f"Updated {change.entity}: {change.entity_id}")
        elif change.operation == "delete":
            print(f"Deleted {change.entity}: {change.entity_id}")
    ```

### Start from Specific Position

```rust
// Start from a specific LSN (useful for recovery)
let stream = client.stream_changes()
    .from_lsn(12345)
    .await?;

// Start from a timestamp
let stream = client.stream_changes()
    .from_timestamp(start_time)
    .await?;

// Start from the beginning (full replay)
let stream = client.stream_changes()
    .from_beginning()
    .await?;
```

### Filter by Entity

```rust
// Only stream specific entities
let stream = client.stream_changes()
    .entities(vec!["User", "Order"])
    .await?;

// Exclude entities
let stream = client.stream_changes()
    .exclude_entities(vec!["Session", "Log"])
    .await?;
```

### Filter by Operation

```rust
// Only inserts
let stream = client.stream_changes()
    .operations(vec![Operation::Insert])
    .await?;

// Inserts and updates (no deletes)
let stream = client.stream_changes()
    .operations(vec![Operation::Insert, Operation::Update])
    .await?;
```

## Checkpointing

Track your progress to resume after failures:

=== "Rust"

    ```rust
    let mut last_lsn: u64 = load_checkpoint().unwrap_or(0);

    let stream = client.stream_changes()
        .from_lsn(last_lsn)
        .await?;

    for change in stream {
        // Process change
        process_change(&change).await?;

        // Checkpoint periodically
        if change.lsn % 1000 == 0 {
            save_checkpoint(change.lsn)?;
        }

        last_lsn = change.lsn;
    }
    ```

=== "TypeScript"

    ```typescript
    let lastLsn = await loadCheckpoint() || 0;

    const stream = client.streamChanges({ fromLsn: lastLsn });

    for await (const change of stream) {
      await processChange(change);

      // Checkpoint every 1000 changes
      if (change.lsn % 1000 === 0) {
        await saveCheckpoint(change.lsn);
      }

      lastLsn = change.lsn;
    }
    ```

## Use Cases

### Search Index Sync

Keep Elasticsearch/Typesense in sync:

```typescript
const stream = client.streamChanges({ entities: ["Product"] });

for await (const change of stream) {
  switch (change.operation) {
    case "insert":
    case "update":
      await searchClient.index("products", {
        id: change.entityId,
        ...change.newData,
      });
      break;
    case "delete":
      await searchClient.delete("products", change.entityId);
      break;
  }
}
```

### Cache Invalidation

```rust
let stream = client.stream_changes()
    .entities(vec!["User", "Product"])
    .await?;

for change in stream {
    let cache_key = format!("{}:{}", change.entity, hex::encode(change.entity_id));
    cache.invalidate(&cache_key).await?;
}
```

### Event Publishing

```typescript
const stream = client.streamChanges();

for await (const change of stream) {
  const event = {
    type: `${change.entity}.${change.operation}`,
    timestamp: change.timestamp,
    data: {
      entityId: change.entityId,
      old: change.oldData,
      new: change.newData,
    },
  };

  await messageQueue.publish("events", event);
}
```

### Audit Log

```rust
let stream = client.stream_changes()
    .from_beginning()
    .await?;

for change in stream {
    let audit_entry = AuditEntry {
        timestamp: change.timestamp,
        entity_type: change.entity,
        entity_id: change.entity_id,
        operation: change.operation,
        old_values: change.old_data,
        new_values: change.new_data,
        // Add user context if available
        user_id: extract_user_id(&change),
    };

    audit_log.append(audit_entry).await?;
}
```

### Real-Time Notifications

```typescript
const stream = client.streamChanges({ entities: ["Order"] });

for await (const change of stream) {
  if (change.operation === "update") {
    const oldStatus = change.oldData?.status;
    const newStatus = change.newData?.status;

    if (oldStatus !== newStatus && newStatus === "shipped") {
      await sendNotification({
        type: "order_shipped",
        orderId: change.entityId,
        userId: change.newData.userId,
      });
    }
  }
}
```

### Data Replication

```rust
// Replicate to secondary database
let stream = source_client.stream_changes()
    .from_lsn(last_replicated_lsn)
    .await?;

for change in stream {
    match change.operation {
        Operation::Insert => {
            target_client.insert(&change.entity, change.new_data.unwrap()).await?;
        }
        Operation::Update => {
            target_client.update(&change.entity, change.entity_id,
                change.new_data.unwrap()).await?;
        }
        Operation::Delete => {
            target_client.delete(&change.entity, change.entity_id).await?;
        }
    }
}
```

## Batch Processing

Process changes in batches for efficiency:

```rust
let stream = client.stream_changes()
    .batch_size(100)
    .batch_timeout(Duration::from_secs(1))
    .await?;

while let Some(batch) = stream.next_batch().await? {
    // Process batch
    let inserts: Vec<_> = batch.iter()
        .filter(|c| c.operation == Operation::Insert)
        .collect();

    bulk_index_to_search(inserts).await?;

    // Checkpoint at batch boundary
    if let Some(last) = batch.last() {
        save_checkpoint(last.lsn)?;
    }
}
```

## Configuration

### Server Configuration

```toml
# ormdb.toml
[cdc]
enabled = true
retention_hours = 168  # 7 days
max_batch_size = 1000
```

### Retention Policy

```bash
# View CDC status
ormdb admin cdc status

# Output:
# CDC Status:
#   Enabled: true
#   Current LSN: 1,234,567
#   Oldest LSN: 1,000,000
#   Retention: 168 hours
#   Disk usage: 256 MB

# Manually trim old changes
ormdb admin cdc trim --before-lsn 1100000
```

## Error Handling

### Reconnection

```rust
loop {
    let result = client.stream_changes()
        .from_lsn(last_lsn)
        .await;

    match result {
        Ok(stream) => {
            for change in stream {
                match change {
                    Ok(c) => {
                        process_change(&c).await?;
                        last_lsn = c.lsn;
                    }
                    Err(e) => {
                        log::error!("Stream error: {}", e);
                        break; // Reconnect
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Connection error: {}", e);
        }
    }

    // Backoff before reconnecting
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

### Handling Gaps

```rust
let stream = client.stream_changes()
    .from_lsn(last_lsn)
    .await?;

for change in stream {
    if change.lsn > last_lsn + 1 {
        log::warn!("Gap detected: {} -> {}", last_lsn, change.lsn);
        // Handle gap (e.g., trigger full sync)
    }

    process_change(&change).await?;
    last_lsn = change.lsn;
}
```

## Best Practices

### 1. Always Checkpoint

```rust
// Don't lose progress
save_checkpoint(change.lsn)?;
```

### 2. Process Idempotently

```typescript
// Handle reprocessing after failures
async function processChange(change: Change) {
  // Use upsert instead of insert
  await searchClient.upsert("products", {
    id: change.entityId,
    ...change.newData,
  });
}
```

### 3. Monitor Lag

```rust
let current_lsn = client.current_lsn().await?;
let lag = current_lsn - last_processed_lsn;

if lag > 10000 {
    alert("CDC consumer is falling behind!");
}
```

### 4. Use Separate Consumers

```yaml
# Run multiple consumers for different purposes
consumers:
  - name: search-sync
    entities: [Product, Category]
    checkpoint: redis://search-checkpoint

  - name: analytics
    entities: [Order, User]
    checkpoint: redis://analytics-checkpoint

  - name: audit
    entities: ["*"]
    checkpoint: redis://audit-checkpoint
```

### 5. Handle Schema Changes

```rust
for change in stream {
    // Check for schema compatibility
    if !is_compatible_schema(&change) {
        log::warn!("Schema change detected, restarting consumer");
        break;
    }

    process_change(&change).await?;
}
```

## API Reference

### ChangeStreamBuilder

| Method | Description |
|--------|-------------|
| `from_lsn(lsn)` | Start from specific LSN |
| `from_timestamp(ts)` | Start from timestamp |
| `from_beginning()` | Start from earliest available |
| `entities(list)` | Filter by entity types |
| `exclude_entities(list)` | Exclude entity types |
| `operations(list)` | Filter by operation types |
| `batch_size(n)` | Set batch size |
| `batch_timeout(d)` | Set batch timeout |

### ChangeEntry Fields

| Field | Type | Description |
|-------|------|-------------|
| `lsn` | `u64` | Log Sequence Number |
| `timestamp` | `i64` | Change timestamp |
| `entity` | `String` | Entity type name |
| `entity_id` | `[u8; 16]` | Entity UUID |
| `operation` | `Operation` | Insert/Update/Delete |
| `old_data` | `Option<EntityData>` | Previous values |
| `new_data` | `Option<EntityData>` | New values |

---

## Next Steps

- **[Real-time Dashboard Example](../examples/realtime-dashboard.md)** - CDC in action
- **[Operations Guide](../operations/index.md)** - Production CDC deployment
- **[Monitoring](../operations/monitoring.md)** - Monitor CDC consumers
