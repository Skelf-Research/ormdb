# ormdb-server

[![Crates.io](https://img.shields.io/crates/v/ormdb-server.svg)](https://crates.io/crates/ormdb-server)
[![Documentation](https://docs.rs/ormdb-server/badge.svg)](https://docs.rs/ormdb-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Standalone database server for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-server` is the main entry point for running ORMDB as a standalone database service. It handles:

- Client connections via NNG (nanomsg-next-gen)
- Request routing and protocol handling
- Mutation processing with cascades
- Change data capture and pub/sub
- Replication streams

## Installation

```bash
# Install from crates.io
cargo install ormdb-server

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release -p ormdb-server
```

## Usage

```bash
# Start the server
ormdb-server --data-dir ./data --port 5432

# With logging
RUST_LOG=info ormdb-server --data-dir ./data

# Show all options
ormdb-server --help
```

## Configuration

| Flag | Description | Default |
|------|-------------|---------|
| `--data-dir` | Data directory path | `./data` |
| `--port` | Server port | `5432` |
| `--host` | Bind address | `127.0.0.1` |

## Connecting

Use the ORMDB client libraries to connect:

**Rust**
```rust
use ormdb_client::Client;

let client = Client::connect("ormdb://localhost:5432").await?;
```

**TypeScript**
```typescript
import { OrmdbClient } from '@ormdb/client';

const client = new OrmdbClient({ url: 'ormdb://localhost:5432' });
```

**Python**
```python
from ormdb import Client

client = Client("ormdb://localhost:5432")
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
