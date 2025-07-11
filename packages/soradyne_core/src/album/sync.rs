//! Synchronization utilities for albums

use super::crdt::{CrdtError, ReplicaId, LogicalTime, CrdtCollection};
use super::album::{MediaAlbum, AlbumMetadata};
use super::operations::{EditOp, MediaId, MediaType};
use crate::storage::block_manager::BlockManager;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

// === Sync Message Types ===

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Request sync for a specific album
    SyncRequest {
        album_id: String,
        last_seen: LogicalTime,
    },
    
    /// Response with album updates
    SyncResponse {
        album_id: String,
        album_data: Option<Vec<u8>>, // Serialized album if full sync
        incremental_ops: Vec<(MediaId, Vec<EditOp>)>, // Per-media ops if incremental
    },
    
    /// Announce new album
    AlbumAnnouncement {
        album_id: String,
        title: String,
        created_by: ReplicaId,
    },
    
    /// Request media data
    MediaRequest {
        block_id: [u8; 32],
    },
    
    /// Response with media data
    MediaResponse {
        block_id: [u8; 32],
        data: Vec<u8>,
    },
}

// === Album Sync Manager ===

pub struct AlbumSyncManager {
    albums: std::collections::HashMap<String, MediaAlbum>,
    block_manager: Arc<BlockManager>,
    replica_id: ReplicaId,
}

impl AlbumSyncManager {
    pub fn new(block_manager: Arc<BlockManager>, replica_id: ReplicaId) -> Self {
        Self {
            albums: std::collections::HashMap::new(),
            block_manager,
            replica_id,
        }
    }
    
    /// Create a new album
    pub fn create_album(&mut self, title: String) -> Result<String, CrdtError> {
        let album_id = uuid::Uuid::new_v4().to_string();
        let album = MediaAlbum::new(album_id.clone(), title, self.replica_id.clone())
            .with_block_manager(self.block_manager.clone());
        
        self.albums.insert(album_id.clone(), album);
        Ok(album_id)
    }
    
    /// Get an album
    pub fn get_album(&self, album_id: &str) -> Option<&MediaAlbum> {
        self.albums.get(album_id)
    }
    
    /// Get a mutable album
    pub fn get_album_mut(&mut self, album_id: &str) -> Option<&mut MediaAlbum> {
        self.albums.get_mut(album_id)
    }
    
    /// Add media to an album
    pub async fn add_media_to_album(
        &mut self, 
        album_id: &str, 
        media_id: MediaId,
        media_data: &[u8],
        media_type: MediaType,
        filename: String
    ) -> Result<(), CrdtError> {
        // Store media in block system
        let block_file = crate::storage::block_file::BlockFile::new(self.block_manager.clone());
        block_file.append(media_data).await.map_err(CrdtError::Flow)?;
        
        let block_id = block_file.root_block().await
            .ok_or_else(|| CrdtError::InvalidOperation("Failed to get block ID".to_string()))?;
        
        // Create set_media operation
        let op = EditOp::set_media(
            self.replica_id.clone(),
            block_id,
            media_type,
            filename,
            media_data.len()
        );
        
        // Apply to album
        if let Some(album) = self.albums.get_mut(album_id) {
            album.apply_to_item(&media_id, op)?;
        }
        
        Ok(())
    }
    
    /// Apply an operation to a media item
    pub fn apply_operation(
        &mut self, 
        album_id: &str, 
        media_id: &MediaId, 
        op: EditOp
    ) -> Result<(), CrdtError> {
        if let Some(album) = self.albums.get_mut(album_id) {
            album.apply_to_item(media_id, op)
        } else {
            Err(CrdtError::InvalidOperation(format!("Album {} not found", album_id)))
        }
    }
    
    /// Merge an album from another replica
    pub fn merge_album(&mut self, other_album: MediaAlbum) -> Result<(), CrdtError> {
        let album_id = other_album.album_id.clone();
        
        if let Some(existing_album) = self.albums.get_mut(&album_id) {
            existing_album.merge_collection(&other_album)?;
        } else {
            self.albums.insert(album_id, other_album);
        }
        
        Ok(())
    }
    
    /// Generate sync message for an album
    pub fn generate_sync_request(&self, album_id: &str, last_seen: LogicalTime) -> SyncMessage {
        SyncMessage::SyncRequest {
            album_id: album_id.to_string(),
            last_seen,
        }
    }
    
    /// Handle incoming sync request
    pub fn handle_sync_request(&self, album_id: &str, _last_seen: LogicalTime) -> Option<SyncMessage> {
        if let Some(album) = self.albums.get(album_id) {
            // For now, always send full album data
            // TODO: Implement incremental sync based on last_seen
            if let Ok(album_data) = album.to_bytes() {
                return Some(SyncMessage::SyncResponse {
                    album_id: album_id.to_string(),
                    album_data: Some(album_data),
                    incremental_ops: Vec::new(),
                });
            }
        }
        None
    }
    
    /// Handle incoming sync response
    pub fn handle_sync_response(&mut self, response: SyncMessage) -> Result<(), CrdtError> {
        match response {
            SyncMessage::SyncResponse { album_id, album_data, incremental_ops } => {
                if let Some(album_data) = album_data {
                    // Full sync
                    let other_album = MediaAlbum::from_bytes(&album_data)?;
                    self.merge_album(other_album)?;
                } else {
                    // Incremental sync
                    if let Some(album) = self.albums.get_mut(&album_id) {
                        for (media_id, ops) in incremental_ops {
                            for op in ops {
                                album.apply_to_item(&media_id, op)?;
                            }
                        }
                    }
                }
            }
            _ => return Err(CrdtError::InvalidOperation("Expected sync response".to_string())),
        }
        Ok(())
    }
    
    /// Get media data by block ID
    pub async fn get_media_data(&self, block_id: &[u8; 32]) -> Result<Vec<u8>, CrdtError> {
        self.block_manager.read_block(block_id).await.map_err(CrdtError::Flow)
    }
    
    /// List all albums
    pub fn list_albums(&self) -> Vec<(String, &AlbumMetadata)> {
        self.albums.iter()
            .map(|(id, album)| (id.clone(), &album.metadata))
            .collect()
    }
}

// === Test Utilities ===

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;
    
    pub fn create_test_sync_manager() -> (AlbumSyncManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().to_path_buf();
        
        // Create rimsd directories
        let mut rimsd_dirs = Vec::new();
        for i in 0..3 {
            let device_dir = test_dir.join(format!("rimsd_{}", i));
            let rimsd_dir = device_dir.join(".rimsd");
            std::fs::create_dir_all(&rimsd_dir).unwrap();
            rimsd_dirs.push(rimsd_dir);
        }
        
        let metadata_path = test_dir.join("metadata.json");
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            2, // threshold
            3, // total_shards
        ).unwrap());
        
        let sync_manager = AlbumSyncManager::new(block_manager, "test_replica".to_string());
        
        (sync_manager, temp_dir)
    }
    
    pub fn create_test_sync_manager_with_distinct_devices() -> (AlbumSyncManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().to_path_buf();
        
        // Create rimsd directories with unique device signatures
        let mut rimsd_dirs = Vec::new();
        for i in 0..3 {
            let device_dir = test_dir.join(format!("rimsd_{}", i));
            let rimsd_dir = device_dir.join(".rimsd");
            std::fs::create_dir_all(&rimsd_dir).unwrap();
            
            // Create a unique device signature file for each "device"
            let device_signature = rimsd_dir.join("device_signature.txt");
            std::fs::write(&device_signature, format!("device-{}-{}", i, uuid::Uuid::new_v4())).unwrap();
            
            // Create a mock filesystem UUID file
            let fs_uuid_file = rimsd_dir.join("fs_uuid.txt");
            std::fs::write(&fs_uuid_file, format!("fs-uuid-{}-{}", i, uuid::Uuid::new_v4())).unwrap();
            
            // Create mock hardware info
            let hw_info_file = rimsd_dir.join("hardware_info.txt");
            std::fs::write(&hw_info_file, format!("hw-serial-{}-manufacturer-{}", i, i)).unwrap();
            
            rimsd_dirs.push(rimsd_dir);
        }
        
        let metadata_path = test_dir.join("metadata.json");
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            2, // threshold
            3, // total_shards
        ).unwrap());
        
        let sync_manager = AlbumSyncManager::new(block_manager, "test_replica".to_string());
        
        (sync_manager, temp_dir)
    }
}
