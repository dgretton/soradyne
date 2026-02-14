//! GATT service definition and RoutedEnvelope message format
//!
//! Defines the Soradyne BLE GATT service UUIDs and the RoutedEnvelope
//! message type used for multi-hop mesh communication between pieces.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Generate a deterministic UUID v4-format from a namespace and name.
/// Uses SHA-256 and formats the output as a UUID (similar to UUID v5 but
/// using SHA-256 instead of SHA-1).
fn deterministic_uuid(namespace: &str, name: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(name.as_bytes());
    let hash = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    // Set version 4 and variant bits for UUID compatibility.
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 1
    Uuid::from_bytes(bytes)
}

/// Primary Soradyne GATT service UUID.
pub fn soradyne_service_uuid() -> Uuid {
    deterministic_uuid("soradyne.rim", "service")
}

/// Characteristic UUID for sending/receiving RoutedEnvelopes.
pub fn envelope_char_uuid() -> Uuid {
    deterministic_uuid("soradyne.rim", "envelope")
}

/// Characteristic UUID for topology state.
pub fn topology_char_uuid() -> Uuid {
    deterministic_uuid("soradyne.rim", "topology")
}

/// Message types carried by RoutedEnvelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Topology synchronization (piece discovery, liveness).
    TopologySync,
    /// Flow data synchronization (CRDT operations).
    FlowSync,
    /// Capsule membership gossip (key distribution, membership changes).
    CapsuleGossip,
}

/// A routed message envelope for multi-hop mesh communication.
///
/// Messages are forwarded between pieces based on TTL and destination.
/// Broadcast messages use `Uuid::nil()` as the destination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutedEnvelope {
    /// Source piece ID.
    pub source: Uuid,
    /// Destination piece ID, or `Uuid::nil()` for broadcast.
    pub destination: Uuid,
    /// Time-to-live: decremented on each hop, dropped at 0.
    pub ttl: u8,
    /// The type of message carried.
    pub message_type: MessageType,
    /// Opaque payload bytes.
    pub payload: Vec<u8>,
}

impl RoutedEnvelope {
    /// Create a unicast envelope addressed to a specific peer.
    pub fn new_unicast(
        source: Uuid,
        destination: Uuid,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            source,
            destination,
            ttl: 8,
            message_type,
            payload,
        }
    }

    /// Create a broadcast envelope (destination = nil UUID).
    pub fn new_broadcast(source: Uuid, message_type: MessageType, payload: Vec<u8>) -> Self {
        Self {
            source,
            destination: Uuid::nil(),
            ttl: 8,
            message_type,
            payload,
        }
    }

    /// Check whether this envelope should be forwarded to a given peer.
    ///
    /// Returns true if:
    /// - TTL > 0
    /// - The peer is not the original source
    /// - The envelope is a broadcast OR the peer is the destination
    pub fn should_forward_to(&self, peer_id: &Uuid) -> bool {
        if self.ttl == 0 {
            return false;
        }
        if peer_id == &self.source {
            return false;
        }
        // Broadcast (nil destination) forwards to everyone except source.
        // Unicast forwards only to the destination.
        self.destination == Uuid::nil() || peer_id == &self.destination
    }

    /// Create a forwarded copy with TTL decremented.
    /// Returns `None` if TTL is already 0.
    pub fn forwarded(&self) -> Option<Self> {
        if self.ttl == 0 {
            return None;
        }
        let mut copy = self.clone();
        copy.ttl -= 1;
        Some(copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicast_forwarding() {
        let source = Uuid::new_v4();
        let dest = Uuid::new_v4();
        let other = Uuid::new_v4();

        let env = RoutedEnvelope::new_unicast(
            source,
            dest,
            MessageType::FlowSync,
            vec![0x01],
        );

        assert!(env.should_forward_to(&dest));
        assert!(!env.should_forward_to(&other));
        assert!(!env.should_forward_to(&source));
    }

    #[test]
    fn test_broadcast_forwarding() {
        let source = Uuid::new_v4();
        let peer_a = Uuid::new_v4();
        let peer_b = Uuid::new_v4();

        let env = RoutedEnvelope::new_broadcast(
            source,
            MessageType::TopologySync,
            vec![0x02],
        );

        assert_eq!(env.destination, Uuid::nil());
        assert!(env.should_forward_to(&peer_a));
        assert!(env.should_forward_to(&peer_b));
        assert!(!env.should_forward_to(&source));
    }

    #[test]
    fn test_ttl_decrement() {
        let env = RoutedEnvelope::new_unicast(
            Uuid::new_v4(),
            Uuid::new_v4(),
            MessageType::FlowSync,
            vec![],
        );
        assert_eq!(env.ttl, 8);

        let fwd = env.forwarded().unwrap();
        assert_eq!(fwd.ttl, 7);

        // Chain until TTL reaches 0.
        let mut current = env;
        for expected_ttl in (0..8).rev() {
            current = current.forwarded().unwrap();
            assert_eq!(current.ttl, expected_ttl);
        }
        assert_eq!(current.ttl, 0);
        assert!(current.forwarded().is_none());
    }

    #[test]
    fn test_no_forward_to_source() {
        let source = Uuid::new_v4();
        let dest = Uuid::new_v4();

        let unicast = RoutedEnvelope::new_unicast(
            source,
            dest,
            MessageType::CapsuleGossip,
            vec![],
        );
        assert!(!unicast.should_forward_to(&source));

        let broadcast = RoutedEnvelope::new_broadcast(
            source,
            MessageType::TopologySync,
            vec![],
        );
        assert!(!broadcast.should_forward_to(&source));
    }

    #[test]
    fn test_serialization_round_trip() {
        let env = RoutedEnvelope::new_unicast(
            Uuid::new_v4(),
            Uuid::new_v4(),
            MessageType::CapsuleGossip,
            vec![0xDE, 0xAD, 0xBE, 0xEF],
        );

        let json = serde_json::to_vec(&env).unwrap();
        let restored: RoutedEnvelope = serde_json::from_slice(&json).unwrap();

        assert_eq!(env, restored);
    }

    #[test]
    fn test_gatt_uuids_deterministic() {
        let uuid1 = soradyne_service_uuid();
        let uuid2 = soradyne_service_uuid();
        assert_eq!(uuid1, uuid2);

        // Different names produce different UUIDs.
        assert_ne!(soradyne_service_uuid(), envelope_char_uuid());
        assert_ne!(envelope_char_uuid(), topology_char_uuid());
    }
}
