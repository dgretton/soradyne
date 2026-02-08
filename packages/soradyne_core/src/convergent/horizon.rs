//! Causal context tracking via Horizons
//!
//! A Horizon records what operations a device had seen when it performed an action.
//! This enables informed-remove semantics: removes only affect states the remover knew about.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unique identifier for a device/replica
pub type DeviceId = String;

/// Sequence number within a device's operation stream
pub type SeqNum = u64;

/// A Horizon captures the causal context of an operation.
///
/// It maps each known device to the highest sequence number seen from that device.
/// When comparing operations, we can determine if one "happened before" another,
/// or if they were concurrent (neither knew about the other).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Horizon {
    /// Map from device ID to the highest sequence number seen from that device
    seen: BTreeMap<DeviceId, SeqNum>,
}

impl Horizon {
    /// Create an empty horizon (knows nothing)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a horizon that has seen up to seq from device
    pub fn at(device: DeviceId, seq: SeqNum) -> Self {
        let mut h = Self::new();
        h.seen.insert(device, seq);
        h
    }

    /// Get the sequence number seen from a device (0 if never seen)
    pub fn get(&self, device: &DeviceId) -> SeqNum {
        self.seen.get(device).copied().unwrap_or(0)
    }

    /// Record having seen an operation from a device
    pub fn observe(&mut self, device: &DeviceId, seq: SeqNum) {
        let current = self.seen.entry(device.clone()).or_insert(0);
        if seq > *current {
            *current = seq;
        }
    }

    /// Merge another horizon into this one (take max of each device)
    pub fn merge(&mut self, other: &Horizon) {
        for (device, seq) in &other.seen {
            self.observe(device, *seq);
        }
    }

    /// Check if this horizon has seen a specific operation
    pub fn has_seen(&self, device: &DeviceId, seq: SeqNum) -> bool {
        self.get(device) >= seq
    }

    /// Check if this horizon dominates another (has seen everything other has seen)
    pub fn dominates(&self, other: &Horizon) -> bool {
        other.seen.iter().all(|(d, s)| self.get(d) >= *s)
    }

    /// Check if two horizons are concurrent (neither dominates the other)
    pub fn is_concurrent_with(&self, other: &Horizon) -> bool {
        !self.dominates(other) && !other.dominates(self)
    }

    /// Get all devices this horizon knows about
    pub fn devices(&self) -> impl Iterator<Item = &DeviceId> {
        self.seen.keys()
    }

    /// Compute a deterministic hash for state comparison
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        for (device, seq) in &self.seen {
            device.hash(&mut hasher);
            seq.hash(&mut hasher);
        }
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizon_observe_and_get() {
        let mut h = Horizon::new();
        assert_eq!(h.get(&"A".into()), 0);

        h.observe(&"A".into(), 5);
        assert_eq!(h.get(&"A".into()), 5);

        // Doesn't go backwards
        h.observe(&"A".into(), 3);
        assert_eq!(h.get(&"A".into()), 5);

        h.observe(&"A".into(), 7);
        assert_eq!(h.get(&"A".into()), 7);
    }

    #[test]
    fn test_horizon_dominates() {
        let mut h1 = Horizon::new();
        h1.observe(&"A".into(), 5);
        h1.observe(&"B".into(), 3);

        let mut h2 = Horizon::new();
        h2.observe(&"A".into(), 3);
        h2.observe(&"B".into(), 2);

        assert!(h1.dominates(&h2));
        assert!(!h2.dominates(&h1));
    }

    #[test]
    fn test_horizon_concurrent() {
        let mut h1 = Horizon::new();
        h1.observe(&"A".into(), 5);
        h1.observe(&"B".into(), 2);

        let mut h2 = Horizon::new();
        h2.observe(&"A".into(), 3);
        h2.observe(&"B".into(), 4);

        // Neither dominates: h1 ahead on A, h2 ahead on B
        assert!(h1.is_concurrent_with(&h2));
    }
}
