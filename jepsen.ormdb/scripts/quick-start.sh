#!/bin/bash
# Quick start script for running Jepsen tests locally

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ORMDB_ROOT="$(dirname "$PROJECT_DIR")"

cd "$PROJECT_DIR"

echo "ORMDB Jepsen Test Quick Start"
echo "=============================="
echo ""

# Check for required tools
check_tool() {
    if ! command -v "$1" &> /dev/null; then
        echo "Error: $1 is not installed"
        exit 1
    fi
}

check_tool docker
check_tool docker-compose
check_tool lein

# Build ormdb binaries if not present
if [ ! -f "$ORMDB_ROOT/target/release/ormdb-server" ] || [ ! -f "$ORMDB_ROOT/target/release/ormdb-gateway" ]; then
    echo "Building ormdb binaries with Raft support..."
    cd "$ORMDB_ROOT"
    cargo build --release -p ormdb-server --features raft -p ormdb-gateway
    cd "$PROJECT_DIR"
fi

# Start the cluster
echo "Starting Jepsen cluster..."
docker-compose up -d

# Wait for nodes to be ready
echo "Waiting for cluster to be ready..."
sleep 10

# Copy binaries to nodes
echo "Deploying ormdb binaries..."
for node in n1 n2 n3 n4 n5; do
    docker cp "$ORMDB_ROOT/target/release/ormdb-server" "jepsen-${node}:/ormdb-binaries/"
    docker cp "$ORMDB_ROOT/target/release/ormdb-gateway" "jepsen-${node}:/ormdb-binaries/"
    docker exec "jepsen-${node}" chmod +x /ormdb-binaries/ormdb-server /ormdb-binaries/ormdb-gateway
done

echo ""
echo "Cluster is ready!"
echo ""
echo "To run a test:"
echo "  lein run test --workload register --nemesis none --time-limit 60"
echo ""
echo "Available workloads: register, bank, set, list-append"
echo "Available nemeses: none, kill, pause, partition, clock, combined"
echo ""
echo "To stop the cluster:"
echo "  docker-compose down -v"
