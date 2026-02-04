# ormdb-core

[![Crates.io](https://img.shields.io/crates/v/ormdb-core.svg)](https://crates.io/crates/ormdb-core)
[![Documentation](https://docs.rs/ormdb-core/badge.svg)](https://docs.rs/ormdb-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Core storage engine, query executor, and catalog for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-core` is the heart of ORMDB, providing:

- **Storage Engine** - MVCC-based storage with B-tree and hash indexes
- **Query Executor** - Graph-aware query planning and execution
- **Catalog** - Entity, relation, and constraint management
- **Migration Engine** - Online-safe schema migrations
- **Replication** - Change data capture and streaming

## Features

- Zero-copy serialization with `rkyv`
- Embedded storage via `sled`
- ACID transactions with snapshot isolation
- Constraint validation (unique, foreign key, check)
- Row and field-level security

## Usage

```rust
use ormdb_core::{Storage, Catalog, QueryExecutor};

// Initialize storage
let storage = Storage::open("./data")?;

// Create a catalog
let catalog = Catalog::new(&storage)?;

// Execute queries
let executor = QueryExecutor::new(&storage, &catalog);
let results = executor.execute(query)?;
```

## Architecture

```
ormdb-core/
├── src/
│   ├── storage/      # Storage engine, indexes, transactions
│   ├── query/        # Query planner and executor
│   ├── catalog/      # Schema and metadata management
│   ├── migration/    # Migration engine
│   ├── constraint/   # Constraint validation
│   ├── security/     # RLS and audit logging
│   └── replication/  # CDC and change streams
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
