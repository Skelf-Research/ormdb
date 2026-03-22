# ormdb-client

[![Crates.io](https://img.shields.io/crates/v/ormdb-client.svg)](https://crates.io/crates/ormdb-client)
[![Documentation](https://docs.rs/ormdb-client/badge.svg)](https://docs.rs/ormdb-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Async Rust client library for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-client` provides an ergonomic async API for connecting to and querying ORMDB servers. Features include:

- **Async/Await** - Built on Tokio for async operations
- **Connection Pooling** - Efficient connection management
- **Type-Safe Queries** - Leverages `ormdb-proto` for typed queries
- **Zero-Copy** - Minimal serialization overhead with `rkyv`

## Installation

```toml
[dependencies]
ormdb-client = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use ormdb_client::Client;
use ormdb_proto::{Query, Filter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to ORMDB server
    let client = Client::connect("ormdb://localhost:5432").await?;

    // Execute a graph fetch
    let users = client.fetch(
        Query::new("User")
            .filter(Filter::eq("active", true))
            .include("posts")
    ).await?;

    // Create a new record
    let user = client.create("User", json!({
        "name": "Alice",
        "email": "alice@example.com"
    })).await?;

    // Update records
    let updated = client.update("User")
        .filter(Filter::eq("id", user.id))
        .set(json!({ "verified": true }))
        .execute()
        .await?;

    // Transactions
    let tx = client.begin().await?;
    tx.create("Post", json!({ "title": "Hello", "authorId": user.id })).await?;
    tx.commit().await?;

    Ok(())
}
```

## API Reference

### Client

| Method | Description |
|--------|-------------|
| `connect(url)` | Connect to an ORMDB server |
| `fetch(query)` | Execute a graph fetch query |
| `create(entity, data)` | Create a new record |
| `update(entity)` | Start an update builder |
| `delete(entity)` | Start a delete builder |
| `begin()` | Start a transaction |

### Transaction

| Method | Description |
|--------|-------------|
| `fetch/create/update/delete` | Same as Client |
| `commit()` | Commit the transaction |
| `rollback()` | Rollback the transaction |

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
