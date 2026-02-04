# ORMDB

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Build Status](https://github.com/Skelf-Research/ormdb/actions/workflows/ci.yml/badge.svg)](https://github.com/Skelf-Research/ormdb/actions)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Documentation](https://img.shields.io/badge/docs-online-green.svg)](https://docs.skelfresearch.com/ormdb)

**The database that speaks ORM natively.** No more N+1 queries. No more impedance mismatch. Just declare your model graph and let the database do the rest.

---

## Why ORMDB?

Traditional databases force ORMs to translate object graphs into SQL, leading to:
- **N+1 query problems** that kill performance
- **Impedance mismatch** between objects and tables
- **Risky migrations** that require downtime
- **Cache invalidation headaches** with no built-in solution

ORMDB flips this around. The database understands your entities, relations, and constraints natively.

```typescript
// Fetch a user with their posts and comments in ONE round-trip
const user = await db.fetch({
  entity: "User",
  where: { id: userId },
  include: {
    posts: {
      include: { comments: true }
    }
  }
});
```

## Features

- **Graph Fetches** - Fetch entire object graphs in a single query
- **Native Relations** - First-class support for one-to-one, one-to-many, and many-to-many
- **ACID Transactions** - Full relational guarantees with constraints and joins
- **Safe Migrations** - Online schema changes with automatic safety grading
- **Change Streams** - Built-in CDC for cache invalidation and real-time updates
- **ORM Adapters** - Drop-in support for Prisma, Drizzle, TypeORM, SQLAlchemy, and more

## Quick Start

### Installation

```bash
# Install the server
cargo install ormdb-server

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release
```

### Start the Server

```bash
ormdb-server --data-dir ./data --port 5432
```

### Connect with Your Favorite ORM

**TypeScript (Prisma adapter)**
```bash
npm install @ormdb/client
```

```typescript
import { OrmdbClient } from '@ormdb/client';
import { PrismaAdapter } from '@ormdb/client/prisma';

const client = new OrmdbClient({ url: 'ormdb://localhost:5432' });
const prisma = new PrismaAdapter(client);
```

**Python (SQLAlchemy adapter)**
```bash
pip install ormdb[sqlalchemy]
```

```python
from sqlalchemy import create_engine
engine = create_engine("ormdb://localhost:5432/mydb")
```

## Documentation

| Guide | Description |
|-------|-------------|
| [Getting Started](https://docs.skelfresearch.com/ormdb/getting-started/) | Installation and first steps |
| [Core Concepts](https://docs.skelfresearch.com/ormdb/concepts/) | Graph queries, MVCC, storage architecture |
| [Tutorials](https://docs.skelfresearch.com/ormdb/tutorials/) | Schema design, querying, mutations |
| [ORM Adapters](https://docs.skelfresearch.com/ormdb/adapters/) | Prisma, Drizzle, TypeORM, SQLAlchemy, Django |
| [API Reference](https://docs.skelfresearch.com/ormdb/reference/) | Query API, mutations, configuration |

### Architecture Docs

- [`docs/architecture.md`](docs/architecture.md) - System layers and execution paths
- [`docs/modeling.md`](docs/modeling.md) - Schema model, relations, and constraints
- [`docs/querying.md`](docs/querying.md) - Graph fetch semantics and output shapes
- [`docs/migrations.md`](docs/migrations.md) - Online-safe migration engine
- [`docs/security.md`](docs/security.md) - RLS, field-level controls, auditing

## Project Structure

```
ormdb/
├── crates/
│   ├── ormdb-core/     # Storage engine, query executor, catalog
│   ├── ormdb-server/   # Standalone database server
│   ├── ormdb-proto/    # Wire protocol (rkyv zero-copy)
│   ├── ormdb-lang/     # Query language parser
│   ├── ormdb-client/   # Rust client library
│   ├── ormdb-cli/      # Command-line interface
│   └── ormdb-gateway/  # HTTP/REST gateway
├── clients/
│   ├── typescript/     # @ormdb/client + ORM adapters
│   └── python/         # Python client + SQLAlchemy/Django
└── docs/               # Architecture documentation
```

## Benchmarks

ORMDB is designed to outperform traditional ORM+database stacks on graph-shaped queries.

```bash
# Run benchmarks
cargo bench

# Compare against SQLite and PostgreSQL
./scripts/run-comparison.sh
```

See [`docs/benchmarks.md`](docs/benchmarks.md) for methodology and results.

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

```bash
# Development setup
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build
cargo test
```

## Roadmap

See [`docs/roadmap.md`](docs/roadmap.md) for implementation phases and milestones.

**Current Status:** Alpha (v0.1.0)

## Community

- [GitHub Issues](https://github.com/Skelf-Research/ormdb/issues) - Bug reports and feature requests
- [Documentation](https://docs.skelfresearch.com/ormdb) - Full documentation site

## License

This project is licensed under the [MIT License](LICENSE).

---

Created by [Dipankar Sarkar](mailto:me@dipankar.name) at [Skelf Research](https://skelfresearch.com)
