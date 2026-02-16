//! Runtime ensemble topology — the live connectivity graph
//!
//! Tracks which pieces are online and how they can reach each other.
//! The topology is a directed multigraph: edge A→B doesn't imply B→A
//! (asymmetric BLE connectivity is real), and multiple edges between
//! the same pair are allowed (different transport types).

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The ensemble's connectivity graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsembleTopology {
    /// Known online pieces (device_id -> presence info)
    pub online_pieces: HashMap<Uuid, PiecePresence>,
    /// Directed edges: who can reach whom, via what transport.
    pub edges: Vec<TopologyEdge>,
    /// Our local view timestamp
    pub last_updated: DateTime<Utc>,
}

/// Presence information for a single piece in the ensemble.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PiecePresence {
    pub device_id: Uuid,
    pub last_advertisement: DateTime<Utc>,
    pub last_data_exchange: Option<DateTime<Utc>>,
    pub rssi: Option<i16>,
    pub reachability: PieceReachability,
}

/// How we can reach a piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PieceReachability {
    /// Seen in advertisement but no data path established yet
    AdvertisementOnly,
    /// We have a direct BLE connection to this piece
    Direct,
    /// Reachable through one or more intermediary pieces
    Indirect {
        /// Next hop toward this piece
        next_hop: Uuid,
        /// Estimated hop count
        hop_count: u8,
    },
}

/// A directed edge in the topology multigraph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub transport: TransportType,
    pub quality: ConnectionQuality,
}

/// Transport type for an edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TransportType {
    BleDirect,
    BleRelayed { via: Uuid },
    SimulatedBle,
}

/// Quality metrics for a connection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionQuality {
    pub rssi: Option<i16>,
    pub latency_ms: Option<u32>,
    /// Estimated throughput in bytes/sec
    pub bandwidth_estimate: Option<u32>,
}

impl ConnectionQuality {
    /// Create an unknown quality (all fields None).
    pub fn unknown() -> Self {
        Self {
            rssi: None,
            latency_ms: None,
            bandwidth_estimate: None,
        }
    }
}

/// Stable discriminant for transport type ordering in hash computation.
fn transport_discriminant(t: &TransportType) -> u8 {
    match t {
        TransportType::BleDirect => 0,
        TransportType::BleRelayed { .. } => 1,
        TransportType::SimulatedBle => 2,
    }
}

impl EnsembleTopology {
    /// Create a new empty topology.
    pub fn new() -> Self {
        Self {
            online_pieces: HashMap::new(),
            edges: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Add or update a piece's presence. Returns true if this is a new piece.
    pub fn upsert_piece(&mut self, presence: PiecePresence) -> bool {
        self.last_updated = Utc::now();
        self.online_pieces
            .insert(presence.device_id, presence)
            .is_none()
    }

    /// Remove a piece and all edges involving it. Returns true if the piece existed.
    pub fn remove_piece(&mut self, device_id: &Uuid) -> bool {
        if self.online_pieces.remove(device_id).is_some() {
            self.remove_edges_for(device_id);
            self.last_updated = Utc::now();
            true
        } else {
            false
        }
    }

    /// Get a piece's presence info.
    pub fn get_piece(&self, device_id: &Uuid) -> Option<&PiecePresence> {
        self.online_pieces.get(device_id)
    }

    /// Get a mutable reference to a piece's presence info.
    pub fn get_piece_mut(&mut self, device_id: &Uuid) -> Option<&mut PiecePresence> {
        self.online_pieces.get_mut(device_id)
    }

    /// Add a directed edge.
    pub fn add_edge(&mut self, edge: TopologyEdge) {
        self.edges.push(edge);
        self.last_updated = Utc::now();
    }

    /// Remove all edges from `from` to `to`. Returns the number removed.
    pub fn remove_edges_between(&mut self, from: &Uuid, to: &Uuid) -> usize {
        let before = self.edges.len();
        self.edges.retain(|e| &e.from != from || &e.to != to);
        let removed = before - self.edges.len();
        if removed > 0 {
            self.last_updated = Utc::now();
        }
        removed
    }

    /// Remove all edges involving a specific piece. Returns the number removed.
    pub fn remove_edges_for(&mut self, device_id: &Uuid) -> usize {
        let before = self.edges.len();
        self.edges
            .retain(|e| &e.from != device_id && &e.to != device_id);
        before - self.edges.len()
    }

    /// Get all edges originating from a specific piece.
    pub fn edges_from(&self, device_id: &Uuid) -> Vec<&TopologyEdge> {
        self.edges.iter().filter(|e| &e.from == device_id).collect()
    }

    /// Get all edges targeting a specific piece.
    pub fn edges_to(&self, device_id: &Uuid) -> Vec<&TopologyEdge> {
        self.edges.iter().filter(|e| &e.to == device_id).collect()
    }

    /// Check if `target` is reachable from `source` via directed edges (BFS).
    pub fn is_reachable(&self, source: &Uuid, target: &Uuid) -> bool {
        self.compute_reachability(source, target).is_some()
    }

    /// Compute reachability from `source` to `target`: the next hop and hop count.
    /// Returns None if unreachable.
    pub fn compute_reachability(
        &self,
        source: &Uuid,
        target: &Uuid,
    ) -> Option<PieceReachability> {
        if source == target {
            return Some(PieceReachability::Direct);
        }

        // BFS: queue of (current_node, first_hop_from_source, hop_count)
        let mut visited: HashSet<Uuid> = HashSet::new();
        visited.insert(*source);
        let mut queue: VecDeque<(Uuid, Uuid, u8)> = VecDeque::new();

        // Seed with direct neighbors of source
        for edge in self.edges_from(source) {
            if !visited.contains(&edge.to) {
                visited.insert(edge.to);
                if &edge.to == target {
                    return Some(PieceReachability::Direct);
                }
                queue.push_back((edge.to, edge.to, 1));
            }
        }

        while let Some((current, first_hop, hops)) = queue.pop_front() {
            for edge in self.edges_from(&current) {
                if !visited.contains(&edge.to) {
                    visited.insert(edge.to);
                    if &edge.to == target {
                        return Some(PieceReachability::Indirect {
                            next_hop: first_hop,
                            hop_count: hops + 1,
                        });
                    }
                    queue.push_back((edge.to, first_hop, hops + 1));
                }
            }
        }

        None
    }

    /// Compute a deterministic topology hash for quick comparison in advertisements.
    /// Uses SHA-256 over sorted piece IDs + sorted edges, truncated to u32.
    pub fn topology_hash(&self) -> u32 {
        let mut hasher = Sha256::new();

        // Sort piece IDs for determinism
        let mut piece_ids: Vec<&Uuid> = self.online_pieces.keys().collect();
        piece_ids.sort();
        for id in &piece_ids {
            hasher.update(id.as_bytes());
        }

        // Sort edges for determinism
        let mut sorted_edges: Vec<&TopologyEdge> = self.edges.iter().collect();
        sorted_edges.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then(a.to.cmp(&b.to))
                .then(transport_discriminant(&a.transport).cmp(&transport_discriminant(&b.transport)))
        });
        for edge in &sorted_edges {
            hasher.update(edge.from.as_bytes());
            hasher.update(edge.to.as_bytes());
            hasher.update(&[transport_discriminant(&edge.transport)]);
            if let TransportType::BleRelayed { via } = &edge.transport {
                hasher.update(via.as_bytes());
            }
        }

        let hash = hasher.finalize();
        u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
    }

    /// Number of online pieces.
    pub fn piece_count(&self) -> usize {
        self.online_pieces.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_presence(device_id: Uuid) -> PiecePresence {
        PiecePresence {
            device_id,
            last_advertisement: Utc::now(),
            last_data_exchange: None,
            rssi: None,
            reachability: PieceReachability::AdvertisementOnly,
        }
    }

    fn make_direct_edge(from: Uuid, to: Uuid) -> TopologyEdge {
        TopologyEdge {
            from,
            to,
            transport: TransportType::BleDirect,
            quality: ConnectionQuality::unknown(),
        }
    }

    #[test]
    fn test_new_topology_is_empty() {
        let topo = EnsembleTopology::new();
        assert_eq!(topo.piece_count(), 0);
        assert_eq!(topo.edge_count(), 0);
    }

    #[test]
    fn test_upsert_piece_new() {
        let mut topo = EnsembleTopology::new();
        let id = Uuid::new_v4();
        assert!(topo.upsert_piece(make_presence(id)));
        assert_eq!(topo.piece_count(), 1);
        assert!(topo.get_piece(&id).is_some());
    }

    #[test]
    fn test_upsert_piece_update() {
        let mut topo = EnsembleTopology::new();
        let id = Uuid::new_v4();
        assert!(topo.upsert_piece(make_presence(id)));

        // Upsert again with same device_id — returns false (not new)
        let mut updated = make_presence(id);
        updated.rssi = Some(-60);
        assert!(!topo.upsert_piece(updated));
        assert_eq!(topo.piece_count(), 1);
        assert_eq!(topo.get_piece(&id).unwrap().rssi, Some(-60));
    }

    #[test]
    fn test_remove_piece() {
        let mut topo = EnsembleTopology::new();
        let id = Uuid::new_v4();
        topo.upsert_piece(make_presence(id));
        assert!(topo.remove_piece(&id));
        assert_eq!(topo.piece_count(), 0);

        // Removing nonexistent returns false
        assert!(!topo.remove_piece(&Uuid::new_v4()));
    }

    #[test]
    fn test_remove_piece_removes_associated_edges() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        topo.upsert_piece(make_presence(a));
        topo.upsert_piece(make_presence(b));
        topo.upsert_piece(make_presence(c));

        topo.add_edge(make_direct_edge(a, b));
        topo.add_edge(make_direct_edge(b, c));
        topo.add_edge(make_direct_edge(c, a));
        assert_eq!(topo.edge_count(), 3);

        topo.remove_piece(&b);
        // Both a->b and b->c should be gone
        assert_eq!(topo.edge_count(), 1);
        assert_eq!(topo.edges[0].from, c);
        assert_eq!(topo.edges[0].to, a);
    }

    #[test]
    fn test_add_edge() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        assert_eq!(topo.edge_count(), 1);
        assert_eq!(topo.edges_from(&a).len(), 1);
        assert_eq!(topo.edges_to(&b).len(), 1);
    }

    #[test]
    fn test_remove_edges_between() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        topo.add_edge(TopologyEdge {
            from: a,
            to: b,
            transport: TransportType::SimulatedBle,
            quality: ConnectionQuality::unknown(),
        });
        topo.add_edge(make_direct_edge(a, c));

        let removed = topo.remove_edges_between(&a, &b);
        assert_eq!(removed, 2);
        assert_eq!(topo.edge_count(), 1); // a->c remains
    }

    #[test]
    fn test_is_reachable_direct() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        assert!(topo.is_reachable(&a, &b));
    }

    #[test]
    fn test_is_reachable_indirect() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        topo.add_edge(make_direct_edge(b, c));
        assert!(topo.is_reachable(&a, &c));
    }

    #[test]
    fn test_is_not_reachable_wrong_direction() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        // Directed: b cannot reach a
        assert!(!topo.is_reachable(&b, &a));
    }

    #[test]
    fn test_compute_reachability_direct() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));

        let reach = topo.compute_reachability(&a, &b).unwrap();
        assert_eq!(reach, PieceReachability::Direct);
    }

    #[test]
    fn test_compute_reachability_indirect() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        topo.add_edge(make_direct_edge(a, b));
        topo.add_edge(make_direct_edge(b, c));

        let reach = topo.compute_reachability(&a, &c).unwrap();
        assert_eq!(
            reach,
            PieceReachability::Indirect {
                next_hop: b,
                hop_count: 2,
            }
        );
    }

    #[test]
    fn test_compute_reachability_unreachable() {
        let mut topo = EnsembleTopology::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        topo.upsert_piece(make_presence(a));
        topo.upsert_piece(make_presence(b));
        // No edges — unreachable
        assert!(topo.compute_reachability(&a, &b).is_none());
    }

    #[test]
    fn test_topology_hash_deterministic() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        let mut topo1 = EnsembleTopology::new();
        topo1.upsert_piece(make_presence(a));
        topo1.upsert_piece(make_presence(b));
        topo1.add_edge(make_direct_edge(a, b));

        let mut topo2 = EnsembleTopology::new();
        topo2.upsert_piece(make_presence(a));
        topo2.upsert_piece(make_presence(b));
        topo2.add_edge(make_direct_edge(a, b));

        assert_eq!(topo1.topology_hash(), topo2.topology_hash());
    }

    #[test]
    fn test_topology_hash_order_independent() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        // Build topology in one order
        let mut topo1 = EnsembleTopology::new();
        topo1.upsert_piece(make_presence(a));
        topo1.upsert_piece(make_presence(b));
        topo1.upsert_piece(make_presence(c));
        topo1.add_edge(make_direct_edge(a, b));
        topo1.add_edge(make_direct_edge(b, c));

        // Build same topology in different order
        let mut topo2 = EnsembleTopology::new();
        topo2.upsert_piece(make_presence(c));
        topo2.upsert_piece(make_presence(a));
        topo2.upsert_piece(make_presence(b));
        topo2.add_edge(make_direct_edge(b, c));
        topo2.add_edge(make_direct_edge(a, b));

        assert_eq!(topo1.topology_hash(), topo2.topology_hash());
    }

    #[test]
    fn test_topology_hash_changes_on_mutation() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        let mut topo = EnsembleTopology::new();
        topo.upsert_piece(make_presence(a));
        let hash1 = topo.topology_hash();

        topo.upsert_piece(make_presence(b));
        let hash2 = topo.topology_hash();
        assert_ne!(hash1, hash2);

        topo.add_edge(make_direct_edge(a, b));
        let hash3 = topo.topology_hash();
        assert_ne!(hash2, hash3);
    }
}
