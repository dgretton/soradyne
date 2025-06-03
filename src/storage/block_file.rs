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
        
        // Read existing data if any
        let mut file_data = if let Some(root) = *self.root_block.read().await {
            self.manager.read_block(&root).await?
        } else {
            Vec::new()
        };
        
        // Append new data
        file_data.extend_from_slice(data);
        
        // Write the complete file data (BlockManager will handle direct vs indirect blocks)
        let new_root = if file_data.len() <= BLOCK_SIZE {
            // Small file - use direct block
            self.manager.write_direct_block(&file_data).await?
        } else {
            // Large file - use indirect blocks
            self.write_large_file(&file_data).await?
        };
        
        // Update metadata
        *self.root_block.write().await = Some(new_root);
        *self.size.write().await = new_size;
        
        Ok(())
    }
    
    pub async fn read(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(root) = *self.root_block.read().await {
            let size = *self.size.read().await;
            
            if size <= BLOCK_SIZE {
                // Small file - direct block
                self.manager.read_block(&root).await
            } else {
                // Large file - indirect block
                self.read_large_file(&root).await
            }
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
    
    /// Write a large file using indirect blocks
    async fn write_large_file(&self, data: &[u8]) -> Result<BlockId, FlowError> {
        // Split data into chunks that fit in direct blocks
        let mut block_ids = Vec::new();
        
        for chunk in data.chunks(BLOCK_SIZE) {
            let block_id = self.manager.write_direct_block(chunk).await?;
            block_ids.push(block_id);
        }
        
        // Create indirect block containing the addresses of all data blocks
        let mut addresses_data = Vec::new();
        for block_id in &block_ids {
            addresses_data.extend_from_slice(block_id);
        }
        
        // Write the indirect block
        let indirect_block_id = self.manager.write_direct_block(&addresses_data).await?;
        
        Ok(indirect_block_id)
    }
    
    /// Read a large file from indirect blocks
    async fn read_large_file(&self, indirect_block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        // Read the indirect block to get addresses
        let addresses_data = self.manager.read_block(indirect_block_id).await?;
        
        // Parse block addresses (each address is 32 bytes)
        const BLOCK_ID_SIZE: usize = 32;
        if addresses_data.len() % BLOCK_ID_SIZE != 0 {
            return Err(FlowError::PersistenceError(
                "Invalid indirect block data".to_string()
            ));
        }
        
        let mut result = Vec::new();
        
        // Read each data block and append to result
        for chunk in addresses_data.chunks_exact(BLOCK_ID_SIZE) {
            let mut block_id = [0u8; BLOCK_ID_SIZE];
            block_id.copy_from_slice(chunk);
            
            let block_data = self.manager.read_block(&block_id).await?;
            result.extend_from_slice(&block_data);
        }
        
        Ok(result)
    }
}
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
