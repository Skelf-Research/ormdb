# ormdb-proto

[![Crates.io](https://img.shields.io/crates/v/ormdb-proto.svg)](https://crates.io/crates/ormdb-proto)
[![Documentation](https://docs.rs/ormdb-proto/badge.svg)](https://docs.rs/ormdb-proto)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Wire protocol definitions for [ORMDB](https://github.com/Skelf-Research/ormdb) with zero-copy serialization.

## Overview

`ormdb-proto` defines the binary protocol used for client-server communication in ORMDB. It provides:

- **Query IR** - Typed intermediate representation for graph queries
- **Mutation IR** - Structured mutation operations
- **Result Types** - Typed response structures
- **Zero-Copy Serialization** - Using `rkyv` for minimal overhead

## Features

- Zero-copy deserialization with `rkyv`
- Strongly typed query and mutation structures
- Efficient binary encoding
- Serde support for JSON interop

## Usage

```rust
use ormdb_proto::{Query, Filter, Include};

// Build a typed query
let query = Query::new("User")
    .filter(Filter::eq("id", 1))
    .include(Include::relation("posts"));

// Serialize for wire transport
let bytes = query.to_bytes()?;

// Zero-copy deserialization
let archived = Query::from_bytes(&bytes)?;
```

## Protocol Structure

```
Request
├── Query
│   ├── entity: String
│   ├── filter: Filter
│   ├── include: Vec<Include>
│   └── options: QueryOptions
└── Mutation
    ├── Create { entity, data }
    ├── Update { entity, filter, data }
    └── Delete { entity, filter }

Response
├── QueryResult { rows, shape }
├── MutationResult { affected, returning }
└── Error { code, message }
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
