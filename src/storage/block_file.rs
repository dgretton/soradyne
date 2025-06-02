use crate::storage::block_manager::BlockManager;
use crate::storage::block::{BlockId, BLOCK_SIZE};
use crate::flow::FlowError;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct BlockFile {
    manager: Arc<BlockManager>,
    root_block: RwLock<Option<BlockId>>,
    size: RwLock<usize>,
}

impl BlockFile {
    pub fn new(manager: Arc<BlockManager>) -> Self {
        Self {
            manager,
            root_block: RwLock::new(None),
            size: RwLock::new(0),
        }
    }
    
    pub fn from_existing(manager: Arc<BlockManager>, root_block: BlockId, size: usize) -> Self {
        Self {
            manager,
            root_block: RwLock::new(Some(root_block)),
            size: RwLock::new(size),
        }
    }
    
    pub async fn append(&self, data: &[u8]) -> Result<(), FlowError> {
        let current_size = *self.size.read().await;
        let new_size = current_size + data.len();
        
        // For now, we'll support single block files
        // TODO: Implement indirect blocks for larger files
        if new_size > BLOCK_SIZE {
            return Err(FlowError::PersistenceError(
                "File size exceeds single block limit".to_string()
            ));
        }
        
        // Read existing data if any
        let mut file_data = if let Some(root) = *self.root_block.read().await {
            self.manager.read_block(&root).await?
        } else {
            Vec::new()
        };
        
        // Append new data
        file_data.extend_from_slice(data);
        
        // Write new block
        let new_root = self.manager.write_direct_block(&file_data).await?;
        
        // Update metadata
        *self.root_block.write().await = Some(new_root);
        *self.size.write().await = new_size;
        
        Ok(())
    }
    
    pub async fn read(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(root) = *self.root_block.read().await {
            self.manager.read_block(&root).await
        } else {
            Ok(Vec::new())
        }
    }
    
    pub async fn size(&self) -> usize {
        *self.size.read().await
    }
    
    pub async fn root_block(&self) -> Option<BlockId> {
        *self.root_block.read().await
    }
}
