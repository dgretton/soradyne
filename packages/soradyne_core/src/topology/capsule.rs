//! Capsule and Piece data model types
//!
//! A capsule is a group of devices (pieces) that share key material and synchronize
//! data flows. Pieces are the individual devices within a capsule.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::{CapsuleKeyBundle, DeviceIdentity};

use super::TopologyError;

/// Role of a piece within a capsule.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum PieceRole {
    /// Runs Soradyne core, participates in topology
    Full,
    /// Minimal interface, singular role
    Accessory,
}

/// Capabilities of a piece.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PieceCapabilities {
    pub can_host_drip: bool,
    pub can_memorize: bool,
    pub can_route: bool,
    pub has_ui: bool,
    pub storage_bytes: u64,
    pub battery_aware: bool,
}

impl PieceCapabilities {
    /// Convenience constructor for a full-capability piece.
    pub fn full() -> Self {
        Self {
            can_host_drip: true,
            can_memorize: true,
            can_route: true,
            has_ui: true,
            storage_bytes: 0,
            battery_aware: false,
        }
    }

    /// Convenience constructor for an accessory piece (can_memorize + can_route only).
    pub fn accessory() -> Self {
        Self {
            can_host_drip: false,
            can_memorize: true,
            can_route: true,
            has_ui: false,
            storage_bytes: 0,
            battery_aware: false,
        }
    }
}

/// A record of a piece (device) within a capsule.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PieceRecord {
    pub device_id: Uuid,
    pub name: String,
    /// Ed25519 public key
    pub verifying_key: [u8; 32],
    /// X25519 public key
    pub dh_public_key: [u8; 32],
    pub added_at: DateTime<Utc>,
    pub capabilities: PieceCapabilities,
    pub role: PieceRole,
}

impl PieceRecord {
    /// Construct a PieceRecord from a DeviceIdentity, filling in keys and timestamp.
    pub fn from_identity(
        identity: &DeviceIdentity,
        name: String,
        capabilities: PieceCapabilities,
        role: PieceRole,
    ) -> Self {
        Self {
            device_id: identity.device_id(),
            name,
            verifying_key: identity.verifying_key_bytes(),
            dh_public_key: identity.dh_public_bytes(),
            added_at: Utc::now(),
            capabilities,
            role,
        }
    }
}

/// Status of a capsule.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CapsuleStatus {
    Active,
    Retired { retired_at: DateTime<Utc> },
}

/// Minimal flow configuration placeholder.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FlowConfig {
    /// Flow identifier
    pub id: Uuid,
    /// Human-friendly name
    pub name: String,
    /// Identifies the flow's data type (e.g. "giantt", "inventory")
    pub schema_type: String,
    pub created_at: DateTime<Utc>,
}

/// A capsule: a group of pieces that share key material and sync data flows.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Capsule {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    /// Add-only set of pieces
    pub pieces: Vec<PieceRecord>,
    /// Add-only set of flow configurations
    pub flows: Vec<FlowConfig>,
    pub keys: CapsuleKeyBundle,
    pub status: CapsuleStatus,
}

impl Capsule {
    /// Create a new capsule with empty pieces/flows and Active status.
    pub fn new(name: String, keys: CapsuleKeyBundle) -> Self {
        Self {
            id: keys.capsule_id,
            name,
            created_at: Utc::now(),
            pieces: Vec::new(),
            flows: Vec::new(),
            keys,
            status: CapsuleStatus::Active,
        }
    }

    /// Add a piece if its device_id is not already present. Returns whether added.
    pub fn add_piece(&mut self, piece: PieceRecord) -> bool {
        if self.pieces.iter().any(|p| p.device_id == piece.device_id) {
            return false;
        }
        self.pieces.push(piece);
        true
    }

    /// Add a flow if its id is not already present. Returns whether added.
    pub fn add_flow(&mut self, flow: FlowConfig) -> bool {
        if self.flows.iter().any(|f| f.id == flow.id) {
            return false;
        }
        self.flows.push(flow);
        true
    }

    /// Find a piece by device_id.
    pub fn find_piece(&self, device_id: &Uuid) -> Option<&PieceRecord> {
        self.pieces.iter().find(|p| &p.device_id == device_id)
    }

    /// Set-union merge of pieces and flows from another capsule with the same id.
    /// Only merges if `other.id == self.id`. Does NOT merge keys or status.
    pub fn merge(&mut self, other: &Capsule) {
        if other.id != self.id {
            return;
        }
        for piece in &other.pieces {
            if !self.pieces.iter().any(|p| p.device_id == piece.device_id) {
                self.pieces.push(piece.clone());
            }
        }
        for flow in &other.flows {
            if !self.flows.iter().any(|f| f.id == flow.id) {
                self.flows.push(flow.clone());
            }
        }
    }

    /// Retire this capsule.
    pub fn retire(&mut self) {
        self.status = CapsuleStatus::Retired {
            retired_at: Utc::now(),
        };
    }

    /// Check if this capsule is active.
    pub fn is_active(&self) -> bool {
        matches!(self.status, CapsuleStatus::Active)
    }

    /// Serialize to CBOR bytes for BLE gossip transfer.
    pub fn to_gossip_bytes(&self) -> Result<Vec<u8>, TopologyError> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| TopologyError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    /// Deserialize from CBOR gossip bytes.
    pub fn from_gossip_bytes(data: &[u8]) -> Result<Self, TopologyError> {
        ciborium::from_reader(data)
            .map_err(|e| TopologyError::DeserializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_capsule(name: &str) -> Capsule {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        Capsule::new(name.to_string(), keys)
    }

    fn make_test_piece(name: &str) -> PieceRecord {
        let identity = DeviceIdentity::generate();
        PieceRecord::from_identity(&identity, name.to_string(), PieceCapabilities::full(), PieceRole::Full)
    }

    fn make_test_flow(name: &str) -> FlowConfig {
        FlowConfig {
            id: Uuid::new_v4(),
            name: name.to_string(),
            schema_type: "test".to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = make_test_capsule("test capsule");
        assert!(capsule.pieces.is_empty());
        assert!(capsule.flows.is_empty());
        assert!(capsule.is_active());
        assert_eq!(capsule.name, "test capsule");
    }

    #[test]
    fn test_add_piece() {
        let mut capsule = make_test_capsule("test");
        let piece = make_test_piece("phone");

        assert!(capsule.add_piece(piece.clone()));
        assert_eq!(capsule.pieces.len(), 1);

        // Duplicate device_id returns false
        assert!(!capsule.add_piece(piece));
        assert_eq!(capsule.pieces.len(), 1);
    }

    #[test]
    fn test_add_flow() {
        let mut capsule = make_test_capsule("test");
        let flow = make_test_flow("giantt");

        assert!(capsule.add_flow(flow.clone()));
        assert_eq!(capsule.flows.len(), 1);

        // Duplicate flow id returns false
        assert!(!capsule.add_flow(flow));
        assert_eq!(capsule.flows.len(), 1);
    }

    #[test]
    fn test_merge_pieces() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let mut capsule_a = Capsule::new("a".to_string(), keys.clone());
        let mut capsule_b = Capsule::new("b".to_string(), keys);

        let piece1 = make_test_piece("phone");
        let piece2 = make_test_piece("laptop");

        capsule_a.add_piece(piece1);
        capsule_b.add_piece(piece2);

        capsule_a.merge(&capsule_b);
        assert_eq!(capsule_a.pieces.len(), 2);

        capsule_b.merge(&capsule_a);
        assert_eq!(capsule_b.pieces.len(), 2);
    }

    #[test]
    fn test_merge_flows() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let mut capsule_a = Capsule::new("a".to_string(), keys.clone());
        let mut capsule_b = Capsule::new("b".to_string(), keys);

        let flow1 = make_test_flow("giantt");
        let flow2 = make_test_flow("inventory");

        capsule_a.add_flow(flow1);
        capsule_b.add_flow(flow2);

        capsule_a.merge(&capsule_b);
        assert_eq!(capsule_a.flows.len(), 2);

        capsule_b.merge(&capsule_a);
        assert_eq!(capsule_b.flows.len(), 2);
    }

    #[test]
    fn test_merge_idempotent() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let mut capsule_a = Capsule::new("a".to_string(), keys.clone());
        let capsule_b = Capsule::new("b".to_string(), keys);

        let piece = make_test_piece("phone");
        capsule_a.add_piece(piece.clone());

        // Clone before merge to capture state
        let mut capsule_a_copy = capsule_a.clone();
        capsule_a_copy.merge(&capsule_b);
        capsule_a_copy.merge(&capsule_b);

        assert_eq!(capsule_a_copy.pieces.len(), 1);
    }

    #[test]
    fn test_merge_wrong_capsule_id() {
        let mut capsule_a = make_test_capsule("a");
        let capsule_b = make_test_capsule("b");

        let piece = make_test_piece("phone");
        let mut capsule_b_with_piece = capsule_b;
        capsule_b_with_piece.add_piece(piece);

        capsule_a.merge(&capsule_b_with_piece);
        // Different capsule IDs — nothing should merge
        assert!(capsule_a.pieces.is_empty());
    }

    #[test]
    fn test_piece_from_identity() {
        let identity = DeviceIdentity::generate();
        let piece = PieceRecord::from_identity(
            &identity,
            "my phone".to_string(),
            PieceCapabilities::full(),
            PieceRole::Full,
        );

        assert_eq!(piece.device_id, identity.device_id());
        assert_eq!(piece.verifying_key, identity.verifying_key_bytes());
        assert_eq!(piece.dh_public_key, identity.dh_public_bytes());
        assert_eq!(piece.name, "my phone");
        assert_eq!(piece.role, PieceRole::Full);
    }

    #[test]
    fn test_retire() {
        let mut capsule = make_test_capsule("test");
        assert!(capsule.is_active());

        capsule.retire();
        assert!(!capsule.is_active());
        assert!(matches!(capsule.status, CapsuleStatus::Retired { .. }));
    }

    #[test]
    fn test_gossip_round_trip() {
        let mut capsule = make_test_capsule("gossip test");
        capsule.add_piece(make_test_piece("phone"));
        capsule.add_piece(make_test_piece("laptop"));
        capsule.add_flow(make_test_flow("giantt"));

        let bytes = capsule.to_gossip_bytes().unwrap();
        let restored = Capsule::from_gossip_bytes(&bytes).unwrap();

        assert_eq!(restored.id, capsule.id);
        assert_eq!(restored.name, capsule.name);
        assert_eq!(restored.pieces.len(), 2);
        assert_eq!(restored.flows.len(), 1);
        assert_eq!(restored.keys.capsule_id, capsule.keys.capsule_id);
        assert_eq!(restored.keys.advertisement_key, capsule.keys.advertisement_key);
        assert!(restored.is_active());
    }

    #[test]
    fn test_find_piece() {
        let mut capsule = make_test_capsule("test");
        let piece = make_test_piece("phone");
        let device_id = piece.device_id;
        capsule.add_piece(piece);

        assert!(capsule.find_piece(&device_id).is_some());
        assert_eq!(capsule.find_piece(&device_id).unwrap().name, "phone");

        let unknown_id = Uuid::new_v4();
        assert!(capsule.find_piece(&unknown_id).is_none());
    }
}
