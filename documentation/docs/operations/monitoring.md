# Monitoring Guide

Monitor ORMDB health, performance, and usage.

## Overview

ORMDB provides multiple monitoring interfaces:

- **Health endpoints** - Liveness and readiness checks
- **Prometheus metrics** - Detailed performance metrics
- **Structured logging** - Operational logs
- **Admin CLI** - Interactive diagnostics

## Health Endpoints

### Liveness Check

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "version": "1.0.0"
}
```

### Readiness Check

```bash
curl http://localhost:8080/ready
```

Response:
```json
{
  "status": "ready",
  "storage": "ok",
  "connections": 42
}
```

## Prometheus Metrics

### Enable Metrics

```toml
# ormdb.toml
[metrics]
enabled = true
port = 9090
path = "/metrics"
```

### Available Metrics

#### Server Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_server_uptime_seconds` | Gauge | Server uptime |
| `ormdb_server_connections_active` | Gauge | Active connections |
| `ormdb_server_connections_total` | Counter | Total connections |
| `ormdb_server_requests_total` | Counter | Total requests |
| `ormdb_server_request_duration_seconds` | Histogram | Request latency |

#### Query Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_query_total` | Counter | Total queries |
| `ormdb_query_duration_seconds` | Histogram | Query latency |
| `ormdb_query_rows_returned` | Histogram | Rows per query |
| `ormdb_query_entities_scanned` | Histogram | Entities scanned |
| `ormdb_query_budget_exceeded_total` | Counter | Budget exceeded |

#### Mutation Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_mutation_total` | Counter | Total mutations |
| `ormdb_mutation_duration_seconds` | Histogram | Mutation latency |
| `ormdb_mutation_rows_affected` | Histogram | Rows affected |
| `ormdb_mutation_conflicts_total` | Counter | Version conflicts |

#### Storage Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_storage_size_bytes` | Gauge | Data size |
| `ormdb_storage_index_size_bytes` | Gauge | Index size |
| `ormdb_storage_wal_size_bytes` | Gauge | WAL size |
| `ormdb_storage_cache_hits_total` | Counter | Cache hits |
| `ormdb_storage_cache_misses_total` | Counter | Cache misses |
| `ormdb_storage_cache_hit_ratio` | Gauge | Cache hit ratio |

#### Entity Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_entity_count` | Gauge | Entities per type |
| `ormdb_entity_size_bytes` | Gauge | Size per entity type |

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'ormdb'
    static_configs:
      - targets: ['ormdb:9090']
    scrape_interval: 15s
```

### Example Queries

```promql
# Request rate
rate(ormdb_server_requests_total[5m])

# Average query latency
histogram_quantile(0.95, rate(ormdb_query_duration_seconds_bucket[5m]))

# Cache hit ratio
ormdb_storage_cache_hit_ratio

# Error rate
rate(ormdb_server_requests_total{status="error"}[5m])
  / rate(ormdb_server_requests_total[5m])

# Slow queries (>100ms)
rate(ormdb_query_duration_seconds_bucket{le="0.1"}[5m])
```

## Grafana Dashboards

### Import Dashboard

```bash
# Download dashboard
curl -O https://raw.githubusercontent.com/ormdb/ormdb/main/dashboards/ormdb-overview.json

# Import via Grafana API
curl -X POST \
  -H "Content-Type: application/json" \
  -d @ormdb-overview.json \
  http://admin:admin@localhost:3000/api/dashboards/db
```

### Key Dashboard Panels

1. **Overview**
   - Server uptime
   - Active connections
   - Request rate
   - Error rate

2. **Performance**
   - Query latency (p50, p95, p99)
   - Mutation latency
   - Cache hit ratio
   - Slow query count

3. **Storage**
   - Data size growth
   - Index size
   - WAL size
   - Disk usage

4. **Entities**
   - Entity counts
   - Growth rates
   - Query patterns

## Alerting

### Prometheus Alerts

```yaml
# alerts.yml
groups:
  - name: ormdb
    rules:
      - alert: OrmdbDown
        expr: up{job="ormdb"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "ORMDB is down"

      - alert: HighErrorRate
        expr: |
          rate(ormdb_server_requests_total{status="error"}[5m])
          / rate(ormdb_server_requests_total[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate (>5%)"

      - alert: SlowQueries
        expr: |
          histogram_quantile(0.95, rate(ormdb_query_duration_seconds_bucket[5m])) > 1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "P95 query latency > 1s"

      - alert: LowCacheHitRatio
        expr: ormdb_storage_cache_hit_ratio < 0.8
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Cache hit ratio below 80%"

      - alert: HighConnectionCount
        expr: ormdb_server_connections_active > 900
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Approaching connection limit"

      - alert: DiskSpaceLow
        expr: |
          ormdb_storage_size_bytes
          / (ormdb_storage_size_bytes + ormdb_storage_free_bytes) > 0.85
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Disk usage above 85%"
```

### Alert Manager Configuration

```yaml
# alertmanager.yml
route:
  receiver: 'team-notifications'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'slack'
    slack_configs:
      - channel: '#alerts'
        api_url: 'https://hooks.slack.com/...'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: '...'
```

## Logging

### Log Configuration

```toml
# ormdb.toml
[logging]
level = "info"           # trace, debug, info, warn, error
format = "json"          # json, pretty, compact
file = "/var/log/ormdb/ormdb.log"
max_size_mb = 100
max_files = 10

[logging.slow_query]
enabled = true
threshold_ms = 100
log_file = "/var/log/ormdb/slow.log"
```

### Log Format

```json
{
  "timestamp": "2024-01-15T12:00:00.000Z",
  "level": "info",
  "target": "ormdb::query",
  "message": "Query completed",
  "entity": "User",
  "duration_ms": 5.2,
  "rows_returned": 42,
  "request_id": "abc-123"
}
```

### Log Aggregation

#### Fluentd

```yaml
# fluent.conf
<source>
  @type tail
  path /var/log/ormdb/ormdb.log
  pos_file /var/log/ormdb/ormdb.log.pos
  tag ormdb
  <parse>
    @type json
  </parse>
</source>

<match ormdb>
  @type elasticsearch
  host elasticsearch
  port 9200
  index_name ormdb-logs
</match>
```

#### Vector

```toml
# vector.toml
[sources.ormdb_logs]
type = "file"
include = ["/var/log/ormdb/*.log"]

[transforms.parse_json]
type = "json_parser"
inputs = ["ormdb_logs"]

[sinks.elasticsearch]
type = "elasticsearch"
inputs = ["parse_json"]
endpoint = "http://elasticsearch:9200"
index = "ormdb-logs-%Y-%m-%d"
```

## Admin CLI Diagnostics

### Server Status

```bash
ormdb admin status

# Output:
# Server Status
# ─────────────────────────────────
# Version:     1.0.0
# Uptime:      5d 12h 30m
# Connections: 42 / 1000
# Memory:      1.2 GB / 4 GB
# CPU:         25%
```

### Query Statistics

```bash
ormdb admin stats --queries

# Output:
# Query Statistics (last hour)
# ─────────────────────────────────
# Total:       125,432
# Avg latency: 2.3ms
# P50:         1.2ms
# P95:         8.5ms
# P99:         45.2ms
#
# Top entities:
#   User:    45,234 queries
#   Post:    38,123 queries
#   Comment: 28,456 queries
```

### Storage Statistics

```bash
ormdb admin stats --storage

# Output:
# Storage Statistics
# ─────────────────────────────────
# Data size:   5.4 GB
# Index size:  1.2 GB
# WAL size:    256 MB
# Free space:  45.2 GB
# Cache hits:  94.2%
```

### Slow Query Log

```bash
ormdb admin slow-queries --limit 10

# Output:
# Slow Queries (>100ms)
# ─────────────────────────────────
# 1. User with posts (avg: 250ms)
#    Count: 42
#    Max: 1.2s
#
# 2. Post full-text search (avg: 180ms)
#    Count: 156
#    Max: 890ms
```

### Connection Info

```bash
ormdb admin connections

# Output:
# Active Connections: 42
# ─────────────────────────────────
# Client          | Queries | Duration
# 192.168.1.10    | 1,234   | 2h 30m
# 192.168.1.11    | 892     | 1h 45m
# 192.168.1.12    | 456     | 45m
```

## Monitoring Best Practices

### 1. Set Up Baseline Metrics

Record normal operation metrics:
- Average query latency
- Typical cache hit ratio
- Normal connection count
- Standard error rate

### 2. Alert on Deviations

```yaml
# Alert when 2x baseline
- alert: HighQueryLatency
  expr: |
    histogram_quantile(0.95, rate(ormdb_query_duration_seconds_bucket[5m]))
    > 2 * avg_over_time(histogram_quantile(0.95, rate(ormdb_query_duration_seconds_bucket[5m]))[7d:1h])
```

### 3. Monitor Trends

- Data growth rate
- Query pattern changes
- Cache efficiency over time

### 4. Correlate Metrics

Cross-reference:
- High latency + low cache hits = possible memory pressure
- High errors + high connections = possible connection exhaustion
- High disk I/O + slow queries = possible index issues

### 5. Regular Reviews

- Weekly: Review slow query log
- Monthly: Analyze growth trends
- Quarterly: Capacity planning review

---

## Next Steps

- **[Troubleshooting](troubleshooting.md)** - Diagnose common issues
- **[Backup & Restore](backup-restore.md)** - Protect your data
- **[Performance Guide](../guides/performance.md)** - Optimize performance
