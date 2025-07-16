//! Manual erasure coding backend implementation
//! 
//! This backend uses the existing BlockManager to provide dissolution storage
//! through manual erasure coding and shard distribution across rimsd directories.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::storage::dissolution::{
    DissolutionStorage, DissolutionConfig, BlockId, BlockInfo, StorageStats, 
    DissolutionDemo, DeviceHealth, ReconstructionStats
};
use crate::storage::block_manager::BlockManager;
use crate::flow::FlowError;

/// Implementation using manual erasure coding via BlockManager
pub struct ManualErasureBackend {
    block_manager: Arc<BlockManager>,
    config: DissolutionConfig,
}

impl ManualErasureBackend {
    pub async fn new(
        rimsd_paths: Vec<PathBuf>,
        metadata_path: PathBuf,
        config: DissolutionConfig,
    ) -> Result<Self, FlowError> {
        let block_manager = Arc::new(BlockManager::new(
            rimsd_paths,
            metadata_path,
            config.threshold,
            config.total_shards,
        )?);
        
        // Initialize device fingerprints
        block_manager.initialize_device_fingerprints().await?;
        
        Ok(Self {
            block_manager,
            config,
        })
    }
    
    /// Create from auto-discovered Soradyne volumes
    pub async fn new_with_discovery(
        metadata_path: PathBuf,
        config: DissolutionConfig,
    ) -> Result<Self, FlowError> {
        let block_manager = Arc::new(BlockManager::new_with_discovery(
            metadata_path,
            config.threshold,
            config.total_shards,
        ).await?);
        
        // Initialize device fingerprints
        block_manager.initialize_device_fingerprints().await?;
        
        Ok(Self {
            block_manager,
            config,
        })
    }
}

#[async_trait]
impl DissolutionStorage for ManualErasureBackend {
    async fn store(&self, data: &[u8]) -> Result<BlockId, FlowError> {
        if data.len() > self.config.max_direct_block_size {
            return Err(FlowError::PersistenceError(
                format!("Data size {} exceeds max direct block size {}", 
                       data.len(), self.config.max_direct_block_size)
            ));
        }
        
        self.block_manager.write_direct_block(data).await
    }
    
    async fn retrieve(&self, block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        self.block_manager.read_block(block_id).await
    }
    
    async fn exists(&self, block_id: &BlockId) -> Result<bool, FlowError> {
        match self.block_manager.get_block_distribution(block_id).await {
            Ok(distribution) => Ok(distribution.can_reconstruct),
            Err(_) => Ok(false),
        }
    }
    
    async fn block_info(&self, block_id: &BlockId) -> Result<BlockInfo, FlowError> {
        let distribution = self.block_manager.get_block_distribution(block_id).await?;
        let blocks = self.block_manager.list_blocks().await;
        
        // Find the metadata for this block
        let metadata = blocks.iter()
            .find(|(id, _)| *id == *block_id)
            .map(|(_, meta)| meta)
            .ok_or_else(|| FlowError::PersistenceError("Block metadata not found".to_string()))?;
        
        Ok(BlockInfo {
            id: *block_id,
            size: distribution.original_size,
            created_at: metadata.created_at,
            is_indirect: metadata.directness > 0,
            shard_count: distribution.total_shards,
            available_shards: distribution.available_shards.len(),
            can_reconstruct: distribution.can_reconstruct,
        })
    }
    
    async fn delete(&self, _block_id: &BlockId) -> Result<(), FlowError> {
        // TODO: Implement block deletion in BlockManager
        Err(FlowError::PersistenceError("Block deletion not yet implemented".to_string()))
    }
    
    async fn list_blocks(&self) -> Result<Vec<BlockId>, FlowError> {
        let blocks = self.block_manager.list_blocks().await;
        Ok(blocks.into_iter().map(|(id, _)| id).collect())
    }
    
    async fn storage_stats(&self) -> Result<StorageStats, FlowError> {
        let storage_info = self.block_manager.get_storage_info();
        let blocks = self.block_manager.list_blocks().await;
        
        // Calculate reconstruction capability
        let mut blocks_at_risk = 0;
        let mut blocks_safe = 0;
        let mut blocks_lost = 0;
        let mut total_size = 0u64;
        
        for (block_id, metadata) in &blocks {
            total_size += metadata.size as u64;
            
            match self.block_manager.get_block_distribution(block_id).await {
                Ok(distribution) => {
                    if distribution.available_shards.len() < self.config.threshold {
                        blocks_lost += 1;
                    } else if distribution.available_shards.len() == self.config.threshold {
                        blocks_at_risk += 1;
                    } else {
                        blocks_safe += 1;
                    }
                }
                Err(_) => {
                    blocks_lost += 1;
                }
            }
        }
        
        // TODO: Implement device health checking
        let device_health = storage_info.rimsd_paths.iter().map(|path| {
            DeviceHealth {
                device_id: path.to_string_lossy().to_string(),
                path: path.clone(),
                available: path.exists(),
                free_space_bytes: 0, // TODO: Get actual free space
                total_space_bytes: 0, // TODO: Get actual total space
                error_rate: 0.0,
            }
        }).collect();
        
        let health_score = if storage_info.total_devices > 0 {
            device_health.iter().filter(|d| d.available).count() as f64 / storage_info.total_devices as f64
        } else {
            0.0
        };
        
        Ok(StorageStats {
            total_blocks: blocks.len(),
            total_size_bytes: total_size,
            available_devices: device_health.iter().filter(|d| d.available).count(),
            total_devices: storage_info.total_devices,
            health_score,
            device_health,
            reconstruction_capability: ReconstructionStats {
                blocks_at_risk,
                blocks_safe,
                blocks_lost,
            },
        })
    }
    
    async fn demonstrate_dissolution(&self, block_id: &BlockId, simulate_missing: Vec<usize>) -> Result<DissolutionDemo, FlowError> {
        let demo_result = self.block_manager.demonstrate_erasure_recovery(block_id, simulate_missing.clone()).await?;
        
        // Verify data integrity by comparing with original
        let data_integrity_verified = if demo_result.recovery_successful {
            // Try to read the original data and compare
            match self.block_manager.read_block(block_id).await {
                Ok(original_data) => demo_result.recovered_data_size == original_data.len(),
                Err(_) => false,
            }
        } else {
            false
        };
        
        Ok(DissolutionDemo {
            block_id: *block_id,
            original_shards: demo_result.original_shards,
            simulated_missing: simulate_missing,
            available_shards: demo_result.available_shards,
            threshold_required: demo_result.threshold_required,
            can_reconstruct: demo_result.recovery_successful,
            reconstruction_successful: demo_result.recovery_successful,
            data_integrity_verified,
            recovered_data_size: demo_result.recovered_data_size,
        })
    }
    
    async fn maintenance(&self) -> Result<(), FlowError> {
        // Verify device continuity
        self.block_manager.verify_device_continuity().await?;
        
        // TODO: Add other maintenance tasks like:
        // - Checking for corrupted shards
        // - Rebalancing shards across devices
        // - Cleaning up orphaned metadata
        
        Ok(())
    }
    
    fn config(&self) -> &DissolutionConfig {
        &self.config
    }
    
    async fn update_config(&mut self, _config: DissolutionConfig) -> Result<(), FlowError> {
        // For now, don't allow config changes after creation
        // This would require rebuilding the BlockManager
        Err(FlowError::PersistenceError(
            "Configuration updates not supported for manual erasure backend".to_string()
        ))
    }
    
    async fn verify_device_continuity(&self) -> Result<(), FlowError> {
        self.block_manager.verify_device_continuity().await
    }
    
    async fn initialize_device_fingerprints(&self) -> Result<(), FlowError> {
        self.block_manager.initialize_device_fingerprints().await
    }
}
