//! Dissolution storage abstraction layer
//! 
//! This module provides a unified interface for different dissolution storage backends,
//! allowing the system to work with manual erasure coding, bcachefs, or future backends
//! through a common API.

use async_trait::async_trait;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use crate::flow::FlowError;

/// Block ID type - 32 bytes for cryptographic hashing
pub type BlockId = [u8; 32];

/// Configuration for a dissolution storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DissolutionConfig {
    /// Minimum number of shards needed to reconstruct data
    pub threshold: usize,
    /// Total number of shards to create
    pub total_shards: usize,
    /// Maximum size for direct blocks (before using indirect blocks)
    pub max_direct_block_size: usize,
    /// Backend-specific configuration
    pub backend_config: BackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendConfig {
    /// Soradyne erasure coding with custom shard distribution
    SdynErasure {
        /// Paths to rimsd directories for shard storage
        rimsd_paths: Vec<PathBuf>,
        /// Path for metadata storage
        metadata_path: PathBuf,
    },
    /// bcachefs-based dissolution (Linux only)
    BcacheFS {
        /// bcachefs device paths
        device_paths: Vec<PathBuf>,
        /// Mount options for SD card optimization
        mount_options: Vec<String>,
        /// Whether to disable CoW for longevity
        disable_cow: bool,
    },
    /// Future: ZFS with erasure coding
    ZFS {
        pool_name: String,
        redundancy_level: String,
    },
    /// Future: Custom distributed filesystem
    Custom {
        implementation_name: String,
        config: serde_json::Value,
    },
}

/// Information about a stored block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub id: BlockId,
    pub size: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_indirect: bool,
    pub shard_count: usize,
    pub available_shards: usize,
    pub can_reconstruct: bool,
}

/// Result of a dissolution demonstration/test
#[derive(Debug, Clone)]
pub struct DissolutionDemo {
    pub block_id: BlockId,
    pub original_shards: usize,
    pub simulated_missing: Vec<usize>,
    pub available_shards: usize,
    pub threshold_required: usize,
    pub can_reconstruct: bool,
    pub reconstruction_successful: bool,
    pub data_integrity_verified: bool,
    pub recovered_data_size: usize,
}

/// Statistics about storage usage and health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_blocks: usize,
    pub total_size_bytes: u64,
    pub available_devices: usize,
    pub total_devices: usize,
    pub health_score: f64, // 0.0 to 1.0
    pub device_health: Vec<DeviceHealth>,
    pub reconstruction_capability: ReconstructionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHealth {
    pub device_id: String,
    pub path: PathBuf,
    pub available: bool,
    pub free_space_bytes: u64,
    pub total_space_bytes: u64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructionStats {
    pub blocks_at_risk: usize, // blocks with exactly threshold shards
    pub blocks_safe: usize,    // blocks with > threshold shards
    pub blocks_lost: usize,    // blocks with < threshold shards
}

/// Core abstraction for dissolution storage backends
#[async_trait]
pub trait DissolutionStorage: Send + Sync {
    /// Store data using dissolution, returning a block ID
    async fn store(&self, data: &[u8]) -> Result<BlockId, FlowError>;
    
    /// Retrieve data by block ID, reconstructing from shards
    async fn retrieve(&self, block_id: &BlockId) -> Result<Vec<u8>, FlowError>;
    
    /// Check if a block exists and can be reconstructed
    async fn exists(&self, block_id: &BlockId) -> Result<bool, FlowError>;
    
    /// Get information about a stored block
    async fn block_info(&self, block_id: &BlockId) -> Result<BlockInfo, FlowError>;
    
    /// Delete a block and its shards
    async fn delete(&self, block_id: &BlockId) -> Result<(), FlowError>;
    
    /// List all available blocks
    async fn list_blocks(&self) -> Result<Vec<BlockId>, FlowError>;
    
    /// Get storage statistics
    async fn storage_stats(&self) -> Result<StorageStats, FlowError>;
    
    /// Demonstrate dissolution by simulating shard loss
    async fn demonstrate_dissolution(&self, block_id: &BlockId, simulate_missing: Vec<usize>) -> Result<DissolutionDemo, FlowError>;
    
    /// Perform maintenance operations (compaction, defrag, etc.)
    async fn maintenance(&self) -> Result<(), FlowError>;
    
    /// Get the current configuration
    fn config(&self) -> &DissolutionConfig;
    
    /// Update configuration (where possible)
    async fn update_config(&mut self, config: DissolutionConfig) -> Result<(), FlowError>;
    
    /// Verify device identity and continuity
    async fn verify_device_continuity(&self) -> Result<(), FlowError>;
    
    /// Initialize device fingerprints
    async fn initialize_device_fingerprints(&self) -> Result<(), FlowError>;
}

/// High-level file interface that works with any dissolution backend
pub struct DissolutionFile {
    storage: crate::storage::backends::DissolutionBackend,
    root_block: Option<BlockId>,
    size: usize,
}

impl DissolutionFile {
    /// Create a new file
    pub fn new(storage: crate::storage::backends::DissolutionBackend) -> Self {
        Self {
            storage,
            root_block: None,
            size: 0,
        }
    }
    
    /// Open existing file from root block
    pub fn from_existing(storage: crate::storage::backends::DissolutionBackend, root_block: BlockId, size: usize) -> Self {
        Self {
            storage,
            root_block: Some(root_block),
            size,
        }
    }
    
    /// Write data to the file
    pub async fn write(&mut self, data: &[u8]) -> Result<(), FlowError> {
        let block_id = self.storage.store(data).await?;
        self.root_block = Some(block_id);
        self.size = data.len();
        Ok(())
    }
    
    /// Read the entire file
    pub async fn read(&self) -> Result<Vec<u8>, FlowError> {
        match self.root_block {
            Some(block_id) => self.storage.retrieve(&block_id).await,
            None => Ok(vec![]),
        }
    }
    
    /// Get the root block ID for storage/sharing
    pub fn root_block(&self) -> Option<BlockId> {
        self.root_block
    }
    
    /// Get file size
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// Check if file exists and can be reconstructed
    pub async fn exists(&self) -> Result<bool, FlowError> {
        match self.root_block {
            Some(block_id) => self.storage.exists(&block_id).await,
            None => Ok(false),
        }
    }
    
    /// Get detailed information about the file's storage
    pub async fn info(&self) -> Result<Option<BlockInfo>, FlowError> {
        match self.root_block {
            Some(block_id) => Ok(Some(self.storage.block_info(&block_id).await?)),
            None => Ok(None),
        }
    }
}
