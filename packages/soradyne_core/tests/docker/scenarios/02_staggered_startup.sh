#!/usr/bin/env bash
# Scenario 2: Staggered startup — offline device catches up
#
# Device A writes data before B even exists. B starts later, pairs, and
# receives all of A's history via horizon exchange on connect.

set -euo pipefail
source "$(dirname "$0")/../lib.sh"

create_network

echo "── Step 1: Start device A only ──"
start_device dev-a

echo ""
echo "── Step 2: Create capsule and flow on A ──"
create_capsule dev-a "staggered-test"
capsule="$CAPSULE_ID"
invite="$INVITE_CODE"

create_flow dev-a "$capsule"
flow="$FLOW_UUID"

echo ""
echo "── Step 3: Write items on A (B doesn't exist yet) ──"
add_item dev-a "$flow" "early_1" "Written before B exists"
add_item dev-a "$flow" "early_2" "Also before B"
add_item dev-a "$flow" "early_3" "Third early item"

echo ""
echo "── Step 4: NOW start device B ──"
start_device dev-b

echo ""
echo "── Step 5: Pair B with A ──"
import_invite dev-b "$invite"
accept_response dev-a "$capsule" "$RESPONSE_CODE"

echo ""
echo "── Step 6: Configure static peers ──"
dev_a_id="$(device_id dev-a)"
dev_b_id="$(device_id dev-b)"
add_static_peer dev-a "$capsule" "$dev_b_id" "dev-b"
add_static_peer dev-b "$capsule" "$dev_a_id" "dev-a"

echo ""
echo "── Step 7: Create flow on B, start sync on both ──"
create_flow_with_uuid dev-b "$capsule" "$flow"
start_sync dev-a
start_sync dev-b
wait_for_sync 8

echo ""
echo "── Step 8: Verify B has all of A's items ──"
assert_has_item dev-b "$flow" "early_1"
assert_has_item dev-b "$flow" "early_2"
assert_has_item dev-b "$flow" "early_3"

echo ""
echo "All assertions passed."
