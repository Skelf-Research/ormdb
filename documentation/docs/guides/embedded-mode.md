# Embedded Mode

Run ORMDB directly in your Rust application without a separate server.

## Overview

ORMDB Embedded is an in-process graph database library that eliminates the need for network communication or server management. It provides the full power of ORMDB's typed queries and graph fetches while running entirely within your application process.

### When to Use Embedded Mode

| Use Case | Embedded | Client/Server |
|----------|----------|---------------|
| Single-process applications | Recommended | - |
| Desktop/CLI applications | Recommended | - |
| Microservices | Good fit | Recommended |
| Multi-language stack | - | Recommended |
| Zero network latency required | Recommended | - |
| Horizontal scaling needed | - | Recommended |
| Development/testing | Recommended | - |

### Feature Comparison

| Feature | Embedded | Client/Server |
|---------|----------|---------------|
| Queries | Full support | Full support |
| Mutations | Full support | Full support |
| Transactions (OCC) | Full support | Full support |
| Aggregates | Planned | Full support |
| Schema management | Full support | Full support |
| Change streams (CDC) | Planned | Full support |
| Backup/restore | Full support | Full support |
| Multi-writer | Yes (OCC) | Yes |
| Multi-language | Rust only | Rust, TypeScript, Python |
| Network overhead | None | HTTP/TCP |

---

## Installation

Add the `ormdb` crate to your `Cargo.toml`:

```toml
[dependencies]
ormdb = "0.1"
```

### Feature Flags

| Feature | Description |
|---------|-------------|
| `default` | Core embedded database |
| `async` | Async API (requires `tokio`) |
| `query-lang` | Query language parser |
| `full` | All features enabled |

```toml
# With async support
ormdb = { version = "0.1", features = ["async"] }

# All features
ormdb = { version = "0.1", features = ["full"] }
```

---

## Opening a Database

### File-Based Database

```rust
use ormdb::Database;

fn main() -> ormdb::Result<()> {
    // Open or create a database at the specified path
    let db = Database::open("./my_data")?;

    // Database is ready to use
    Ok(())
}
```

### In-Memory Database

```rust
use ormdb::Database;

// Perfect for testing - data is lost when dropped
let db = Database::open_memory()?;
```

### Custom Configuration

```rust
use ormdb::{Database, Config, RetentionPolicy};

let config = Config {
    cache_capacity: 2 * 1024 * 1024 * 1024,  // 2GB cache
    flush_every_ms: Some(1000),               // Flush every second
    compression: true,                         // Enable compression
    retention: RetentionPolicy::default(),    // MVCC garbage collection
};

let db = Database::open_with_config("./my_data", config)?;
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `cache_capacity` | `u64` | 1GB | In-memory cache size in bytes |
| `flush_every_ms` | `Option<u64>` | `None` | Auto-flush interval; `None` = flush on every write |
| `compression` | `bool` | `true` | Enable storage compression |
| `retention` | `RetentionPolicy` | default | MVCC version garbage collection policy |

---

## Schema Definition

Define your schema using the fluent builder API.

### Basic Schema

```rust
use ormdb::{Database, ScalarType};

let db = Database::open("./my_data")?;

db.schema()
    .entity("User")
        .field("id", ScalarType::Uuid).primary_key()
        .field("name", ScalarType::String)
        .field("email", ScalarType::String).unique()
        .field("age", ScalarType::Int32).optional()
    .entity("Post")
        .field("id", ScalarType::Uuid).primary_key()
        .field("title", ScalarType::String)
        .field("content", ScalarType::String)
        .field("published", ScalarType::Bool)
        .relation("author", "User").required()
    .apply()?;
```

### Supported Field Types

| Type | Rust Equivalent | Description |
|------|-----------------|-------------|
| `ScalarType::Uuid` | `[u8; 16]` | 128-bit UUID |
| `ScalarType::String` | `String` | UTF-8 string |
| `ScalarType::Int32` | `i32` | 32-bit signed integer |
| `ScalarType::Int64` | `i64` | 64-bit signed integer |
| `ScalarType::Float64` | `f64` | 64-bit floating point |
| `ScalarType::Bool` | `bool` | Boolean |
| `ScalarType::Bytes` | `Vec<u8>` | Binary data |
| `ScalarType::Timestamp` | `i64` | Unix timestamp |
| `ScalarType::Json` | `serde_json::Value` | JSON data |

### Field Modifiers

| Method | Description |
|--------|-------------|
| `.primary_key()` | Mark as primary key (required, one per entity) |
| `.unique()` | Create unique constraint |
| `.indexed()` | Create secondary index |
| `.optional()` | Allow null values |
| `.required()` | Disallow null values (default) |

### Relations

```rust
db.schema()
    .entity("Comment")
        .field("id", ScalarType::Uuid).primary_key()
        .field("content", ScalarType::String)
        .relation("post", "Post").required()      // Required relation
        .relation("author", "User").required()
        .relation("parent", "Comment").optional() // Optional (nullable) relation
    .apply()?;
```

---

## Query API

Build type-safe queries with the fluent Query builder.

### Basic Query

```rust
// Fetch all users
let users = db.query("User").execute()?;

for user in &users {
    println!("User: {}", user.get_string("name").unwrap_or_default());
}
```

### Filtering

```rust
// Equality filter
let active_users = db.query("User")
    .filter("status", "active")
    .execute()?;

// Comparison filters
let adults = db.query("User")
    .filter_gte("age", 18)
    .execute()?;

// Multiple filters (AND)
let results = db.query("User")
    .filter("status", "active")
    .filter_gte("age", 18)
    .filter_lt("age", 65)
    .execute()?;
```

### Filter Methods

| Method | SQL Equivalent | Example |
|--------|----------------|---------|
| `.filter(field, value)` | `= value` | `.filter("name", "Alice")` |
| `.filter_ne(field, value)` | `!= value` | `.filter_ne("status", "deleted")` |
| `.filter_gt(field, value)` | `> value` | `.filter_gt("age", 18)` |
| `.filter_gte(field, value)` | `>= value` | `.filter_gte("score", 100)` |
| `.filter_lt(field, value)` | `< value` | `.filter_lt("price", 50)` |
| `.filter_lte(field, value)` | `<= value` | `.filter_lte("quantity", 10)` |
| `.filter_null(field)` | `IS NULL` | `.filter_null("deleted_at")` |
| `.filter_not_null(field)` | `IS NOT NULL` | `.filter_not_null("email")` |
| `.filter_like(field, pattern)` | `LIKE pattern` | `.filter_like("email", "%@gmail.com")` |
| `.filter_in(field, values)` | `IN (...)` | `.filter_in("status", &["active", "pending"])` |
| `.filter_expr(expr)` | Custom | See below |

### Complex Filter Expressions

```rust
use ormdb_proto::{FilterExpr, Value};

// OR expression
let results = db.query("User")
    .filter_expr(FilterExpr::or(
        FilterExpr::eq("role", Value::String("admin".into())),
        FilterExpr::eq("role", Value::String("moderator".into()))
    ))
    .execute()?;

// Nested expressions
let results = db.query("Order")
    .filter_expr(FilterExpr::and(
        FilterExpr::gte("total", Value::Float64(100.0)),
        FilterExpr::or(
            FilterExpr::eq("status", Value::String("pending".into())),
            FilterExpr::eq("status", Value::String("processing".into()))
        )
    ))
    .execute()?;
```

### Including Relations

```rust
// Include related entities
let users = db.query("User")
    .include("posts")
    .execute()?;

for user in &users {
    println!("User: {}", user.get_string("name").unwrap_or_default());
    for post in user.relation("posts") {
        println!("  - {}", post.get_string("title").unwrap_or_default());
    }
}

// Filtered includes
let users = db.query("User")
    .include_filtered("posts", |q| {
        q.filter("published", true)
         .order_by("created_at")
         .limit(5)
    })
    .execute()?;

// Nested includes
let users = db.query("User")
    .include_filtered("posts", |q| {
        q.include("comments")
    })
    .execute()?;
```

### Selecting Fields

```rust
// Select specific fields only
let users = db.query("User")
    .select(&["id", "name", "email"])
    .execute()?;
```

### Ordering

```rust
// Ascending order (default)
let users = db.query("User")
    .order_by("name")
    .execute()?;

// Descending order
let users = db.query("User")
    .order_by_desc("created_at")
    .execute()?;
```

### Pagination

```rust
// Limit results
let users = db.query("User")
    .limit(10)
    .execute()?;

// Offset pagination
let page_2 = db.query("User")
    .order_by("name")
    .limit(10)
    .offset(10)
    .execute()?;
```

### Execution Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `.execute()` | `Result<Vec<Entity>>` | Execute and return all matching entities |
| `.first()` | `Result<Option<Entity>>` | Return first match or None |
| `.count()` | `Result<u64>` | Return count of matching entities |

```rust
// Get first match
let admin = db.query("User")
    .filter("role", "admin")
    .first()?;

if let Some(user) = admin {
    println!("Admin: {}", user.get_string("name").unwrap_or_default());
}

// Count matches
let active_count = db.query("User")
    .filter("status", "active")
    .count()?;

println!("Active users: {}", active_count);
```

---

## Mutation API

### Insert

```rust
// Insert returns the generated UUID
let user_id = db.insert("User")
    .set("name", "Alice")
    .set("email", "alice@example.com")
    .set("age", 30)
    .execute()?;

println!("Created user: {:02x?}", user_id);

// Insert with specific ID
let specific_id: [u8; 16] = /* your UUID */;
db.insert("User")
    .set("name", "Bob")
    .set("email", "bob@example.com")
    .execute_with_id(specific_id)?;
```

### Update

```rust
// Update by ID - returns true if found and updated
let updated = db.update("User", user_id)
    .set("name", "Alice Smith")
    .set("age", 31)
    .execute()?;

if updated {
    println!("User updated");
} else {
    println!("User not found");
}
```

### Delete

```rust
// Delete by ID - returns true if found and deleted
let deleted = db.delete("User", user_id)
    .execute()?;

if deleted {
    println!("User deleted");
}
```

!!! note "Soft Deletes"
    Deletes are implemented as soft deletes (tombstones) to support MVCC.
    Old versions are cleaned up based on the retention policy.

---

## Transactions

ORMDB Embedded uses **Optimistic Concurrency Control (OCC)** for transactions. Multiple transactions can run concurrently, with conflicts detected at commit time.

### Closure-Based Transactions

```rust
db.with_transaction(|tx| {
    // Read user
    let user = tx.read("User", user_id)?
        .ok_or_else(|| ormdb::Error::NotFound("User not found".into()))?;

    let current_balance = user.get_i64("balance").unwrap_or(0);

    // Update within transaction
    tx.update("User", user_id)
        .set("balance", current_balance + 100)
        .execute()?;

    // Insert audit log
    tx.insert("AuditLog")
        .set("user_id", user_id)
        .set("action", "credit")
        .set("amount", 100)
        .execute()?;

    Ok(())
})?;
```

### Manual Transaction Control

```rust
let mut tx = db.transaction();

tx.insert("User")
    .set("name", "Charlie")
    .set("email", "charlie@example.com")
    .execute()?;

tx.insert("User")
    .set("name", "Diana")
    .set("email", "diana@example.com")
    .execute()?;

// Explicitly commit
tx.commit()?;

// Or let it auto-rollback on drop
```

### Handling Conflicts

```rust
use ormdb::Error;

match db.with_transaction(|tx| {
    // ... transaction operations
    Ok(())
}) {
    Ok(_) => println!("Transaction committed"),
    Err(Error::TransactionConflict { entity_id, expected_version, actual_version }) => {
        println!(
            "Conflict on {:02x?}: expected v{}, found v{}",
            entity_id, expected_version, actual_version
        );
        // Retry logic here
    }
    Err(e) => return Err(e),
}
```

### Transaction Methods

| Method | Description |
|--------|-------------|
| `tx.insert(entity)` | Insert within transaction |
| `tx.update(entity, id)` | Update within transaction |
| `tx.read(entity, id)` | Read with version tracking |
| `tx.exists(entity, id)` | Check existence with version tracking |
| `tx.commit()` | Commit all changes atomically |

!!! warning "Conflict Detection"
    A `TransactionConflict` error occurs when another transaction modified an entity
    that your transaction read or modified. The solution is typically to retry the
    entire transaction.

---

## Working with Entities

The `Entity` struct provides type-safe accessors for field values.

### Field Accessors

```rust
let user = db.query("User").first()?.unwrap();

// Get field with Option
let name: Option<&str> = user.get_string("name");
let age: Option<i32> = user.get_i32("age");
let balance: Option<i64> = user.get_i64("balance");
let score: Option<f64> = user.get_f64("score");
let active: Option<bool> = user.get_bool("active");
let id: Option<&[u8]> = user.get_uuid("id");

// Generic getter
use ormdb_proto::Value;
let value: Option<&Value> = user.get("name");
```

### Entity Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `entity.id()` | `[u8; 16]` | Entity UUID as bytes |
| `entity.id_hex()` | `String` | Entity UUID as hex string |
| `entity.get(field)` | `Option<&Value>` | Get raw value |
| `entity.get_string(field)` | `Option<&str>` | Get as string |
| `entity.get_i32(field)` | `Option<i32>` | Get as i32 |
| `entity.get_i64(field)` | `Option<i64>` | Get as i64 |
| `entity.get_f64(field)` | `Option<f64>` | Get as f64 |
| `entity.get_bool(field)` | `Option<bool>` | Get as bool |
| `entity.get_uuid(field)` | `Option<&[u8]>` | Get as UUID bytes |
| `entity.relation(name)` | `&[Entity]` | Get related entities |
| `entity.has(field)` | `bool` | Check if field is non-null |
| `entity.field_names()` | `impl Iterator<Item=&str>` | Iterate field names |

---

## Error Handling

```rust
use ormdb::Error;

match db.insert("User").set("email", "duplicate@example.com").execute() {
    Ok(id) => println!("Created: {:02x?}", id),

    Err(Error::UniqueViolation { entity, field, value }) => {
        println!("Duplicate {}.{} = {:?}", entity, field, value);
    }

    Err(Error::ForeignKeyViolation { entity, field, referenced_entity }) => {
        println!("Invalid reference: {}.{} -> {}", entity, field, referenced_entity);
    }

    Err(Error::ConstraintViolation(msg)) => {
        println!("Constraint violation: {}", msg);
    }

    Err(Error::NotFound(msg)) => {
        println!("Not found: {}", msg);
    }

    Err(Error::TransactionConflict { entity_id, .. }) => {
        println!("Conflict on {:02x?}", entity_id);
    }

    Err(e) => println!("Error: {}", e),
}
```

### Error Variants

| Variant | Description |
|---------|-------------|
| `Database(String)` | General database error |
| `Storage(String)` | Storage layer error |
| `Schema(String)` | Schema validation error |
| `Query(String)` | Query execution error |
| `NotFound(String)` | Entity not found |
| `InvalidArgument(String)` | Invalid argument provided |
| `Transaction(String)` | Transaction error |
| `TransactionConflict { entity_id, expected_version, actual_version }` | OCC conflict |
| `ConstraintViolation(String)` | Generic constraint violation |
| `UniqueViolation { entity, field, value }` | Unique constraint violation |
| `ForeignKeyViolation { entity, field, referenced_entity }` | Foreign key violation |
| `Serialization(String)` | Serialization error |
| `Internal(String)` | Internal error |

---

## Database Maintenance

### Flushing to Disk

```rust
// Force pending writes to disk
db.flush()?;
```

### Compaction

```rust
// Reclaim space from old MVCC versions
db.compact()?;
```

### Statistics

```rust
// Refresh query optimizer statistics
db.refresh_statistics()?;
```

---

## Multi-threaded Usage

The `Database` handle uses `Arc` internally and is cheap to clone. Share it across threads freely.

```rust
use std::thread;

let db = Database::open("./my_data")?;

let handles: Vec<_> = (0..10).map(|i| {
    let db = db.clone();  // Cheap clone
    thread::spawn(move || {
        db.insert("Task")
            .set("name", format!("Task {}", i))
            .execute()
    })
}).collect();

for handle in handles {
    handle.join().unwrap()?;
}
```

---

## Complete Example

```rust
use ormdb::{Database, ScalarType, Error};

fn main() -> ormdb::Result<()> {
    // Open database
    let db = Database::open("./blog_data")?;

    // Define schema
    db.schema()
        .entity("Author")
            .field("id", ScalarType::Uuid).primary_key()
            .field("name", ScalarType::String)
            .field("email", ScalarType::String).unique()
        .entity("Post")
            .field("id", ScalarType::Uuid).primary_key()
            .field("title", ScalarType::String)
            .field("content", ScalarType::String)
            .field("published", ScalarType::Bool)
            .relation("author", "Author").required()
        .entity("Comment")
            .field("id", ScalarType::Uuid).primary_key()
            .field("content", ScalarType::String)
            .relation("post", "Post").required()
            .relation("author", "Author").required()
        .apply()?;

    // Create an author
    let author_id = db.insert("Author")
        .set("name", "Jane Doe")
        .set("email", "jane@example.com")
        .execute()?;

    // Create a post
    let post_id = db.insert("Post")
        .set("title", "Hello, ORMDB!")
        .set("content", "This is my first post using embedded ORMDB.")
        .set("published", true)
        .set("author", author_id)
        .execute()?;

    // Create comments
    for i in 1..=3 {
        db.insert("Comment")
            .set("content", format!("Comment #{}", i))
            .set("post", post_id)
            .set("author", author_id)
            .execute()?;
    }

    // Query with includes
    let posts = db.query("Post")
        .filter("published", true)
        .include("author")
        .include("comments")
        .order_by_desc("title")
        .execute()?;

    for post in &posts {
        let author_name = post.relation("author")
            .first()
            .and_then(|a| a.get_string("name"))
            .unwrap_or("Unknown");

        println!("'{}' by {}",
            post.get_string("title").unwrap_or_default(),
            author_name
        );

        let comment_count = post.relation("comments").len();
        println!("  {} comments", comment_count);
    }

    // Transaction example
    db.with_transaction(|tx| {
        let post = tx.read("Post", post_id)?
            .ok_or_else(|| Error::NotFound("Post".into()))?;

        if post.get_bool("published") == Some(true) {
            tx.update("Post", post_id)
                .set("published", false)
                .execute()?;
        }
        Ok(())
    })?;

    // Cleanup
    db.flush()?;

    Ok(())
}
```

---

## Best Practices

1. **Reuse the Database handle** - It's cheap to clone and thread-safe
2. **Use in-memory databases for tests** - Fast and isolated
3. **Set appropriate cache sizes** - Larger caches improve read performance
4. **Call `compact()` periodically** - Reclaim space from deleted/updated records
5. **Handle `TransactionConflict`** - Implement retry logic for concurrent workloads
6. **Use transactions for multi-entity operations** - Ensures consistency
7. **Select only needed fields** - Reduces memory usage for large entities
8. **Use filtered includes** - Avoid loading unnecessary related data

---

## Next Steps

- **[CLI Reference](../reference/cli.md)** - Use embedded mode from the command line
- **[Rust Client](../clients/rust.md)** - Client mode for distributed deployments
- **[Transactions Tutorial](../tutorials/transactions.md)** - Deep dive into transactions
- **[Performance Guide](performance.md)** - Optimization strategies
