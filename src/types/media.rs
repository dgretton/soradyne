use crate::storage::block_file::BlockFile;
use crate::storage::block_manager::BlockManager;
use crate::flow::FlowError;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoMetadata {
    pub id: Uuid,
    pub name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub created_at: DateTime<Utc>,
    pub root_block: Option<[u8; 32]>,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub id: Uuid,
    pub name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub created_at: DateTime<Utc>,
    pub root_block: Option<[u8; 32]>,
    pub size: usize,
}

pub struct PhotoStorage {
    block_manager: Arc<BlockManager>,
}

impl PhotoStorage {
    pub fn new(block_manager: Arc<BlockManager>) -> Self {
        Self { block_manager }
    }
    
    pub async fn save_photo(&self, name: &str, mime_type: &str, data: &[u8]) 
        -> Result<PhotoMetadata, FlowError> {
        let file = BlockFile::new(self.block_manager.clone());
        file.append(data).await?;
        
        let metadata = PhotoMetadata {
            id: Uuid::new_v4(),
            name: name.to_string(),
            mime_type: mime_type.to_string(),
            width: 0, // TODO: Parse from image data
            height: 0, // TODO: Parse from image data
            created_at: Utc::now(),
            root_block: file.root_block().await,
            size: data.len(),
        };
        
        Ok(metadata)
    }
    
    pub async fn load_photo(&self, metadata: &PhotoMetadata) 
        -> Result<Vec<u8>, FlowError> {
        if let Some(root_block) = metadata.root_block {
            let file = BlockFile::from_existing(
                self.block_manager.clone(), 
                root_block, 
                metadata.size
            );
            file.read().await
        } else {
            Err(FlowError::PersistenceError("Photo has no data".to_string()))
        }
    }
}

pub struct VideoStorage {
    block_manager: Arc<BlockManager>,
}

impl VideoStorage {
    pub fn new(block_manager: Arc<BlockManager>) -> Self {
        Self { block_manager }
    }
    
    pub async fn save_video(&self, name: &str, mime_type: &str, data: &[u8]) 
        -> Result<VideoMetadata, FlowError> {
        let file = BlockFile::new(self.block_manager.clone());
        file.append(data).await?;
        
        let metadata = VideoMetadata {
            id: Uuid::new_v4(),
            name: name.to_string(),
            mime_type: mime_type.to_string(),
            width: 0, // TODO: Parse from video data
            height: 0, // TODO: Parse from video data
            duration_seconds: 0.0, // TODO: Parse from video data
            created_at: Utc::now(),
            root_block: file.root_block().await,
            size: data.len(),
        };
        
        Ok(metadata)
    }
    
    pub async fn load_video(&self, metadata: &VideoMetadata) 
        -> Result<Vec<u8>, FlowError> {
        if let Some(root_block) = metadata.root_block {
            let file = BlockFile::from_existing(
                self.block_manager.clone(), 
                root_block, 
                metadata.size
            );
            file.read().await
        } else {
            Err(FlowError::PersistenceError("Video has no data".to_string()))
        }
    }
}
