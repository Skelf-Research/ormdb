# Jepsen Tests for ORMDB

This directory contains [Jepsen](https://jepsen.io) tests for verifying the consistency and correctness of ORMDB under various failure conditions.

## Prerequisites

- Docker and Docker Compose
- Leiningen (Clojure build tool)
- Rust toolchain (for building ormdb)

## Quick Start

1. **Build ormdb binaries:**
   ```bash
   cd ..
   cargo build --release -p ormdb-server -p ormdb-gateway
   ```

2. **Start the test cluster:**
   ```bash
   ./scripts/quick-start.sh
   ```

3. **Run a test:**
   ```bash
   lein run test --workload register --nemesis none --time-limit 60
   ```

4. **View results:**
   ```bash
   open store/latest/timeline.html
   ```

5. **Stop the cluster:**
   ```bash
   docker-compose down -v
   ```

## Test Workloads

| Workload | Description | Consistency Model |
|----------|-------------|-------------------|
| `register` | Single-register read/write/CAS | Linearizability |
| `bank` | Account transfers | Serializability |
| `set` | Append-only sets | Set consistency |
| `list-append` | Transactional list appends | Strict serializability (Elle) |

## Nemesis Types

| Nemesis | Description |
|---------|-------------|
| `none` | No faults (baseline) |
| `kill` | SIGKILL random nodes |
| `pause` | SIGSTOP/SIGCONT processes |
| `partition` | Network partitions |
| `clock` | Clock skew injection |
| `combined` | All fault types |

## Running Tests

### Single Test

```bash
lein run test \
  --workload register \
  --nemesis partition \
  --time-limit 180 \
  --rate 10 \
  --nodes n1,n2,n3,n4,n5
```

### All Tests

```bash
./scripts/run-all-tests.sh
```

### From Docker

```bash
docker-compose run control lein run test --workload register --nemesis none
```

## CLI Options

| Option | Description | Default |
|--------|-------------|---------|
| `--workload` | Test workload | `register` |
| `--nemesis` | Fault type | `none` |
| `--time-limit` | Test duration (seconds) | `60` |
| `--rate` | Operations per second | `10` |
| `--nodes` | Cluster nodes | `n1,n2,n3,n4,n5` |
| `--concurrency` | Clients per node | `5` |
| `--accounts` | Bank accounts | `5` |
| `--key-count` | Keys for multi-key tests | `10` |

## Understanding Results

Test results are stored in `store/latest/`:

- `results.edn` - Overall test results
- `timeline.html` - Visual timeline of operations
- `history.edn` - Full operation history
- `jepsen.log` - Test logs

### Interpreting Results

```clojure
;; Passing test
{:valid? true
 ...}

;; Failing test (consistency violation)
{:valid? false
 :anomalies {...}
 ...}
```

## Project Structure

```
jepsen.ormdb/
├── project.clj                 # Dependencies
├── docker-compose.yml          # Test cluster
├── src/jepsen/ormdb/
│   ├── core.clj               # Main entry point
│   ├── db.clj                 # DB lifecycle
│   ├── client.clj             # HTTP client
│   ├── nemesis.clj            # Fault injection
│   ├── checker.clj            # Custom checkers
│   └── workloads/
│       ├── register.clj       # Linearizability
│       ├── bank.clj           # Serializability
│       ├── set.clj            # Set consistency
│       └── list_append.clj    # Elle (txn)
├── scripts/
│   ├── quick-start.sh
│   └── run-all-tests.sh
└── store/                      # Test results
```

## Requirements for ORMDB

Before running Jepsen tests, ORMDB must have:

1. **Raft Consensus** - Leader election and log replication
2. **Linearizable Reads** - ReadIndex or LeaseRead
3. **HTTP Gateway** - REST API via ormdb-gateway

## CI Integration

Tests run automatically via GitHub Actions:
- On push/PR: Quick tests (register, bank with no faults)
- Nightly: Comprehensive tests (all workloads x all nemeses)

## Troubleshooting

### Test Hangs

```bash
# Check node status
docker-compose ps
docker-compose logs n1

# Restart cluster
docker-compose down -v && docker-compose up -d
```

### Connection Refused

Ensure ormdb-gateway is running on port 8080:
```bash
docker exec jepsen-n1 ps aux | grep ormdb
```

### OutOfMemory

Increase JVM heap in `project.clj`:
```clojure
:jvm-opts ["-Xmx16g" ...]
```

## References

- [Jepsen Documentation](https://jepsen.io/analyses)
- [Elle Paper](https://arxiv.org/abs/2003.10554)
- [Knossos](https://github.com/jepsen-io/knossos)
