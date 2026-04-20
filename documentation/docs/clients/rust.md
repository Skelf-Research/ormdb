# Rust Client

ORMDB provides two deployment modes for Rust applications: **Embedded** (in-process) and **Client** (remote server).

## Choosing a Deployment Mode

| Consideration | Embedded | Client |
|---------------|----------|--------|
| **Architecture** | Single process | Client/server |
| **Network** | No overhead | HTTP/TCP |
| **Scaling** | Single machine | Distributed |
| **Use case** | CLI tools, desktop apps, microservices | Multi-service, polyglot stacks |
| **Startup** | Instant | Requires server |

---

## Embedded Mode

Run ORMDB directly in your application without a separate server.

### Installation

```toml
[dependencies]
ormdb = "0.1"
```

### Quick Start

```rust
use ormdb::{Database, ScalarType};

fn main() -> ormdb::Result<()> {
    // Open database (file-based or in-memory)
    let db = Database::open("./my_data")?;
    // let db = Database::open_memory()?;  // For testing

    // Define schema
    db.schema()
        .entity("User")
            .field("id", ScalarType::Uuid).primary_key()
            .field("name", ScalarType::String)
            .field("email", ScalarType::String).unique()
        .entity("Post")
            .field("id", ScalarType::Uuid).primary_key()
            .field("title", ScalarType::String)
            .relation("author", "User").required()
        .apply()?;

    // Insert
    let user_id = db.insert("User")
        .set("name", "Alice")
        .set("email", "alice@example.com")
        .execute()?;

    // Query
    let users = db.query("User")
        .filter("name", "Alice")
        .include("posts")
        .execute()?;

    for user in &users {
        println!("User: {}", user.get_string("name").unwrap_or_default());
    }

    // Update
    db.update("User", user_id)
        .set("name", "Alice Smith")
        .execute()?;

    // Delete
    db.delete("User", user_id).execute()?;

    Ok(())
}
```

### Transactions

```rust
use ormdb::Error;

db.with_transaction(|tx| {
    tx.insert("User").set("name", "Bob").execute()?;
    tx.insert("User").set("name", "Carol").execute()?;
    Ok(())
})?;

// Handle conflicts
match db.with_transaction(|tx| {
    let user = tx.read("User", user_id)?
        .ok_or_else(|| Error::NotFound("User not found".into()))?;
    // ... modify user
    Ok(())
}) {
    Ok(_) => println!("Committed"),
    Err(Error::TransactionConflict { .. }) => println!("Conflict - retry"),
    Err(e) => return Err(e),
}
```

### Query Builder

```rust
// Filtering
let results = db.query("User")
    .filter("status", "active")
    .filter_gte("age", 18)
    .filter_like("email", "%@company.com")
    .filter_in("role", &["admin", "moderator"])
    .execute()?;

// Relations
let users = db.query("User")
    .include("posts")
    .include_filtered("comments", |q| {
        q.filter("approved", true).limit(10)
    })
    .execute()?;

// Pagination
let page = db.query("User")
    .order_by("name")
    .limit(20)
    .offset(40)
    .execute()?;

// Count
let count = db.query("User")
    .filter("active", true)
    .count()?;
```

### Multi-threaded Usage

```rust
use std::thread;

let db = Database::open("./data")?;

let handles: Vec<_> = (0..10).map(|i| {
    let db = db.clone();  // Cheap Arc-based clone
    thread::spawn(move || {
        db.insert("Task")
            .set("name", format!("Task {}", i))
            .execute()
    })
}).collect();

for h in handles {
    h.join().unwrap()?;
}
```

For complete embedded mode documentation, see the **[Embedded Mode Guide](../guides/embedded-mode.md)**.

---

## Client Mode

Connect to a remote ORMDB server.

### Installation

```toml
[dependencies]
ormdb-client = "0.1"
ormdb-proto = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Connection

#### Basic Connection

```rust
use ormdb_client::{Client, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to localhost
    let client = Client::connect(ClientConfig::localhost()).await?;

    // Ping to check connection
    client.ping().await?;

    println!("Connected!");
    Ok(())
}
```

#### Custom Configuration

```rust
use std::time::Duration;

let config = ClientConfig::new("tcp://192.168.1.100:9000")
    .with_timeout(Duration::from_secs(60))
    .with_max_message_size(128 * 1024 * 1024)  // 128MB
    .with_client_id("my-app-1");

let client = Client::connect(config).await?;
```

#### Connection Pool

```rust
use ormdb_client::{ConnectionPool, PoolConfig};

let pool_config = PoolConfig::new()
    .with_min_connections(5)
    .with_max_connections(20);

let pool = ConnectionPool::new(ClientConfig::localhost(), pool_config).await?;

// Get a connection from the pool
let conn = pool.get().await?;
let result = conn.query(query).await?;
// Connection returned to pool when dropped
```

### Queries

#### Basic Query

```rust
use ormdb_proto::GraphQuery;

let query = GraphQuery::new("User");
let result = client.query(query).await?;

println!("Found {} users", result.total_entities());
```

#### With Fields and Filters

```rust
use ormdb_proto::{GraphQuery, FilterExpr, Value, OrderSpec, Pagination};

let query = GraphQuery::new("User")
    .with_fields(vec!["id", "name", "email"])
    .with_filter(FilterExpr::eq("status", Value::String("active".into())))
    .with_order(OrderSpec::asc("name"))
    .with_pagination(Pagination::new(10, 0));

let result = client.query(query).await?;
```

#### With Includes

```rust
use ormdb_proto::RelationInclude;

let query = GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .with_fields(vec!["id", "title"])
        .with_filter(FilterExpr::eq("published", Value::Bool(true))));

let result = client.query(query).await?;

for user in result.entities("User") {
    println!("User: {}", user.get_string("name")?);
    for post in result.related(&user, "posts") {
        println!("  - {}", post.get_string("title")?);
    }
}
```

### Mutations

#### Insert

```rust
use ormdb_proto::{Mutation, Value};

let mutation = Mutation::insert("User")
    .with_field("name", Value::String("Alice".into()))
    .with_field("email", Value::String("alice@example.com".into()));

let result = client.mutate(mutation).await?;
let user_id = result.inserted_id();
```

#### Update

```rust
let mutation = Mutation::update("User", user_id)
    .with_field("name", Value::String("Alice Smith".into()));

client.mutate(mutation).await?;
```

#### Delete

```rust
let mutation = Mutation::delete("User", user_id);
client.mutate(mutation).await?;
```

#### Batch Mutations

```rust
use ormdb_proto::MutationBatch;

let batch = MutationBatch::new()
    .add(Mutation::insert("User").with_field("name", Value::String("Alice".into())))
    .add(Mutation::insert("User").with_field("name", Value::String("Bob".into())));

let results = client.mutate_batch(batch).await?;
```

### Aggregates

```rust
use ormdb_proto::{AggregateQuery, AggregateFunction};

let query = AggregateQuery::new("User")
    .count()
    .sum("age")
    .avg("age")
    .with_filter(FilterExpr::eq("status", Value::String("active".into())));

let result = client.aggregate(query).await?;

println!("Count: {}", result.get_count()?);
println!("Sum of ages: {}", result.get_sum("age")?);
println!("Average age: {}", result.get_avg("age")?);
```

### Error Handling

```rust
use ormdb_client::Error;

match client.mutate(mutation).await {
    Ok(result) => println!("Success: {:?}", result),
    Err(Error::Connection(e)) => println!("Connection error: {}", e),
    Err(Error::Timeout) => println!("Request timed out"),
    Err(Error::Protocol(e)) => println!("Protocol error: {}", e),
    Err(Error::ConstraintViolation(e)) => println!("Constraint: {:?}", e),
    Err(e) => println!("Error: {}", e),
}
```

### Change Streams (CDC)

```rust
// Subscribe to changes
let mut stream = client.stream_changes(StreamOptions {
    entities: Some(vec!["User".into(), "Post".into()]),
    from_lsn: 0,
}).await?;

while let Some(change) = stream.next().await {
    match change? {
        ChangeEvent::Insert { entity, id, fields } => {
            println!("Inserted {} {}", entity, id);
        }
        ChangeEvent::Update { entity, id, before, after } => {
            println!("Updated {} {}", entity, id);
        }
        ChangeEvent::Delete { entity, id } => {
            println!("Deleted {} {}", entity, id);
        }
    }
}
```

---

## Best Practices

### Embedded Mode

1. **Reuse the Database handle** - It's cheap to clone via Arc
2. **Use in-memory databases for tests** - Fast and isolated
3. **Call `compact()` periodically** - Reclaims space from old versions
4. **Handle `TransactionConflict`** - Implement retry logic
5. **Set appropriate cache sizes** - Larger caches improve reads

### Client Mode

1. **Use connection pools** for concurrent applications
2. **Reuse clients** - creating connections is expensive
3. **Set appropriate timeouts** for your workload
4. **Handle errors gracefully** - network issues happen
5. **Use batch mutations** for bulk operations

---

## Next Steps

- **[Embedded Mode Guide](../guides/embedded-mode.md)** - Complete embedded API reference
- **[Query API Reference](../reference/query-api.md)** - Full query protocol documentation
- **[Error Reference](../reference/errors.md)** - All error types
- **[CLI Reference](../reference/cli.md)** - Command-line usage
