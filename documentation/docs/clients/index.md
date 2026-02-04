# Client Libraries

ORMDB provides official client libraries for Rust, TypeScript, and Python.

## Available Clients

| Language | Package | Transport | ORM Adapters |
|----------|---------|-----------|--------------|
| **Rust** | `ormdb-client` | Native (nng) | - |
| **TypeScript** | `@ormdb/client` | HTTP/JSON | Prisma, Drizzle, TypeORM, Kysely, Sequelize |
| **Python** | `ormdb` | HTTP/JSON | SQLAlchemy, Django |

## Client Comparison

| Feature | Rust | TypeScript | Python |
|---------|------|------------|--------|
| Async support | ✅ | ✅ | ✅ |
| Connection pooling | ✅ | ✅ | ✅ |
| Type safety | ✅ | ✅ | ❌ |
| Zero-copy serialization | ✅ | ❌ | ❌ |
| Streaming (CDC) | ✅ | ✅ | ✅ |

## Quick Start

=== "Rust"

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

- **[Rust Client](rust.md)** - Native Rust client with full protocol support
- **[TypeScript Client](typescript.md)** - TypeScript/JavaScript with ORM adapters
- **[Python Client](python.md)** - Python with SQLAlchemy and Django support
