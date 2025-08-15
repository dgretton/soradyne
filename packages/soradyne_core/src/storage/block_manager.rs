use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use sha2::{Sha256, Digest};
use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::storage::block::*;
use crate::storage::device_identity::{BasicFingerprint, BayesianDeviceIdentifier, fingerprint_device};

#[derive(Debug, Clone)]
pub struct StorageInfo {
    pub total_devices: usize,
    pub threshold: usize,
    pub total_shards: usize,
    pub rimsd_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct BlockDistribution {
    pub block_id: [u8; 32],
    pub total_shards: usize,
    pub available_shards: Vec<ShardInfo>,
    pub missing_shards: Vec<usize>,
    pub can_reconstruct: bool,
    pub original_size: usize,
}

#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub index: usize,
    pub device_path: String,
    pub file_path: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct DemonstrationResult {
    pub original_shards: usize,
    pub simulated_missing: Vec<usize>,
    pub available_shards: usize,
    pub threshold_required: usize,
    pub recovery_successful: bool,
    pub recovered_data_size: usize,
}

const BLOCK_SIZE: usize = 32 * 1024 * 1024; // 32MB
use crate::storage::erasure::ShamirErasureEncoder;
use crate::flow::FlowError;

#[derive(Debug)]
pub struct BlockManager {
    rimsd_directories: Vec<PathBuf>,
    metadata_store: Arc<RwLock<BlockMetadataStore>>,
    erasure_encoder: ShamirErasureEncoder,
    threshold: usize,
    total_shards: usize,
    device_identifier: BayesianDeviceIdentifier,
    device_fingerprints: Arc<RwLock<HashMap<PathBuf, BasicFingerprint>>>,
}

#[derive(Debug)]
pub struct BlockMetadataStore {
    blocks: HashMap<[u8; 32], BlockMetadata>,
    metadata_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct SerializableBlockStore {
    blocks: HashMap<String, BlockMetadata>,
}

impl BlockMetadataStore {
    pub fn load_or_create(metadata_path: PathBuf) -> Result<Self, FlowError> {
        let blocks = if metadata_path.exists() {
            let data = std::fs::read(&metadata_path).map_err(|e| 
                FlowError::PersistenceError(format!("Failed to read metadata: {}", e))
            )?;
            
            let serializable: SerializableBlockStore = serde_json::from_slice(&data).map_err(|e|
                FlowError::PersistenceError(format!("Failed to parse metadata: {}", e))
            )?;
            
            // Convert hex string keys back to [u8; 32]
            let mut blocks = HashMap::new();
            for (hex_key, metadata) in serializable.blocks {
                let block_id = hex::decode(&hex_key).map_err(|e|
                    FlowError::PersistenceError(format!("Invalid block ID in metadata: {}", e))
                )?;
                if block_id.len() != 32 {
                    return Err(FlowError::PersistenceError("Invalid block ID length".to_string()));
                }
                let mut id = [0u8; 32];
                id.copy_from_slice(&block_id);
                blocks.insert(id, metadata);
            }
            blocks
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
    
    pub fn get_block(&self, id: &[u8; 32]) -> Result<BlockMetadata, FlowError> {
        self.blocks.get(id)
            .cloned()
            .ok_or_else(|| FlowError::PersistenceError(
                format!("Block not found: {}", hex::encode(id))
            ))
    }
    
    fn save(&self) -> Result<(), FlowError> {
        // Convert [u8; 32] keys to hex strings for JSON serialization
        let mut serializable_blocks = HashMap::new();
        for (block_id, metadata) in &self.blocks {
            let hex_key = hex::encode(block_id);
            serializable_blocks.insert(hex_key, metadata.clone());
        }
        
        let serializable = SerializableBlockStore {
            blocks: serializable_blocks,
        };
        
        let data = serde_json::to_vec_pretty(&serializable).map_err(|e|
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
        
        let erasure_encoder = ShamirErasureEncoder::new(threshold, total_shards)?;
        
        Ok(Self {
            rimsd_directories,
            metadata_store: Arc::new(RwLock::new(metadata_store)),
            erasure_encoder,
            threshold,
            total_shards,
            device_identifier: BayesianDeviceIdentifier::default(),
            device_fingerprints: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Create a new BlockManager by discovering Soradyne volumes automatically
    pub async fn new_with_discovery(
        metadata_path: PathBuf,
        threshold: usize,
        total_shards: usize,
    ) -> Result<Self, FlowError> {
        let rimsd_dirs = crate::storage::device_identity::discover_soradyne_volumes().await?;
        
        if rimsd_dirs.is_empty() {
            return Err(FlowError::PersistenceError(
                "No Soradyne volumes found. Please initialize some SD cards first.".to_string()
            ));
        }
        
        println!("Found {} Soradyne volumes", rimsd_dirs.len());
        
        Self::new(rimsd_dirs, metadata_path, threshold, total_shards)
    }
    
    /// Get information about the current storage configuration
    pub fn get_storage_info(&self) -> StorageInfo {
        StorageInfo {
            total_devices: self.rimsd_directories.len(),
            threshold: self.threshold,
            total_shards: self.total_shards,
            rimsd_paths: self.rimsd_directories.clone(),
        }
    }
    
    /// Get detailed information about block distribution
    pub async fn get_block_distribution(&self, block_id: &[u8; 32]) -> Result<BlockDistribution, FlowError> {
        let metadata = self.metadata_store.read().await.get_block(block_id)?;
        
        let mut available_shards = Vec::new();
        let mut missing_shards = Vec::new();
        
        for location in &metadata.shard_locations {
            let shard_path = PathBuf::from(&location.rimsd_path)
                .join(&location.relative_path);
            
            if shard_path.exists() {
                available_shards.push(ShardInfo {
                    index: location.shard_index,
                    device_path: location.rimsd_path.clone(),
                    file_path: shard_path.to_string_lossy().to_string(),
                    size: tokio::fs::metadata(&shard_path).await
                        .map(|m| m.len())
                        .unwrap_or(0),
                });
            } else {
                missing_shards.push(location.shard_index);
            }
        }
        
        let can_reconstruct = available_shards.len() >= self.threshold;
        
        Ok(BlockDistribution {
            block_id: *block_id,
            total_shards: metadata.shard_locations.len(),
            available_shards,
            missing_shards,
            can_reconstruct,
            original_size: metadata.size,
        })
    }
    
    /// List all blocks in the metadata store
    pub async fn list_blocks(&self) -> Vec<([u8; 32], BlockMetadata)> {
        let store = self.metadata_store.read().await;
        store.blocks.iter().map(|(id, meta)| (*id, meta.clone())).collect()
    }
    
    /// Demonstrate erasure coding by intentionally "removing" some shards
    pub async fn demonstrate_erasure_recovery(&self, block_id: &[u8; 32], shards_to_simulate_missing: Vec<usize>) -> Result<DemonstrationResult, FlowError> {
        self.demonstrate_fault_tolerance(block_id, shards_to_simulate_missing).await
    }
    
    /// Demonstrate fault tolerance by intentionally "removing" some shards
    pub async fn demonstrate_fault_tolerance(&self, block_id: &[u8; 32], shards_to_simulate_missing: Vec<usize>) -> Result<DemonstrationResult, FlowError> {
        let metadata = self.metadata_store.read().await.get_block(block_id)?;
        
        // Collect available shards, excluding the ones we're simulating as missing
        let mut available_shards = HashMap::new();
        
        for location in &metadata.shard_locations {
            if shards_to_simulate_missing.contains(&location.shard_index) {
                continue; // Simulate this shard as missing
            }
            
            let shard_path = PathBuf::from(&location.rimsd_path)
                .join(&location.relative_path);
            
            if shard_path.exists() {
                let shard_data = tokio::fs::read(&shard_path).await.map_err(|e|
                    FlowError::PersistenceError(format!("Failed to read shard: {}", e))
                )?;
                available_shards.insert(location.shard_index, shard_data);
            }
        }
        
        let available_shards_count = available_shards.len();
        let can_recover = available_shards_count >= self.threshold;
        let recovery_result = if can_recover {
            match self.erasure_encoder.decode(available_shards, metadata.size) {
                Ok(data) => Some(data),
                Err(_) => None,
            }
        } else {
            None
        };
        
        Ok(DemonstrationResult {
            original_shards: metadata.shard_locations.len(),
            simulated_missing: shards_to_simulate_missing,
            available_shards: available_shards_count,
            threshold_required: self.threshold,
            recovery_successful: recovery_result.is_some(),
            recovered_data_size: recovery_result.as_ref().map(|d| d.len()).unwrap_or(0),
        })
    }
    
    pub async fn write_direct_block(&self, data: &[u8]) -> Result<[u8; 32], FlowError> {
        if data.len() > BLOCK_SIZE {
            return Err(FlowError::PersistenceError(
                format!("Data size {} exceeds block size {}", data.len(), BLOCK_SIZE)
            ));
        }
        
        let id = self.generate_block_id();
        let nonce = ShamirErasureEncoder::derive_nonce(&id);
        
        let metadata = BlockMetadata {
            id,
            directness: 0,
            size: data.len(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            shard_locations: Vec::new(),
            encryption_version: 1, // New Shamir+RS format
            nonce,
        };
        
        // Shamir + Reed-Solomon encode the data
        let shards_with_keys = self.erasure_encoder.encode(data, &id)?;
        
        // Distribute shards across rimsd directories
        println!("📦 Distributing {} encrypted shards across {} devices:", shards_with_keys.len(), self.rimsd_directories.len());
        let mut shard_locations = Vec::new();
        
        for (i, shard_with_key) in shards_with_keys.iter().enumerate() {
            let rimsd_dir = &self.rimsd_directories[i % self.rimsd_directories.len()].as_path();
            let shard_path = self.shard_path(rimsd_dir, &id, i);
            let key_share_path = self.key_share_path(rimsd_dir, &id, i);
            
            println!("   📝 Writing shard {} ({} bytes) + key share → {}", 
                i, shard_with_key.shard_data.len(), shard_path.display());
            
            // Write shard data to disk
            if let Some(parent) = shard_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e|
                    FlowError::PersistenceError(format!("Failed to create shard directory: {}", e))
                )?;
            }
            tokio::fs::write(&shard_path, &shard_with_key.shard_data).await.map_err(|e|
                FlowError::PersistenceError(format!("Failed to write shard: {}", e))
            )?;
            
            // Write key share to disk
            if let Some(parent) = key_share_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e|
                    FlowError::PersistenceError(format!("Failed to create key share directory: {}", e))
                )?;
            }
            let key_share_data = serde_json::to_vec(&shard_with_key.key_share).map_err(|e|
                FlowError::PersistenceError(format!("Failed to serialize key share: {}", e))
            )?;
            tokio::fs::write(&key_share_path, &key_share_data).await.map_err(|e|
                FlowError::PersistenceError(format!("Failed to write key share: {}", e))
            )?;
            
            println!("   ✅ Shard {} and key share written successfully", i);
            
            shard_locations.push(ShardLocation {
                shard_index: i,
                device_id: self.get_device_id(),
                rimsd_path: rimsd_dir.to_string_lossy().to_string(),
                relative_path: shard_path.strip_prefix(rimsd_dir)
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                key_share_path: Some(key_share_path.strip_prefix(rimsd_dir)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()),
            });
        }
        println!("🎯 All encrypted shards and key shares distributed successfully!");
        
        // Update metadata
        let mut metadata = metadata;
        metadata.shard_locations = shard_locations;
        
        // Save metadata
        self.metadata_store.write().await.add_block(metadata)?;
        
        Ok(id)
    }
    
    pub async fn read_block(&self, id: &[u8; 32]) -> Result<Vec<u8>, FlowError> {
        let metadata = self.metadata_store.read().await.get_block(id)?;
        
        if metadata.directness == 0 {
            // Direct block - reconstruct from shards
            self.read_direct_block(&metadata).await
        } else {
            // Indirect block - read addresses and recursively read blocks
            Box::pin(self.read_indirect_block(&metadata)).await
        }
    }
    
    async fn read_direct_block(&self, metadata: &BlockMetadata) -> Result<Vec<u8>, FlowError> {
        println!("📖 Reading block with {} total shards (need {} minimum):", 
            metadata.shard_locations.len(), self.threshold);
        
        // Check encryption version for backward compatibility
        if metadata.encryption_version == 0 {
            return self.read_legacy_block(metadata).await;
        }
        
        // Collect available shards and key shares
        let mut shards_with_keys = HashMap::new();
        let mut missing_shards = Vec::new();
        
        for location in &metadata.shard_locations {
            let shard_path = PathBuf::from(&location.rimsd_path)
                .join(&location.relative_path);
            
            let key_share_path = if let Some(key_path) = &location.key_share_path {
                PathBuf::from(&location.rimsd_path).join(key_path)
            } else {
                // Fallback to legacy format
                return self.read_legacy_block(metadata).await;
            };
            
            if shard_path.exists() && key_share_path.exists() {
                match (tokio::fs::read(&shard_path).await, tokio::fs::read(&key_share_path).await) {
                    (Ok(shard_data), Ok(key_share_data)) => {
                        match serde_json::from_slice(&key_share_data) {
                            Ok(key_share) => {
                                println!("   ✅ Read shard {} ({} bytes) + key share ← {}", 
                                    location.shard_index, shard_data.len(), shard_path.display());
                                
                                shards_with_keys.insert(location.shard_index, crate::storage::erasure::ShardWithKey {
                                    shard_data,
                                    key_share,
                                });
                            }
                            Err(e) => {
                                println!("   ❌ Failed to parse key share {}: {}", location.shard_index, e);
                                missing_shards.push(location.shard_index);
                            }
                        }
                    }
                    (Err(e), _) => {
                        println!("   ❌ Failed to read shard {} from {}: {}", 
                            location.shard_index, shard_path.display(), e);
                        missing_shards.push(location.shard_index);
                    }
                    (_, Err(e)) => {
                        println!("   ❌ Failed to read key share {} from {}: {}", 
                            location.shard_index, key_share_path.display(), e);
                        missing_shards.push(location.shard_index);
                    }
                }
            } else {
                println!("   ⚠️  Shard {} or key share missing: {} / {}", 
                    location.shard_index, shard_path.display(), key_share_path.display());
                missing_shards.push(location.shard_index);
            }
        }
        
        println!("📊 Shard status: {} available, {} missing", shards_with_keys.len(), missing_shards.len());
        
        // Check if we have enough shards
        if shards_with_keys.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards available: {} < {} (missing: {:?})", 
                    shards_with_keys.len(), self.threshold, missing_shards)
            ));
        }
        
        println!("🔍 Reconstruction Diagnostics:");
        println!("   Total shards: {}", shards_with_keys.len());
        println!("   Block ID: {}", hex::encode(&metadata.id));
        println!("   Expected size: {}", metadata.size);
        println!("   Encryption version: {}", metadata.encryption_version);
        
        // Log shard details
        for (i, (shard_index, shard_with_key)) in shards_with_keys.iter().enumerate() {
            println!("   Shard {}: {} bytes, Key Share: {} bytes", 
                shard_index, 
                shard_with_key.shard_data.len(), 
                serde_json::to_vec(&shard_with_key.key_share).map(|v| v.len()).unwrap_or(0)
            );
        }

        println!("🔧 Reconstructing data from {} shards using Shamir + Reed-Solomon...", shards_with_keys.len());
        
        // Validate shard sizes are consistent
        let shard_sizes: Vec<usize> = shards_with_keys.values().map(|shard| shard.shard_data.len()).collect();
        if let (Some(min_size), Some(max_size)) = (shard_sizes.iter().min(), shard_sizes.iter().max()) {
            if min_size != max_size {
                println!("❌ Inconsistent shard sizes: min={}, max={}, all={:?}", min_size, max_size, shard_sizes);
                return Err(FlowError::PersistenceError("Inconsistent shard sizes".to_string()));
            }
        }

        // Create streaming decoder and reconstruct
        let mut decoder = self.erasure_encoder
            .decode_with_streaming(shards_with_keys, &metadata.id, metadata.size)
            .map_err(|e| {
                println!("❌ Failed to create streaming decoder: {}", e);
                e
            })?;

        let result = decoder.reconstruct_all().await.map_err(|e| {
            println!("❌ Failed to reconstruct data: {}", e);
            e
        })?;
        
        println!("✅ Successfully reconstructed {} bytes of encrypted data", result.len());
        
        Ok(result)
    }
    
    /// Read legacy RS-only blocks for backward compatibility
    async fn read_legacy_block(&self, metadata: &BlockMetadata) -> Result<Vec<u8>, FlowError> {
        println!("📖 Reading legacy RS-only block...");
        
        let mut shards = HashMap::new();
        let mut missing_shards = Vec::new();
        
        for location in &metadata.shard_locations {
            let shard_path = PathBuf::from(&location.rimsd_path)
                .join(&location.relative_path);
            
            if shard_path.exists() {
                match tokio::fs::read(&shard_path).await {
                    Ok(shard_data) => {
                        println!("   ✅ Read legacy shard {} ({} bytes) ← {}", 
                            location.shard_index, shard_data.len(), shard_path.display());
                        shards.insert(location.shard_index, shard_data);
                    }
                    Err(e) => {
                        println!("   ❌ Failed to read legacy shard {} from {}: {}", 
                            location.shard_index, shard_path.display(), e);
                        missing_shards.push(location.shard_index);
                    }
                }
            } else {
                println!("   ⚠️  Legacy shard {} missing: {}", 
                    location.shard_index, shard_path.display());
                missing_shards.push(location.shard_index);
            }
        }
        
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough legacy shards available: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        // Use legacy decode method
        let result = self.erasure_encoder.decode(shards, metadata.size)?;
        println!("✅ Successfully reconstructed {} bytes from legacy block", result.len());
        
        Ok(result)
    }
    
    async fn read_indirect_block(&self, metadata: &BlockMetadata) -> Result<Vec<u8>, FlowError> {
        // First read the indirect block itself to get addresses
        let addresses_data = self.read_direct_block(metadata).await?;
        let addresses = self.parse_addresses(&addresses_data)?;
        
        // Read all referenced blocks
        let mut result = Vec::new();
        for address in addresses {
            let block_data = Box::pin(self.read_block(&address)).await?;
            result.extend_from_slice(&block_data);
        }
        
        Ok(result)
    }
    
    fn generate_block_id(&self) -> [u8; 32] {
        // Use a cryptographic hash of UUID + timestamp
        let mut hasher = Sha256::new();
        hasher.update(Uuid::new_v4().as_bytes());
        hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }
    
    fn shard_path(&self, rimsd_dir: &Path, block_id: &[u8; 32], shard_index: usize) -> PathBuf {
        // Use first 4 bytes of block ID for directory structure
        let hex_id = hex::encode(block_id);
        rimsd_dir
            .join(&hex_id[..2])
            .join(&hex_id[2..4])
            .join(format!("{}.{}.shard", hex_id, shard_index))
    }
    
    fn key_share_path(&self, rimsd_dir: &Path, block_id: &[u8; 32], shard_index: usize) -> PathBuf {
        // Store key shares alongside shards
        let hex_id = hex::encode(block_id);
        rimsd_dir
            .join(&hex_id[..2])
            .join(&hex_id[2..4])
            .join(format!("{}.{}.keyshare", hex_id, shard_index))
    }
    
    fn get_device_id(&self) -> Uuid {
        // TODO: Get actual device ID from identity manager
        Uuid::new_v4()
    }
    
    fn parse_addresses(&self, data: &[u8]) -> Result<Vec<[u8; 32]>, FlowError> {
        if data.len() % 32 != 0 {
            return Err(FlowError::PersistenceError(
                "Invalid indirect block data".to_string()
            ));
        }
        
        let mut addresses = Vec::new();
        for chunk in data.chunks_exact(32) {
            let mut id = [0u8; 32];
            id.copy_from_slice(chunk);
            addresses.push(id);
        }
        
        Ok(addresses)
    }
    
    /// Verify device identity for all rimsd directories
    pub async fn verify_device_continuity(&self) -> Result<(), FlowError> {
        for rimsd_dir in &self.rimsd_directories {
            self.verify_single_device(rimsd_dir).await?;
        }
        Ok(())
    }
    
    /// Verify identity of a single device
    pub async fn verify_single_device(&self, rimsd_dir: &Path) -> Result<(), FlowError> {
        let current_fingerprint = fingerprint_device(rimsd_dir).await?;
        
        let fingerprints = self.device_fingerprints.read().await;
        if let Some(previous_fingerprint) = fingerprints.get(&rimsd_dir.to_path_buf()) {
            // Check if this could be a legitimate evolution
            if !current_fingerprint.is_valid_evolution(previous_fingerprint)? {
                return Err(FlowError::PersistenceError(
                    format!("Device identity validation failed for {}: incompatible evolution", 
                           rimsd_dir.display())
                ));
            }
            
            // Run Bayesian identification
            let result = self.device_identifier.identify_device(
                &current_fingerprint, 
                previous_fingerprint
            )?;
            
            if !result.is_same_device {
                return Err(FlowError::PersistenceError(
                    format!("Device identity mismatch for {}: confidence {:.2}%, evidence: {:?}", 
                           rimsd_dir.display(), 
                           result.confidence * 100.0,
                           result.evidence_summary)
                ));
            }
        }
        
        // Update stored fingerprint
        drop(fingerprints);
        let mut fingerprints = self.device_fingerprints.write().await;
        fingerprints.insert(rimsd_dir.to_path_buf(), current_fingerprint);
        
        Ok(())
    }
    
    /// Initialize device fingerprints for all rimsd directories
    pub async fn initialize_device_fingerprints(&self) -> Result<(), FlowError> {
        for rimsd_dir in &self.rimsd_directories {
            let fingerprint = fingerprint_device(rimsd_dir).await?;
            let mut fingerprints = self.device_fingerprints.write().await;
            fingerprints.insert(rimsd_dir.to_path_buf(), fingerprint);
        }
        Ok(())
    }
}
