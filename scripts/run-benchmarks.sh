#!/bin/bash
# ORMDB Benchmark Runner
#
# Usage:
#   ./scripts/run-benchmarks.sh           # Run all benchmarks
#   ./scripts/run-benchmarks.sh storage   # Run specific benchmark suite
#   ./scripts/run-benchmarks.sh --quick   # Quick run with fewer iterations

set -e

# Configuration
export RUST_LOG=${RUST_LOG:-warn}
export ORMDB_BENCH_SCALE=${ORMDB_BENCH_SCALE:-medium}  # small/medium/large

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
            echo "ORMDB Benchmark Runner"
            echo ""
            echo "Usage: $0 [options] [benchmark-name]"
            echo ""
            echo "Options:"
            echo "  --quick       Run with fewer iterations for quick feedback"
            echo "  --scale SIZE  Set data scale (small/medium/large)"
            echo "  --help        Show this help message"
            echo ""
            echo "Benchmark suites:"
            echo "  storage       Storage engine benchmarks"
            echo "  query         Query executor benchmarks"
            echo "  mutation      Mutation benchmarks"
            echo "  serialization Serialization (rkyv vs JSON) benchmarks"
            echo "  filter        Filter evaluation benchmarks"
            echo "  join          Join strategy benchmarks"
            echo "  cache         Plan cache benchmarks"
            echo "  e2e           End-to-end benchmarks"
            echo ""
            echo "Environment variables:"
            echo "  RUST_LOG          Log level (default: warn)"
            echo "  ORMDB_BENCH_SCALE Data scale factor (default: medium)"
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

echo "==================================="
echo "ORMDB Benchmark Suite"
echo "==================================="
echo "Scale: $ORMDB_BENCH_SCALE"
echo "Log level: $RUST_LOG"
echo ""

# List of all benchmark suites
BENCHMARKS=(
    "storage"
    "query"
    "mutation"
    "serialization"
    "filter"
    "join"
    "cache"
    "e2e"
)

run_benchmark() {
    local bench="$1"
    echo "-----------------------------------"
    echo "Running: $bench"
    echo "-----------------------------------"
    cargo bench --package ormdb-bench --bench "$bench" $BENCH_ARGS
    echo ""
}

# Run benchmarks
if [ -n "$SPECIFIC_BENCH" ]; then
    run_benchmark "$SPECIFIC_BENCH"
else
    for bench in "${BENCHMARKS[@]}"; do
        run_benchmark "$bench"
    done
fi

echo "==================================="
echo "Benchmark Complete"
echo "==================================="
echo ""
echo "HTML reports available in: $BENCHMARK_DIR/"
echo ""
echo "To view reports:"
echo "  open $BENCHMARK_DIR/report/index.html"
