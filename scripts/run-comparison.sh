#!/bin/bash
# ORMDB Comparison Benchmark Runner
#
# Usage:
#   ./scripts/run-comparison.sh              # Run all comparison benchmarks
#   ./scripts/run-comparison.sh baseline     # Run only baseline benchmarks
#   ./scripts/run-comparison.sh sqlite       # Run only SQLite comparison
#   ./scripts/run-comparison.sh postgres     # Run only PostgreSQL comparison (requires DATABASE_URL)
#   ./scripts/run-comparison.sh --quick      # Quick run with fewer iterations

set -e

# Configuration
export RUST_LOG=${RUST_LOG:-warn}
export ORMDB_BENCH_SCALE=${ORMDB_BENCH_SCALE:-small}  # Use small for comparison

BENCHMARK_DIR="target/criterion"
QUICK_MODE=false
SPECIFIC_BENCH=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --scale)
            export ORMDB_BENCH_SCALE="$2"
            shift 2
            ;;
        --help|-h)
            echo "ORMDB Comparison Benchmark Runner"
            echo ""
            echo "Usage: $0 [options] [benchmark-name]"
            echo ""
            echo "Options:"
            echo "  --quick       Run with fewer iterations for quick feedback"
            echo "  --scale SIZE  Set data scale (small/medium/large)"
            echo "  --help        Show this help message"
            echo ""
            echo "Benchmark suites:"
            echo "  baseline      Internal baselines (raw vs typed)"
            echo "  sqlite        ORMDB vs SQLite comparison"
            echo "  postgres      ORMDB vs PostgreSQL comparison (requires DATABASE_URL)"
            echo ""
            echo "Environment variables:"
            echo "  RUST_LOG              Log level (default: warn)"
            echo "  ORMDB_BENCH_SCALE     Data scale factor (default: small)"
            echo "  DATABASE_URL          PostgreSQL connection string (for postgres benchmarks)"
            echo ""
            echo "Examples:"
            echo "  $0                            # Run baseline and SQLite"
            echo "  $0 sqlite                     # Run only SQLite comparison"
            echo "  $0 postgres                   # Run PostgreSQL comparison"
            echo "  DATABASE_URL=postgres://localhost/bench $0 postgres"
            exit 0
            ;;
        *)
            SPECIFIC_BENCH="$1"
            shift
            ;;
    esac
done

# Build quick mode arguments
BENCH_ARGS=""
if [ "$QUICK_MODE" = true ]; then
    BENCH_ARGS="-- --quick"
    echo "Running in quick mode (fewer iterations)"
fi

echo "====================================="
echo "ORMDB Comparison Benchmarks"
echo "====================================="
echo "Scale: $ORMDB_BENCH_SCALE"
echo "Log level: $RUST_LOG"
echo ""

run_benchmark() {
    local bench="$1"
    local extra_args="$2"
    echo "-----------------------------------"
    echo "Running: $bench"
    echo "-----------------------------------"
    cargo bench --package ormdb-bench --bench "$bench" $extra_args $BENCH_ARGS
    echo ""
}

# Run benchmarks
if [ -n "$SPECIFIC_BENCH" ]; then
    case "$SPECIFIC_BENCH" in
        baseline)
            run_benchmark "baseline"
            ;;
        sqlite)
            run_benchmark "vs_sqlite"
            ;;
        postgres)
            if [ -z "$DATABASE_URL" ]; then
                echo "Error: DATABASE_URL environment variable not set"
                echo "Example: DATABASE_URL=postgres://localhost/ormdb_bench $0 postgres"
                exit 1
            fi
            run_benchmark "vs_postgres" "--features postgres"
            ;;
        *)
            echo "Unknown benchmark: $SPECIFIC_BENCH"
            echo "Available: baseline, sqlite, postgres"
            exit 1
            ;;
    esac
else
    # Run baseline and SQLite by default (no external dependencies)
    run_benchmark "baseline"
    run_benchmark "vs_sqlite"

    # Run PostgreSQL if DATABASE_URL is set
    if [ -n "$DATABASE_URL" ]; then
        run_benchmark "vs_postgres" "--features postgres"
    else
        echo "-----------------------------------"
        echo "Skipping PostgreSQL benchmarks"
        echo "(set DATABASE_URL to enable)"
        echo "-----------------------------------"
    fi
fi

echo "====================================="
echo "Comparison Benchmarks Complete"
echo "====================================="
echo ""
echo "HTML reports available in: $BENCHMARK_DIR/"
echo ""
echo "Key comparisons to analyze:"
echo "  - vs_sqlite/n_plus_1: ORMDB batched vs SQLite N+1/batched/JOIN"
echo "  - vs_sqlite/scan: Full scan performance"
echo "  - vs_sqlite/filter_*: Filter evaluation performance"
echo "  - baseline/*: Internal overhead measurements"
echo ""
echo "To view reports:"
echo "  open $BENCHMARK_DIR/report/index.html"
