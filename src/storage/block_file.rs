//! Block-based file abstraction

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::storage::block_manager::BlockManager;
use crate::flow::FlowError;

const BLOCK_SIZE: usize = 32 * 1024 * 1024; // 32MB

pub struct BlockFile {
    manager: Arc<BlockManager>,
    root_block: RwLock<Option<[u8; 32]>>,
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
    
    pub fn from_existing(manager: Arc<BlockManager>, root_block: [u8; 32], size: usize) -> Self {
        Self {
            manager,
            root_block: RwLock::new(Some(root_block)),
            size: RwLock::new(size),
        }
    }
    
    pub async fn append(&self, data: &[u8]) -> Result<(), FlowError> {
        if data.len() <= BLOCK_SIZE {
            // Small file - use direct block
            let block_id = self.manager.write_direct_block(data).await?;
            *self.root_block.write().await = Some(block_id);
            *self.size.write().await = data.len();
        } else {
            // Large file - would need indirect blocks
            // For now, just store as direct block (will fail if too large)
            let block_id = self.manager.write_direct_block(data).await?;
            *self.root_block.write().await = Some(block_id);
            *self.size.write().await = data.len();
        }
        Ok(())
    }
    
    pub async fn read(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(root) = *self.root_block.read().await {
            self.manager.read_block(&root).await
        } else {
            Ok(Vec::new())
        }
    }
    
    pub async fn root_block(&self) -> Option<[u8; 32]> {
        *self.root_block.read().await
    }
    
    pub async fn size(&self) -> usize {
        *self.size.read().await
    }
}
