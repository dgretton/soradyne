#!/usr/bin/env bash
# Shared helpers for Docker-based soradyne sync integration tests.
#
# Source this file from test scenarios:
#   source "$(dirname "$0")/../lib.sh"

set -euo pipefail

IMAGE_NAME="soradyne-test"
NETWORK_NAME="soradyne-test-net"
CONTAINERS=()

# ── Image & network ──────────────────────────────────────────────────

build_image() {
    local repo_root
    repo_root="$(cd "$(dirname "$0")/../../../../.." && pwd)"
    echo "Building $IMAGE_NAME from $repo_root ..."
    docker build \
        -f "$repo_root/packages/soradyne_core/tests/docker/Dockerfile" \
        -t "$IMAGE_NAME" \
        "$repo_root"
}

create_network() {
    docker network inspect "$NETWORK_NAME" &>/dev/null || \
        docker network create "$NETWORK_NAME" >/dev/null
}

# ── Device lifecycle ─────────────────────────────────────────────────

# Start an idle container acting as a "device".
# Usage: start_device <name>
start_device() {
    local name="$1"
    docker run -d \
        --network "$NETWORK_NAME" \
        --name "$name" \
        --entrypoint sleep \
        "$IMAGE_NAME" infinity >/dev/null
    CONTAINERS+=("$name")
    echo "Started device: $name"
}

# Run soradyne-cli inside a container.
# Usage: exec_cli <device> <args...>
exec_cli() {
    local device="$1"; shift
    docker exec "$device" soradyne-cli "$@"
}

# Get the device UUID.
device_id() {
    exec_cli "$1" device-id
}

# ── Capsule management ───────────────────────────────────────────────

# Create a capsule and capture its invite code.
# Usage: create_capsule <device> <name>
# Sets: CAPSULE_ID, INVITE_CODE
create_capsule() {
    local device="$1" name="$2"
    local output
    output="$(exec_cli "$device" capsule create --name "$name")"

    # Parse: Created capsule "name" (uuid)
    CAPSULE_ID="$(echo "$output" | head -1 | sed 's/.*(\(.*\))/\1/')"
    # Invite code is the last non-empty line
    INVITE_CODE="$(echo "$output" | tail -1)"

    echo "Created capsule $CAPSULE_ID on $device"
}

# Import an invite on the joining device, returning the response code.
# Usage: import_invite <device> <invite_code>
# Sets: RESPONSE_CODE
import_invite() {
    local device="$1" invite_code="$2"
    local output
    output="$(exec_cli "$device" capsule import "$invite_code")"
    RESPONSE_CODE="$(echo "$output" | tail -1)"
    echo "Imported capsule on $device"
}

# Accept a response code on the inviting device.
# Usage: accept_response <device> <capsule_id> <response_code>
accept_response() {
    local device="$1" capsule_id="$2" response_code="$3"
    exec_cli "$device" capsule accept-response "$capsule_id" "$response_code"
}

# Full pairing sequence: create on A, import on B, accept on A.
# Usage: pair_devices <inviter> <joiner> <capsule_name>
# Sets: CAPSULE_ID
pair_devices() {
    local inviter="$1" joiner="$2" name="$3"
    create_capsule "$inviter" "$name"
    import_invite "$joiner" "$INVITE_CODE"
    accept_response "$inviter" "$CAPSULE_ID" "$RESPONSE_CODE"
    echo "Paired $inviter ↔ $joiner in capsule $CAPSULE_ID"
}

# ── Static peers ─────────────────────────────────────────────────────

# Register a static peer on a device.
# Resolves the peer's Docker hostname to an IP inside the container.
# Usage: add_static_peer <device> <capsule_id> <peer_device_id> <peer_container_name>
add_static_peer() {
    local device="$1" capsule_id="$2" peer_id="$3" peer_host="$4"
    # Resolve hostname to IP inside the Docker network
    local peer_ip
    peer_ip="$(docker exec "$device" getent hosts "$peer_host" | awk '{print $1}')"
    if [ -z "$peer_ip" ]; then
        echo "ERROR: could not resolve $peer_host from $device"
        return 1
    fi
    exec_cli "$device" capsule add-peer "$capsule_id" "$peer_id" "${peer_ip}:7979"
}

# ── Flow management ─────────────────────────────────────────────────

# Create a flow associated with a capsule.
# Usage: create_flow <device> <capsule_id>
# Sets: FLOW_UUID
create_flow() {
    local device="$1" capsule_id="$2"
    FLOW_UUID="$(exec_cli "$device" flow create --capsule "$capsule_id")"
    echo "Created flow $FLOW_UUID on $device"
}

# Create a flow with a specific UUID (for sharing across devices).
# Usage: create_flow_with_uuid <device> <capsule_id> <uuid>
create_flow_with_uuid() {
    local device="$1" capsule_id="$2" uuid="$3"
    # Create the flow directory manually and write capsule_id,
    # then touch it via add-item + inspect to initialize.
    docker exec "$device" mkdir -p "/root/.soradyne/flows/$uuid"
    docker exec "$device" sh -c "echo '$capsule_id' > /root/.soradyne/flows/$uuid/capsule_id"
    echo "Created flow $uuid on $device (manual)"
}

# Add an item to a flow.
# Usage: add_item <device> <flow_uuid> <item_id> <title>
add_item() {
    local device="$1" flow_uuid="$2" item_id="$3" title="$4"
    exec_cli "$device" flow add-item "$flow_uuid" "$item_id" "$title"
}

# ── Sync ─────────────────────────────────────────────────────────────

# Start sync in the background inside a container.
# Usage: start_sync <device>
start_sync() {
    local device="$1"
    docker exec -d "$device" soradyne-cli sync
    echo "Sync started on $device"
}

# Stop sync on a device (kills soradyne-cli sync process).
# Usage: stop_sync <device>
stop_sync() {
    local device="$1"
    docker exec "$device" pkill -f "soradyne-cli sync" 2>/dev/null || true
    sleep 0.5
}

# Restart sync (stop then start). Needed after writing items because
# `flow add-item` writes to the on-disk journal but the running sync
# process has its own in-memory flow instance.
# Usage: restart_sync <device>
restart_sync() {
    local device="$1"
    stop_sync "$device"
    start_sync "$device"
}

# ── Assertions ───────────────────────────────────────────────────────

# Assert a flow contains an item ID.
# Usage: assert_has_item <device> <flow_uuid> <item_id>
assert_has_item() {
    local device="$1" flow_uuid="$2" item_id="$3"
    local state
    state="$(exec_cli "$device" flow inspect "$flow_uuid")"
    if echo "$state" | grep -q "$item_id"; then
        echo "  ✓ $device has item $item_id"
    else
        echo "  ✗ $device MISSING item $item_id"
        echo "    Flow state:"
        echo "$state" | sed 's/^/      /'
        return 1
    fi
}

# Assert a flow does NOT contain an item ID.
# Usage: assert_missing_item <device> <flow_uuid> <item_id>
assert_missing_item() {
    local device="$1" flow_uuid="$2" item_id="$3"
    local state
    state="$(exec_cli "$device" flow inspect "$flow_uuid")"
    if echo "$state" | grep -q "$item_id"; then
        echo "  ✗ $device UNEXPECTEDLY has item $item_id"
        return 1
    else
        echo "  ✓ $device correctly missing item $item_id"
    fi
}

# Wait for sync propagation.
# Usage: wait_for_sync [seconds]
wait_for_sync() {
    local seconds="${1:-6}"
    echo "Waiting ${seconds}s for sync propagation..."
    sleep "$seconds"
}

# ── Cleanup ──────────────────────────────────────────────────────────

cleanup() {
    echo "Cleaning up..."
    for container in "${CONTAINERS[@]}"; do
        docker rm -f "$container" &>/dev/null || true
    done
    docker network rm "$NETWORK_NAME" &>/dev/null || true
    echo "Done."
}

# Register cleanup on exit
trap cleanup EXIT
