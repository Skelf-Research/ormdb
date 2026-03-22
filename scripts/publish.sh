#!/bin/bash

# ORMDB Crates Publishing Script
# Publishes crates to crates.io in dependency order with delays

set -e

SLEEP_DURATION=30

echo "========================================"
echo "ORMDB Crates Publishing Script"
echo "========================================"
echo ""
echo "This script will publish all ORMDB crates to crates.io"
echo "in the correct dependency order with ${SLEEP_DURATION}s delays."
echo ""
echo "Crates will be published in this order:"
echo "  1. ormdb-proto   (no dependencies)"
echo "  2. ormdb-lang    (depends on: ormdb-proto)"
echo "  3. ormdb-core    (depends on: ormdb-proto)"
echo "  4. ormdb-client  (depends on: ormdb-proto)"
echo "  5. ormdb-server  (depends on: ormdb-core, ormdb-proto)"
echo "  6. ormdb-cli     (depends on: ormdb-client, ormdb-lang, ormdb-proto)"
echo "  7. ormdb-gateway (depends on: ormdb-client, ormdb-proto)"
echo ""
echo "Note: ormdb-bench is skipped (publish = false)"
echo ""

# Check if --dry-run flag is passed
DRY_RUN=""
if [[ "$1" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    echo "DRY RUN MODE - No actual publishing will occur"
    echo ""
fi

read -p "Continue with publishing? (y/N) " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo ""

publish_crate() {
    local crate=$1
    local crate_path="crates/${crate}"

    echo "----------------------------------------"
    echo "Publishing: ${crate}"
    echo "----------------------------------------"

    cd "${crate_path}"
    cargo publish ${DRY_RUN}
    cd - > /dev/null

    if [[ -z "$DRY_RUN" ]]; then
        echo ""
        echo "Waiting ${SLEEP_DURATION} seconds for crates.io to index..."
        sleep ${SLEEP_DURATION}
    fi

    echo ""
}

# Change to project root
cd "$(dirname "$0")/.."

# Verify we're in the right directory
if [[ ! -f "Cargo.toml" ]]; then
    echo "Error: Must be run from the ormdb project root"
    exit 1
fi

# Run cargo check first to ensure everything compiles
echo "Running cargo check to verify build..."
cargo check --all
echo ""

# Publish in dependency order
publish_crate "ormdb-proto"
publish_crate "ormdb-lang"
publish_crate "ormdb-core"
publish_crate "ormdb-client"
publish_crate "ormdb-server"
publish_crate "ormdb-cli"
publish_crate "ormdb-gateway"

echo "========================================"
echo "All crates published successfully!"
echo "========================================"
echo ""
echo "Published crates:"
echo "  - ormdb-proto"
echo "  - ormdb-lang"
echo "  - ormdb-core"
echo "  - ormdb-client"
echo "  - ormdb-server"
echo "  - ormdb-cli"
echo "  - ormdb-gateway"
echo ""
echo "View on crates.io:"
echo "  https://crates.io/crates/ormdb-proto"
echo "  https://crates.io/crates/ormdb-lang"
echo "  https://crates.io/crates/ormdb-core"
echo "  https://crates.io/crates/ormdb-client"
echo "  https://crates.io/crates/ormdb-server"
echo "  https://crates.io/crates/ormdb-cli"
echo "  https://crates.io/crates/ormdb-gateway"
