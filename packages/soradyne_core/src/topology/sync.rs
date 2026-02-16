//! Topology synchronization protocol message types
//!
//! These are serialized into the `payload` field of a `RoutedEnvelope`
//! with `message_type: MessageType::TopologySync`. They define the
//! messages exchanged between pieces to share connectivity information.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ble::transport::BleAddress;
use crate::topology::capsule::PieceCapabilities;

use super::ensemble::ConnectionQuality;

/// Messages exchanged during topology synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TopologySyncMessage {
    /// "Here's my current topology view"
    TopologyUpdate {
        /// Pieces I can see directly (have a BLE connection to)
        direct_peers: Vec<PeerInfo>,
        /// Pieces I've heard about from others (indirect knowledge)
        indirect_peers: Vec<PeerInfo>,
        /// Hash of my topology state for quick comparison
        topology_hash: u32,
    },
    /// "I have connection info for piece X that you might need"
    PeerIntroduction {
        /// The piece being introduced
        piece_id: Uuid,
        /// BLE address info for reaching them
        ble_address: Option<BleAddress>,
        /// Their last known advertisement data
        last_advertisement: Option<Vec<u8>>,
        /// Connectivity quality from the introducer's perspective
        quality: ConnectionQuality,
    },
}

/// Information about a peer piece, used inside TopologyUpdate messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    /// The peer's device UUID
    pub device_id: Uuid,
    /// BLE address (if known)
    pub ble_address: Option<BleAddress>,
    /// When we last saw this peer
    pub last_seen: DateTime<Utc>,
    /// The peer's capabilities
    pub capabilities: PieceCapabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer_info() -> PeerInfo {
        PeerInfo {
            device_id: Uuid::new_v4(),
            ble_address: Some(BleAddress::Simulated(Uuid::new_v4())),
            last_seen: Utc::now(),
            capabilities: PieceCapabilities::full(),
        }
    }

    #[test]
    fn test_topology_update_round_trip() {
        let msg = TopologySyncMessage::TopologyUpdate {
            direct_peers: vec![make_peer_info()],
            indirect_peers: vec![make_peer_info(), make_peer_info()],
            topology_hash: 0xDEADBEEF,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let restored: TopologySyncMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn test_peer_introduction_round_trip() {
        let msg = TopologySyncMessage::PeerIntroduction {
            piece_id: Uuid::new_v4(),
            ble_address: Some(BleAddress::Real([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])),
            last_advertisement: Some(vec![0xAA, 0xBB]),
            quality: ConnectionQuality {
                rssi: Some(-55),
                latency_ms: Some(30),
                bandwidth_estimate: None,
            },
        };

        let json = serde_json::to_string(&msg).unwrap();
        let restored: TopologySyncMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn test_topology_update_cbor_round_trip() {
        let msg = TopologySyncMessage::TopologyUpdate {
            direct_peers: vec![make_peer_info()],
            indirect_peers: vec![],
            topology_hash: 42,
        };

        let mut buf = Vec::new();
        ciborium::into_writer(&msg, &mut buf).unwrap();
        let restored: TopologySyncMessage = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn test_peer_introduction_with_and_without_ble_address() {
        let with_addr = TopologySyncMessage::PeerIntroduction {
            piece_id: Uuid::new_v4(),
            ble_address: Some(BleAddress::Simulated(Uuid::new_v4())),
            last_advertisement: None,
            quality: ConnectionQuality::unknown(),
        };

        let without_addr = TopologySyncMessage::PeerIntroduction {
            piece_id: Uuid::new_v4(),
            ble_address: None,
            last_advertisement: None,
            quality: ConnectionQuality::unknown(),
        };

        // Both round-trip through JSON
        for msg in [&with_addr, &without_addr] {
            let json = serde_json::to_string(msg).unwrap();
            let restored: TopologySyncMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &restored);
        }
    }
}
