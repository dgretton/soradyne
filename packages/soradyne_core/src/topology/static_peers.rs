//! On-disk storage for static peer addresses.
//!
//! Static peers are devices reachable at known IP:port addresses (e.g. via
//! Tailscale or other VPN) where mDNS/BLE discovery isn't available.
//!
//! Stored in `static_peers.json` alongside the capsule store, with the format:
//! ```json
//! {
//!   "<capsule-uuid>": {
//!     "<peer-device-uuid>": "ip:port"
//!   }
//! }
//! ```
//!
//! These addresses are local configuration — each device stores the addresses
//! *it* uses to reach its peers, which may differ by network context.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::TopologyError;

/// In-memory representation of static peer addresses.
pub struct StaticPeerConfig {
    file_path: PathBuf,
    /// capsule_id → (peer_device_id → socket_addr)
    entries: HashMap<Uuid, HashMap<Uuid, SocketAddr>>,
}

impl StaticPeerConfig {
    /// Load static peer config from the given directory.
    ///
    /// If the file doesn't exist, returns an empty config.
    pub fn load(dir: &Path) -> Result<Self, TopologyError> {
        let file_path = dir.join("static_peers.json");

        let entries = if file_path.exists() {
            let data = std::fs::read_to_string(&file_path)
                .map_err(|e| TopologyError::IoError(e.to_string()))?;
            // Deserialize as string-keyed maps, then parse UUIDs
            let raw: HashMap<String, HashMap<String, String>> = serde_json::from_str(&data)
                .map_err(|e| TopologyError::DeserializationError(e.to_string()))?;

            let mut entries = HashMap::new();
            for (capsule_str, peers) in raw {
                let capsule_id = Uuid::parse_str(&capsule_str)
                    .map_err(|e| TopologyError::DeserializationError(e.to_string()))?;
                let mut peer_map = HashMap::new();
                for (peer_str, addr_str) in peers {
                    let peer_id = Uuid::parse_str(&peer_str)
                        .map_err(|e| TopologyError::DeserializationError(e.to_string()))?;
                    let addr: SocketAddr = addr_str
                        .parse()
                        .map_err(|e: std::net::AddrParseError| {
                            TopologyError::DeserializationError(e.to_string())
                        })?;
                    peer_map.insert(peer_id, addr);
                }
                entries.insert(capsule_id, peer_map);
            }
            entries
        } else {
            HashMap::new()
        };

        Ok(Self { file_path, entries })
    }

    /// Get static peers for a capsule.
    pub fn get(&self, capsule_id: &Uuid) -> HashMap<Uuid, SocketAddr> {
        self.entries.get(capsule_id).cloned().unwrap_or_default()
    }

    /// Set a static peer address for a capsule. Saves to disk.
    pub fn set_peer(
        &mut self,
        capsule_id: Uuid,
        peer_id: Uuid,
        addr: SocketAddr,
    ) -> Result<(), TopologyError> {
        self.entries
            .entry(capsule_id)
            .or_default()
            .insert(peer_id, addr);
        self.save()
    }

    /// Remove a static peer address. Saves to disk.
    pub fn remove_peer(
        &mut self,
        capsule_id: &Uuid,
        peer_id: &Uuid,
    ) -> Result<(), TopologyError> {
        if let Some(peers) = self.entries.get_mut(capsule_id) {
            peers.remove(peer_id);
            if peers.is_empty() {
                self.entries.remove(capsule_id);
            }
        }
        self.save()
    }

    fn save(&self) -> Result<(), TopologyError> {
        // Convert to string-keyed maps for JSON
        let raw: HashMap<String, HashMap<String, String>> = self
            .entries
            .iter()
            .map(|(capsule_id, peers)| {
                let peer_map: HashMap<String, String> = peers
                    .iter()
                    .map(|(peer_id, addr)| (peer_id.to_string(), addr.to_string()))
                    .collect();
                (capsule_id.to_string(), peer_map)
            })
            .collect();

        let json = serde_json::to_string_pretty(&raw)
            .map_err(|e| TopologyError::SerializationError(e.to_string()))?;

        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| TopologyError::IoError(e.to_string()))?;
        }

        std::fs::write(&self.file_path, json)
            .map_err(|e| TopologyError::IoError(e.to_string()))?;
        Ok(())
    }
}
