#!/usr/bin/env bash
# Scenario 3: Three-device convergence
#
# Three devices in a full-mesh topology each write an item, then start
# sync. All three must converge to the same state.

set -euo pipefail
source "$(dirname "$0")/../lib.sh"

create_network

echo "── Step 1: Start three devices ──"
start_device dev-a
start_device dev-b
start_device dev-c

echo ""
echo "── Step 2: Create capsule on A, pair with B and C ──"
# Pair A↔B
create_capsule dev-a "mesh-test"
capsule="$CAPSULE_ID"
invite_ab="$INVITE_CODE"

import_invite dev-b "$invite_ab"
accept_response dev-a "$capsule" "$RESPONSE_CODE"

# Export a fresh invite for C
invite_ac="$(exec_cli dev-a capsule export "$capsule" | tail -1)"
import_invite dev-c "$invite_ac"
accept_response dev-a "$capsule" "$RESPONSE_CODE"

echo ""
echo "── Step 3: Configure full-mesh static peers ──"
dev_a_id="$(device_id dev-a)"
dev_b_id="$(device_id dev-b)"
dev_c_id="$(device_id dev-c)"

add_static_peer dev-a "$capsule" "$dev_b_id" "dev-b"
add_static_peer dev-a "$capsule" "$dev_c_id" "dev-c"
add_static_peer dev-b "$capsule" "$dev_a_id" "dev-a"
add_static_peer dev-b "$capsule" "$dev_c_id" "dev-c"
add_static_peer dev-c "$capsule" "$dev_a_id" "dev-a"
add_static_peer dev-c "$capsule" "$dev_b_id" "dev-b"

echo ""
echo "── Step 4: Create shared flow on all devices ──"
create_flow dev-a "$capsule"
flow="$FLOW_UUID"
create_flow_with_uuid dev-b "$capsule" "$flow"
create_flow_with_uuid dev-c "$capsule" "$flow"

echo ""
echo "── Step 5: Write one item on each device (before sync) ──"
add_item dev-a "$flow" "from_a" "Item written by A"
add_item dev-b "$flow" "from_b" "Item written by B"
add_item dev-c "$flow" "from_c" "Item written by C"

echo ""
echo "── Step 6: Start sync on all devices ──"
start_sync dev-a
start_sync dev-b
start_sync dev-c
wait_for_sync 12

echo ""
echo "── Step 7: Verify all devices have all items ──"
for dev in dev-a dev-b dev-c; do
    echo "Checking $dev..."
    assert_has_item "$dev" "$flow" "from_a"
    assert_has_item "$dev" "$flow" "from_b"
    assert_has_item "$dev" "$flow" "from_c"
done

echo ""
echo "All assertions passed."
