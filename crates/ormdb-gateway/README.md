# ormdb-gateway

[![Crates.io](https://img.shields.io/crates/v/ormdb-gateway.svg)](https://crates.io/crates/ormdb-gateway)
[![Documentation](https://docs.rs/ormdb-gateway/badge.svg)](https://docs.rs/ormdb-gateway)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

HTTP/REST gateway for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-gateway` provides an HTTP API layer for ORMDB, enabling access from any HTTP client. Features include:

- **REST API** - Standard HTTP endpoints for CRUD operations
- **JSON Protocol** - JSON request/response format
- **CORS Support** - Configurable cross-origin requests
- **OpenAPI** - Auto-generated API documentation
- **Authentication** - Pluggable auth middleware

## Installation

```bash
# Install from crates.io
cargo install ormdb-gateway

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release -p ormdb-gateway
```

## Usage

```bash
# Start the gateway
ormdb-gateway --ormdb-url ormdb://localhost:5432 --port 8080

# With CORS enabled
ormdb-gateway --cors-origins "http://localhost:3000"
```

## API Endpoints

### Query

```http
POST /query
Content-Type: application/json

{
  "entity": "User",
  "filter": { "id": { "$eq": 1 } },
  "include": {
    "posts": {
      "include": { "comments": true }
    }
  }
}
```

### Create

```http
POST /entities/User
Content-Type: application/json

{
  "name": "Alice",
  "email": "alice@example.com"
}
```

### Update

```http
PATCH /entities/User/1
Content-Type: application/json

{
  "verified": true
}
```

### Delete

```http
DELETE /entities/User/1
```

### Batch Operations

```http
POST /batch
Content-Type: application/json

{
  "operations": [
    { "op": "create", "entity": "User", "data": {...} },
    { "op": "update", "entity": "Post", "id": 1, "data": {...} }
  ]
}
```

## Configuration

| Flag | Description | Default |
|------|-------------|---------|
| `--ormdb-url` | ORMDB server URL | `ormdb://localhost:5432` |
| `--port` | HTTP port | `8080` |
| `--host` | Bind address | `0.0.0.0` |
| `--cors-origins` | Allowed CORS origins | None |

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
