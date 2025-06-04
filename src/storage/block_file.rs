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
            // Large file - split into chunks and use indirect blocks
            let mut chunk_block_ids = Vec::new();
            
            // Split data into chunks
            for chunk in data.chunks(BLOCK_SIZE) {
                let chunk_block_id = self.manager.write_direct_block(chunk).await?;
                chunk_block_ids.push(chunk_block_id);
            }
            
            // Create indirect block containing the chunk addresses
            let mut addresses_data = Vec::new();
            for block_id in &chunk_block_ids {
                addresses_data.extend_from_slice(block_id);
            }
            
            let indirect_block_id = self.manager.write_direct_block(&addresses_data).await?;
            *self.root_block.write().await = Some(indirect_block_id);
            *self.size.write().await = data.len();
        }
        Ok(())
    }
    
    pub async fn read(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(root) = *self.root_block.read().await {
            let size = *self.size.read().await;
            if size <= BLOCK_SIZE {
                // Direct block
                self.manager.read_block(&root).await
            } else {
                // Indirect block - read addresses and reconstruct
                let addresses_data = self.manager.read_block(&root).await?;
                let mut result = Vec::new();
                
                // Parse addresses (each is 32 bytes)
                for chunk in addresses_data.chunks_exact(32) {
                    let mut block_id = [0u8; 32];
                    block_id.copy_from_slice(chunk);
                    let chunk_data = self.manager.read_block(&block_id).await?;
                    result.extend_from_slice(&chunk_data);
                }
                
                // Truncate to actual file size (last chunk might be padded)
                result.truncate(size);
                Ok(result)
            }
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
