#!/usr/bin/env bash
# Run all Docker-based sync integration tests.
#
# Usage:
#   ./run_tests.sh              # run all scenarios
#   ./run_tests.sh 01           # run only scenario 01
#   ./run_tests.sh --no-build   # skip Docker image build

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SCENARIOS_DIR="$SCRIPT_DIR/scenarios"

# Parse args
SKIP_BUILD=false
FILTER=""
for arg in "$@"; do
    case "$arg" in
        --no-build) SKIP_BUILD=true ;;
        *) FILTER="$arg" ;;
    esac
done

# Source lib for build_image
source "$SCRIPT_DIR/lib.sh"

# Build image (unless skipped)
if [ "$SKIP_BUILD" = false ]; then
    build_image
fi

# Ensure the test network doesn't linger from a previous run
docker network rm "$NETWORK_NAME" &>/dev/null || true

# Run scenarios
passed=0
failed=0
total=0

for scenario in "$SCENARIOS_DIR"/*.sh; do
    name="$(basename "$scenario" .sh)"

    # Apply filter if specified
    if [ -n "$FILTER" ] && [[ "$name" != *"$FILTER"* ]]; then
        continue
    fi

    total=$((total + 1))
    echo ""
    echo "════════════════════════════════════════════════════════════"
    echo "  SCENARIO: $name"
    echo "════════════════════════════════════════════════════════════"
    echo ""

    if bash "$scenario"; then
        passed=$((passed + 1))
        echo ""
        echo "  → PASSED"
    else
        failed=$((failed + 1))
        echo ""
        echo "  → FAILED"
    fi
done

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  RESULTS: $passed/$total passed, $failed failed"
echo "════════════════════════════════════════════════════════════"

exit "$failed"
