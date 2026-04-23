#!/usr/bin/env bash
# Scenario 1: Two-device bidirectional sync
#
# Two devices pair, write items, start sync, and verify both sides converge.
# Items are written before sync starts so the journal is populated when the
# sync process opens the flow.

set -euo pipefail
source "$(dirname "$0")/../lib.sh"

create_network

echo "── Step 1: Start devices ──"
start_device dev-a
start_device dev-b

echo ""
echo "── Step 2: Pair devices ──"
pair_devices dev-a dev-b "test-capsule"
capsule="$CAPSULE_ID"

echo ""
echo "── Step 3: Configure static peers ──"
dev_a_id="$(device_id dev-a)"
dev_b_id="$(device_id dev-b)"
add_static_peer dev-a "$capsule" "$dev_b_id" "dev-b"
add_static_peer dev-b "$capsule" "$dev_a_id" "dev-a"

echo ""
echo "── Step 4: Create flow on both devices ──"
create_flow dev-a "$capsule"
flow="$FLOW_UUID"
create_flow_with_uuid dev-b "$capsule" "$flow"

echo ""
echo "── Step 5: Write item on A (before sync) ──"
add_item dev-a "$flow" "task_from_a" "Written on device A"

echo ""
echo "── Step 6: Start sync on both ──"
start_sync dev-a
start_sync dev-b
wait_for_sync 6

echo ""
echo "── Step 7: Verify B received A's item ──"
assert_has_item dev-b "$flow" "task_from_a"

echo ""
echo "── Step 8: Write item on B, restart sync on both to pick it up ──"
add_item dev-b "$flow" "task_from_b" "Written on device B"
# Restart both sides so they reconnect with updated journals
restart_sync dev-a
restart_sync dev-b
wait_for_sync 8

echo ""
echo "── Step 9: Verify A received B's item ──"
assert_has_item dev-a "$flow" "task_from_b"

echo ""
echo "── Step 10: Verify both have everything ──"
assert_has_item dev-a "$flow" "task_from_a"
assert_has_item dev-a "$flow" "task_from_b"
assert_has_item dev-b "$flow" "task_from_a"
assert_has_item dev-b "$flow" "task_from_b"

echo ""
echo "All assertions passed."
