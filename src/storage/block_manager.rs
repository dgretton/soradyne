use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};

use crate::storage::block::*;
use crate::storage::erasure::ErasureEncoder;
use crate::flow::FlowError;

pub struct BlockManager {
    rimsd_directories: Vec<PathBuf>,
    metadata_store: Arc<RwLock<BlockMetadataStore>>,
    erasure_encoder: ErasureEncoder,
    threshold: usize,
    total_shards: usize,
}

pub struct BlockMetadataStore {
    blocks: HashMap<BlockId, BlockMetadata>,
    metadata_path: PathBuf,
}

impl BlockMetadataStore {
    pub fn load_or_create(metadata_path: PathBuf) -> Result<Self, FlowError> {
        let blocks = if metadata_path.exists() {
            let data = std::fs::read(&metadata_path).map_err(|e| 
                FlowError::PersistenceError(format!("Failed to read metadata: {}", e))
            )?;
            
            serde_json::from_slice(&data).map_err(|e|
                FlowError::PersistenceError(format!("Failed to parse metadata: {}", e))
            )?
        } else {
            HashMap::new()
        };
        
        Ok(Self {
            blocks,
            metadata_path,
        })
    }
    
    pub fn add_block(&mut self, metadata: BlockMetadata) -> Result<(), FlowError> {
        self.blocks.insert(metadata.id, metadata);
        self.save()
    }
    
    pub fn get_block(&self, id: &BlockId) -> Result<BlockMetadata, FlowError> {
        self.blocks.get(id)
            .cloned()
            .ok_or_else(|| FlowError::PersistenceError(
                format!("Block not found: {}", hex::encode(id))
            ))
    }
    
    fn save(&self) -> Result<(), FlowError> {
        let data = serde_json::to_vec_pretty(&self.blocks).map_err(|e|
            FlowError::PersistenceError(format!("Failed to serialize metadata: {}", e))
        )?;
        
        std::fs::write(&self.metadata_path, data).map_err(|e|
            FlowError::PersistenceError(format!("Failed to write metadata: {}", e))
        )
    }
}

impl BlockManager {
    pub fn new(
        rimsd_directories: Vec<PathBuf>, 
        metadata_path: PathBuf,
        threshold: usize,
        total_shards: usize,
    ) -> Result<Self, FlowError> {
        // Ensure all rimsd directories exist
        for dir in &rimsd_directories {
            std::fs::create_dir_all(dir).map_err(|e| 
                FlowError::PersistenceError(format!("Failed to create rimsd directory: {}", e))
            )?;
        }
        
        let metadata_store = BlockMetadataStore::load_or_create(metadata_path)?;
        
        Ok(Self {
            rimsd_directories,
            metadata_store: Arc::new(RwLock::new(metadata_store)),
            erasure_encoder: ErasureEncoder::new(threshold, total_shards),
            threshold,
            total_shards,
        })
    }
    
    pub async fn write_direct_block(&self, data: &[u8]) -> Result<BlockId, FlowError> {
        if data.len() > BLOCK_SIZE {
            return Err(FlowError::PersistenceError(
                format!("Data size {} exceeds block size {}", data.len(), BLOCK_SIZE)
            ));
        }
        
        let id = self.generate_block_id();
        let metadata = BlockMetadata {
            id,
            directness: 0,
            size: data.len(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            shard_locations: Vec::new(),
        };
        
        // Erasure encode the data
        let shards = self.erasure_encoder.encode(data)?;
        
        // Distribute shards across rimsd directories
        let mut shard_locations = Vec::new();
        for (i, shard) in shards.iter().enumerate() {
            let rimsd_dir = &self.rimsd_directories[i % self.rimsd_directories.len()];
            let shard_path = self.shard_path(rimsd_dir, &id, i);
            
            // Write shard to disk
            if let Some(parent) = shard_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e|
                    FlowError::PersistenceError(format!("Failed to create shard directory: {}", e))
                )?;
            }
            tokio::fs::write(&shard_path, shard).await.map_err(|e|
                FlowError::PersistenceError(format!("Failed to write shard: {}", e))
            )?;
            
            shard_locations.push(ShardLocation {
                shard_index: i,
                device_id: self.get_device_id(),
                rimsd_path: rimsd_dir.to_string_lossy().to_string(),
                relative_path: shard_path.strip_prefix(rimsd_dir)
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            });
        }
        
        // Update metadata
        let mut metadata = metadata;
        metadata.shard_locations = shard_locations;
        
        // Save metadata
        self.metadata_store.write().await.add_block(metadata)?;
        
        Ok(id)
    }
    
    pub async fn read_block(&self, id: &BlockId) -> Result<Vec<u8>, FlowError> {
        let metadata = self.metadata_store.read().await.get_block(id)?;
        
        if metadata.directness == 0 {
            // Direct block - reconstruct from shards
            self.read_direct_block(&metadata).await
        } else {
            // Indirect block - read addresses and recursively read blocks
            self.read_indirect_block(&metadata).await
        }
    }
    
    async fn read_direct_block(&self, metadata: &BlockMetadata) -> Result<Vec<u8>, FlowError> {
        // Collect available shards
        let mut shards = HashMap::new();
        
        for location in &metadata.shard_locations {
            let shard_path = PathBuf::from(&location.rimsd_path)
                .join(&location.relative_path);
            
            if shard_path.exists() {
                let shard_data = tokio::fs::read(&shard_path).await.map_err(|e|
                    FlowError::PersistenceError(format!("Failed to read shard: {}", e))
                )?;
                shards.insert(location.shard_index, shard_data);
            }
        }
        
        // Check if we have enough shards
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards available: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        // Reconstruct data from shards
        self.erasure_encoder.decode(shards, metadata.size)
    }
    
    async fn read_indirect_block(&self, metadata: &BlockMetadata) -> Result<Vec<u8>, FlowError> {
        // First read the indirect block itself to get addresses
        let addresses_data = self.read_direct_block(metadata).await?;
        let addresses = self.parse_addresses(&addresses_data)?;
        
        // Read all referenced blocks
        let mut result = Vec::new();
        for address in addresses {
            let block_data = self.read_block(&address).await?;
            result.extend_from_slice(&block_data);
        }
        
        Ok(result)
    }
    
    fn generate_block_id(&self) -> BlockId {
        // Use a cryptographic hash of UUID + timestamp
        let mut hasher = Sha256::new();
        hasher.update(Uuid::new_v4().as_bytes());
        hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }
    
    fn shard_path(&self, rimsd_dir: &Path, block_id: &BlockId, shard_index: usize) -> PathBuf {
        // Use first 4 bytes of block ID for directory structure
        let hex_id = hex::encode(block_id);
        rimsd_dir
            .join(&hex_id[..2])
            .join(&hex_id[2..4])
            .join(format!("{}.{}.shard", hex_id, shard_index))
    }
    
    fn get_device_id(&self) -> Uuid {
        // TODO: Get actual device ID from identity manager
        Uuid::new_v4()
    }
    
    fn parse_addresses(&self, data: &[u8]) -> Result<Vec<BlockId>, FlowError> {
        if data.len() % BLOCK_ID_SIZE != 0 {
            return Err(FlowError::PersistenceError(
                "Invalid indirect block data".to_string()
            ));
        }
        
        let mut addresses = Vec::new();
        for chunk in data.chunks_exact(BLOCK_ID_SIZE) {
            let mut id = [0u8; BLOCK_ID_SIZE];
            id.copy_from_slice(chunk);
            addresses.push(id);
        }
        
        Ok(addresses)
    }
}
