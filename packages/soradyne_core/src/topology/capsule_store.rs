//! On-disk persistence for capsules
//!
//! Stores capsules as individual `{capsule_id}.json` files in a directory.
//! Maintains an in-memory cache for fast access.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::identity::CapsuleKeyBundle;

use super::capsule::{Capsule, PieceRecord, FlowConfig};
use super::TopologyError;

/// Write a capsule to disk as `{id}.json` in the given directory.
fn write_capsule(storage_path: &Path, capsule: &Capsule) -> Result<(), TopologyError> {
    std::fs::create_dir_all(storage_path)
        .map_err(|e| TopologyError::IoError(e.to_string()))?;

    let path = storage_path.join(format!("{}.json", capsule.id));
    let json = serde_json::to_string_pretty(capsule)
        .map_err(|e| TopologyError::SerializationError(e.to_string()))?;

    std::fs::write(path, json).map_err(|e| TopologyError::IoError(e.to_string()))?;
    Ok(())
}

/// Persistent store for capsules.
pub struct CapsuleStore {
    storage_path: PathBuf,
    capsules: HashMap<Uuid, Capsule>,
}

impl CapsuleStore {
    /// Create a new store with an empty cache (does not load from disk).
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            storage_path,
            capsules: HashMap::new(),
        }
    }

    /// Load all capsules from the storage directory.
    pub fn load(storage_path: &Path) -> Result<Self, TopologyError> {
        let mut capsules = HashMap::new();

        if storage_path.exists() {
            let entries = std::fs::read_dir(storage_path)
                .map_err(|e| TopologyError::IoError(e.to_string()))?;

            for entry in entries {
                let entry = entry.map_err(|e| TopologyError::IoError(e.to_string()))?;
                let path = entry.path();

                if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    let data = std::fs::read(&path)
                        .map_err(|e| TopologyError::IoError(e.to_string()))?;
                    let capsule: Capsule = serde_json::from_slice(&data)
                        .map_err(|e| TopologyError::DeserializationError(e.to_string()))?;
                    capsules.insert(capsule.id, capsule);
                }
            }
        }

        Ok(Self {
            storage_path: storage_path.to_path_buf(),
            capsules,
        })
    }

    /// Write a single capsule to disk as `{id}.json`.
    pub fn save_capsule(&self, capsule: &Capsule) -> Result<(), TopologyError> {
        write_capsule(&self.storage_path, capsule)
    }

    /// Create a new capsule, insert into cache, save to disk, return its id.
    pub fn create_capsule(
        &mut self,
        name: &str,
        keys: CapsuleKeyBundle,
    ) -> Result<Uuid, TopologyError> {
        let capsule = Capsule::new(name.to_string(), keys);
        let id = capsule.id;
        self.save_capsule(&capsule)?;
        self.capsules.insert(id, capsule);
        Ok(id)
    }

    /// Add a piece to a capsule. Saves to disk. Returns whether the piece was added.
    pub fn add_piece(
        &mut self,
        capsule_id: &Uuid,
        piece: PieceRecord,
    ) -> Result<bool, TopologyError> {
        let capsule = self
            .capsules
            .get_mut(capsule_id)
            .ok_or_else(|| TopologyError::NotFound(format!("capsule {}", capsule_id)))?;

        let added = capsule.add_piece(piece);
        if added {
            write_capsule(&self.storage_path, capsule)?;
        }
        Ok(added)
    }

    /// Add a flow to a capsule. Saves to disk. Returns whether the flow was added.
    pub fn add_flow(
        &mut self,
        capsule_id: &Uuid,
        flow: FlowConfig,
    ) -> Result<bool, TopologyError> {
        let capsule = self
            .capsules
            .get_mut(capsule_id)
            .ok_or_else(|| TopologyError::NotFound(format!("capsule {}", capsule_id)))?;

        let added = capsule.add_flow(flow);
        if added {
            write_capsule(&self.storage_path, capsule)?;
        }
        Ok(added)
    }

    /// Get a capsule by id.
    pub fn get_capsule(&self, capsule_id: &Uuid) -> Option<&Capsule> {
        self.capsules.get(capsule_id)
    }

    /// List all capsules.
    pub fn list_capsules(&self) -> Vec<&Capsule> {
        self.capsules.values().collect()
    }

    /// Retire a capsule. Saves to disk.
    pub fn retire_capsule(&mut self, capsule_id: &Uuid) -> Result<(), TopologyError> {
        let capsule = self
            .capsules
            .get_mut(capsule_id)
            .ok_or_else(|| TopologyError::NotFound(format!("capsule {}", capsule_id)))?;

        capsule.retire();
        write_capsule(&self.storage_path, capsule)?;
        Ok(())
    }

    /// Merge a peer's capsule data into our local copy.
    /// Returns NotFound if we don't have this capsule. Returns whether anything changed.
    pub fn merge_from_peer(&mut self, peer_capsule: &Capsule) -> Result<bool, TopologyError> {
        let capsule = self
            .capsules
            .get_mut(&peer_capsule.id)
            .ok_or_else(|| TopologyError::NotFound(format!("capsule {}", peer_capsule.id)))?;

        let pieces_before = capsule.pieces.len();
        let flows_before = capsule.flows.len();

        capsule.merge(peer_capsule);

        let changed =
            capsule.pieces.len() != pieces_before || capsule.flows.len() != flows_before;

        if changed {
            write_capsule(&self.storage_path, capsule)?;
        }
        Ok(changed)
    }

    /// Get key bundles for all active capsules (convenience for BLE advertisement decryption).
    pub fn keys_for_all_capsules(&self) -> Vec<CapsuleKeyBundle> {
        self.capsules
            .values()
            .filter(|c| c.is_active())
            .map(|c| c.keys.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DeviceIdentity;
    use crate::topology::capsule::{PieceCapabilities, PieceRole};
    use chrono::Utc;

    fn make_test_keys() -> CapsuleKeyBundle {
        CapsuleKeyBundle::generate(Uuid::new_v4())
    }

    fn make_test_piece(name: &str) -> PieceRecord {
        let identity = DeviceIdentity::generate();
        PieceRecord::from_identity(
            &identity,
            name.to_string(),
            PieceCapabilities::full(),
            PieceRole::Full,
        )
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
    fn test_create_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let keys = make_test_keys();
        let capsule_id;
        {
            let mut store = CapsuleStore::new(path.clone());
            capsule_id = store.create_capsule("test capsule", keys).unwrap();
            assert!(store.get_capsule(&capsule_id).is_some());
        }

        // Load from same path — capsule should be there
        let store = CapsuleStore::load(&path).unwrap();
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert_eq!(capsule.name, "test capsule");
    }

    #[test]
    fn test_add_piece_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let keys = make_test_keys();
        let piece = make_test_piece("phone");
        let device_id = piece.device_id;

        let capsule_id;
        {
            let mut store = CapsuleStore::new(path.clone());
            capsule_id = store.create_capsule("test", keys).unwrap();
            store.add_piece(&capsule_id, piece).unwrap();
        }

        let store = CapsuleStore::load(&path).unwrap();
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert_eq!(capsule.pieces.len(), 1);
        assert!(capsule.find_piece(&device_id).is_some());
    }

    #[test]
    fn test_merge_from_peer_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let keys = make_test_keys();
        let capsule_id;
        {
            let mut store = CapsuleStore::new(path.clone());
            capsule_id = store.create_capsule("test", keys.clone()).unwrap();

            // Simulate a peer's version of the same capsule with an extra piece
            let mut peer_capsule = Capsule::new("test".to_string(), keys);
            peer_capsule.add_piece(make_test_piece("peer phone"));

            let changed = store.merge_from_peer(&peer_capsule).unwrap();
            assert!(changed);
        }

        let store = CapsuleStore::load(&path).unwrap();
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert_eq!(capsule.pieces.len(), 1);
    }

    #[test]
    fn test_retire_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let keys = make_test_keys();
        let capsule_id;
        {
            let mut store = CapsuleStore::new(path.clone());
            capsule_id = store.create_capsule("test", keys).unwrap();
            store.retire_capsule(&capsule_id).unwrap();
        }

        let store = CapsuleStore::load(&path).unwrap();
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert!(!capsule.is_active());
    }

    #[test]
    fn test_list_capsules() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let mut store = CapsuleStore::new(path);
        store.create_capsule("one", make_test_keys()).unwrap();
        store.create_capsule("two", make_test_keys()).unwrap();
        store.create_capsule("three", make_test_keys()).unwrap();

        assert_eq!(store.list_capsules().len(), 3);
    }

    #[test]
    fn test_merge_unknown_capsule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let mut store = CapsuleStore::new(path);
        let unknown_capsule = Capsule::new("unknown".to_string(), make_test_keys());

        let result = store.merge_from_peer(&unknown_capsule);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not found"));
    }

    #[test]
    fn test_keys_for_all_capsules() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let mut store = CapsuleStore::new(path);
        let id1 = store.create_capsule("active1", make_test_keys()).unwrap();
        let _id2 = store.create_capsule("active2", make_test_keys()).unwrap();
        let id3 = store.create_capsule("retired", make_test_keys()).unwrap();

        store.retire_capsule(&id3).unwrap();

        let keys = store.keys_for_all_capsules();
        // Only active capsules
        assert_eq!(keys.len(), 2);
        // Verify the retired capsule's keys are not included
        let active_ids: Vec<Uuid> = keys.iter().map(|k| k.capsule_id).collect();
        assert!(active_ids.contains(&id1));
        assert!(!active_ids.contains(&id3));
    }

    #[test]
    fn test_add_flow_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");

        let keys = make_test_keys();
        let flow = make_test_flow("giantt");
        let flow_id = flow.id;

        let capsule_id;
        {
            let mut store = CapsuleStore::new(path.clone());
            capsule_id = store.create_capsule("test", keys).unwrap();
            store.add_flow(&capsule_id, flow).unwrap();
        }

        let store = CapsuleStore::load(&path).unwrap();
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert_eq!(capsule.flows.len(), 1);
        assert_eq!(capsule.flows[0].id, flow_id);
    }
}
