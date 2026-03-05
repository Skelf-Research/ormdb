# Client Libraries

ORMDB provides official client libraries for Rust, TypeScript, and Python, plus an embedded mode for Rust applications.

## Deployment Modes

| Mode | Description | Languages |
|------|-------------|-----------|
| **Embedded** | In-process database, no server required | Rust only |
| **Client** | Connect to remote ORMDB server | Rust, TypeScript, Python |

## Available Clients

| Language | Package | Transport | Embedded | ORM Adapters |
|----------|---------|-----------|----------|--------------|
| **Rust** | `ormdb` / `ormdb-client` | Native (nng) | Yes | - |
| **TypeScript** | `@ormdb/client` | HTTP/JSON | - | Prisma, Drizzle, TypeORM, Kysely, Sequelize |
| **Python** | `ormdb` | HTTP/JSON | - | SQLAlchemy, Django |

## Client Comparison

| Feature | Rust (Embedded) | Rust (Client) | TypeScript | Python |
|---------|-----------------|---------------|------------|--------|
| Async support | Optional | Required | Yes | Yes |
| Connection pooling | N/A | Yes | Yes | Yes |
| Type safety | Yes | Yes | Yes | - |
| Zero-copy serialization | Yes | Yes | - | - |
| Transactions (OCC) | Yes | Yes | Yes | Yes |
| Streaming (CDC) | Planned | Yes | Yes | Yes |
| Schema definition | Yes | - | - | - |

## Quick Start

=== "Rust (Embedded)"

    ```bash
    cargo add ormdb
    ```

    ```rust
    use ormdb::{Database, ScalarType};

    let db = Database::open("./my_data")?;

    db.schema()
        .entity("User")
            .field("id", ScalarType::Uuid).primary_key()
            .field("name", ScalarType::String)
        .apply()?;

    let user_id = db.insert("User")
        .set("name", "Alice")
        .execute()?;

    let users = db.query("User").execute()?;
    ```

=== "Rust (Client)"

    ```bash
    cargo add ormdb-client ormdb-proto
    ```

    ```rust
    use ormdb_client::{Client, ClientConfig};
    use ormdb_proto::GraphQuery;

    let client = Client::connect(ClientConfig::localhost()).await?;
    let result = client.query(GraphQuery::new("User")).await?;
    ```

=== "TypeScript"

    ```bash
    npm install @ormdb/client
    ```

    ```typescript
    import { OrmdbClient } from "@ormdb/client";

    const client = new OrmdbClient("http://localhost:8080");
    const result = await client.query("User");
    ```

=== "Python"

    ```bash
    pip install ormdb
    ```

    ```python
    from ormdb import OrmdbClient

    client = OrmdbClient("http://localhost:8080")
    result = client.query("User")
    ```

## Detailed Guides

- **[Rust Client](rust.md)** - Embedded and client modes for Rust
- **[TypeScript Client](typescript.md)** - TypeScript/JavaScript with ORM adapters
- **[Python Client](python.md)** - Python with SQLAlchemy and Django support
- **[Embedded Mode Guide](../guides/embedded-mode.md)** - Complete embedded mode reference
