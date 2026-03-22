# Configuration Reference

Complete reference for ORMDB server and client configuration.

## Server Configuration

### Configuration File

Default location: `ormdb.toml`

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000
request_timeout_ms = 30000

[storage]
path = "./data"
cache_size_mb = 256
sync_mode = "normal"
compression = true

[query]
max_entities = 10000
max_edges = 50000
max_depth = 5
default_limit = 100

[security]
enable_rls = true
enable_field_masking = true
capability_check = true

[logging]
level = "info"
format = "json"
```

---

## Server Options

### [server]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `host` | string | `"0.0.0.0"` | Bind address |
| `port` | integer | `8080` | Listen port |
| `max_connections` | integer | `1000` | Maximum concurrent connections |
| `request_timeout_ms` | integer | `30000` | Request timeout in milliseconds |
| `worker_threads` | integer | CPU cores | Number of worker threads |
| `enable_http2` | boolean | `true` | Enable HTTP/2 support |

### [storage]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `path` | string | `"./data"` | Data directory path |
| `cache_size_mb` | integer | `256` | Page cache size in MB |
| `sync_mode` | string | `"normal"` | Sync mode (see below) |
| `compression` | boolean | `true` | Enable data compression |
| `max_log_size_mb` | integer | `64` | Max WAL segment size |
| `checkpoint_interval_ms` | integer | `60000` | Checkpoint interval |

**Sync Modes:**

| Mode | Description | Durability |
|------|-------------|------------|
| `fast` | Async writes, fastest | May lose recent writes on crash |
| `normal` | Sync on commit | Durable on commit |
| `paranoid` | Sync every write | Maximum durability, slowest |

### [query]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_entities` | integer | `10000` | Max entities per query |
| `max_edges` | integer | `50000` | Max edges (relations) per query |
| `max_depth` | integer | `5` | Max include depth |
| `default_limit` | integer | `100` | Default pagination limit |
| `timeout_ms` | integer | `10000` | Query timeout |
| `parallel_scan` | boolean | `true` | Enable parallel scanning |

### [security]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable_rls` | boolean | `false` | Enable row-level security |
| `enable_field_masking` | boolean | `false` | Enable field masking |
| `capability_check` | boolean | `true` | Enforce capability tokens |
| `max_token_age_seconds` | integer | `3600` | Max capability token age |

### [logging]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `level` | string | `"info"` | Log level (trace, debug, info, warn, error) |
| `format` | string | `"json"` | Log format (json, pretty, compact) |
| `file` | string | none | Log file path (stdout if not set) |
| `max_size_mb` | integer | `100` | Max log file size |
| `max_files` | integer | `5` | Max rotated log files |

### [metrics]

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable Prometheus metrics |
| `port` | integer | `9090` | Metrics endpoint port |
| `path` | string | `"/metrics"` | Metrics endpoint path |

---

## Environment Variables

All configuration options can be set via environment variables:

```bash
# Server
ORMDB_SERVER_HOST=0.0.0.0
ORMDB_SERVER_PORT=8080

# Storage
ORMDB_STORAGE_PATH=/var/lib/ormdb
ORMDB_STORAGE_CACHE_SIZE_MB=512

# Query
ORMDB_QUERY_MAX_ENTITIES=20000

# Security
ORMDB_SECURITY_ENABLE_RLS=true

# Logging
ORMDB_LOGGING_LEVEL=debug
```

Environment variables take precedence over config file values.

---

## Client Configuration

### Rust Client

```rust
use ormdb_client::{Client, ClientConfig};

let config = ClientConfig {
    base_url: "http://localhost:8080".into(),
    timeout: Duration::from_secs(30),
    max_connections: 10,
    retry_config: RetryConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        exponential_base: 2.0,
    },
    headers: vec![
        ("Authorization".into(), "Bearer token".into()),
    ],
};

let client = Client::with_config(config)?;
```

### TypeScript Client

```typescript
import { OrmdbClient } from "@ormdb/client";

const client = new OrmdbClient({
  baseUrl: "http://localhost:8080",
  timeout: 30000,
  headers: {
    Authorization: "Bearer token",
  },
  retry: {
    maxRetries: 3,
    retryDelay: 100,
    maxDelay: 5000,
  },
  // Connection pooling (Node.js)
  pool: {
    maxConnections: 10,
    idleTimeout: 60000,
  },
});
```

### Python Client

```python
from ormdb import OrmdbClient

client = OrmdbClient(
    base_url="http://localhost:8080",
    timeout=30.0,
    headers={"Authorization": "Bearer token"},
    max_retries=3,
    retry_delay=0.1,
    pool_size=10,
)
```

---

## Client Options Reference

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `base_url` | string | required | Server URL |
| `timeout` | duration | 30s | Request timeout |
| `max_connections` | integer | 10 | Connection pool size |
| `max_retries` | integer | 3 | Max retry attempts |
| `retry_delay` | duration | 100ms | Initial retry delay |
| `max_delay` | duration | 5s | Max retry delay |
| `headers` | map | {} | Default request headers |

---

## Docker Configuration

### docker-compose.yml

```yaml
version: "3.8"

services:
  ormdb:
    image: ormdb/ormdb:latest
    ports:
      - "8080:8080"
      - "9090:9090"
    volumes:
      - ormdb-data:/data
      - ./ormdb.toml:/etc/ormdb/ormdb.toml:ro
    environment:
      - ORMDB_STORAGE_PATH=/data
      - ORMDB_STORAGE_CACHE_SIZE_MB=512
      - ORMDB_LOGGING_LEVEL=info
    deploy:
      resources:
        limits:
          memory: 2G

volumes:
  ormdb-data:
```

### Kubernetes ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ormdb-config
data:
  ormdb.toml: |
    [server]
    host = "0.0.0.0"
    port = 8080

    [storage]
    path = "/data"
    cache_size_mb = 512

    [query]
    max_entities = 10000
    max_depth = 5
```

---

## Production Recommendations

### Memory Sizing

| Workload | Cache Size | Total Memory |
|----------|------------|--------------|
| Development | 64-256 MB | 512 MB |
| Small (< 1GB data) | 256-512 MB | 1 GB |
| Medium (1-10GB data) | 512 MB - 2 GB | 4 GB |
| Large (> 10GB data) | 2-8 GB | 16 GB |

### Storage Configuration

```toml
[storage]
# Production settings
path = "/var/lib/ormdb"
cache_size_mb = 2048
sync_mode = "normal"
compression = true
max_log_size_mb = 128
checkpoint_interval_ms = 30000
```

### Security Configuration

```toml
[security]
# Production settings
enable_rls = true
enable_field_masking = true
capability_check = true
max_token_age_seconds = 900  # 15 minutes

[server]
# TLS (via reverse proxy recommended)
# Or native TLS:
# tls_cert = "/etc/ormdb/cert.pem"
# tls_key = "/etc/ormdb/key.pem"
```

### Logging Configuration

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/ormdb/ormdb.log"
max_size_mb = 100
max_files = 10
```

---

## Configuration Validation

Validate configuration before starting:

```bash
# Check config syntax
ormdb config validate --config ormdb.toml

# Show effective config (with defaults)
ormdb config show --config ormdb.toml

# Test connection settings
ormdb config test --config ormdb.toml
```

---

## Next Steps

- **[Deployment Guide](../operations/deployment.md)** - Deploy ORMDB in production
- **[Monitoring](../operations/monitoring.md)** - Set up observability
- **[CLI Reference](cli.md)** - Configuration commands
