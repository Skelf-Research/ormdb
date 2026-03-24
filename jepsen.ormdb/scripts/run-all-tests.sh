#!/bin/bash
# Run all Jepsen tests for ormdb

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Configuration
TIME_LIMIT=${TIME_LIMIT:-180}
RATE=${RATE:-10}
NODES=${NODES:-"n1,n2,n3,n4,n5"}

# Workloads and nemeses to test
WORKLOADS=("register" "bank" "set" "list-append")
NEMESES=("none" "kill" "partition")

# Results directory
RESULTS_DIR="$PROJECT_DIR/results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "Running Jepsen tests for ormdb"
echo "Results will be saved to: $RESULTS_DIR"
echo "Time limit: $TIME_LIMIT seconds"
echo "Rate: $RATE ops/sec"
echo ""

# Track results
PASSED=0
FAILED=0
ERRORS=()

for workload in "${WORKLOADS[@]}"; do
    for nemesis in "${NEMESES[@]}"; do
        TEST_NAME="${workload}-${nemesis}"
        echo "=========================================="
        echo "Running test: $TEST_NAME"
        echo "=========================================="

        # Run the test
        if lein run test \
            --workload "$workload" \
            --nemesis "$nemesis" \
            --time-limit "$TIME_LIMIT" \
            --rate "$RATE" \
            --nodes "$NODES" \
            "$@" 2>&1 | tee "$RESULTS_DIR/${TEST_NAME}.log"; then

            # Check if test passed
            if [ -f "store/latest/results.edn" ]; then
                if grep -q ":valid? true" "store/latest/results.edn"; then
                    echo "PASSED: $TEST_NAME"
                    PASSED=$((PASSED + 1))
                elif grep -q ":valid? false" "store/latest/results.edn"; then
                    echo "FAILED: $TEST_NAME (consistency violation)"
                    FAILED=$((FAILED + 1))
                    ERRORS+=("$TEST_NAME: consistency violation")
                else
                    echo "UNKNOWN: $TEST_NAME"
                    FAILED=$((FAILED + 1))
                    ERRORS+=("$TEST_NAME: unknown result")
                fi

                # Copy results
                cp -r "store/latest" "$RESULTS_DIR/${TEST_NAME}"
            else
                echo "ERROR: $TEST_NAME (no results file)"
                FAILED=$((FAILED + 1))
                ERRORS+=("$TEST_NAME: no results file")
            fi
        else
            echo "ERROR: $TEST_NAME (test crashed)"
            FAILED=$((FAILED + 1))
            ERRORS+=("$TEST_NAME: test crashed")
        fi

        echo ""
    done
done

# Print summary
echo "=========================================="
echo "TEST SUMMARY"
echo "=========================================="
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "Errors:"
    for error in "${ERRORS[@]}"; do
        echo "  - $error"
    done
    echo ""
    exit 1
else
    echo "All tests passed!"
    exit 0
fi
