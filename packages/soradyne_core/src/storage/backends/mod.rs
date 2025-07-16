//! Storage backend implementations

pub mod sdyn_erasure;
pub mod bcachefs;

pub use sdyn_erasure::SdynErasureBackend;

// Only expose bcachefs on Linux
#[cfg(target_os = "linux")]
pub use bcachefs::BcacheFSBackend;

use std::path::PathBuf;
use crate::storage::dissolution::{DissolutionStorage, DissolutionConfig, BackendConfig, BlockId, BlockInfo, StorageStats, DissolutionDemo};
use crate::flow::FlowError;
use async_trait::async_trait;

/// Concrete enum for dissolution storage backends
#[derive(Clone)]
pub enum DissolutionBackend {
    SdynErasure(SdynErasureBackend),
    #[cfg(target_os = "linux")]
    BcacheFS(BcacheFSBackend),
}

#[async_trait]
impl DissolutionStorage for DissolutionBackend {
    async fn store(&self, data: &[u8]) -> Result<BlockId, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.store(data).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.store(data).await,
        }
    }
    
    async fn retrieve(&self, block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.retrieve(block_id).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.retrieve(block_id).await,
        }
    }
    
    async fn exists(&self, block_id: &BlockId) -> Result<bool, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.exists(block_id).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.exists(block_id).await,
        }
    }
    
    async fn block_info(&self, block_id: &BlockId) -> Result<BlockInfo, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.block_info(block_id).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.block_info(block_id).await,
        }
    }
    
    async fn delete(&self, block_id: &BlockId) -> Result<(), FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.delete(block_id).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.delete(block_id).await,
        }
    }
    
    async fn list_blocks(&self) -> Result<Vec<BlockId>, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.list_blocks().await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.list_blocks().await,
        }
    }
    
    async fn storage_stats(&self) -> Result<StorageStats, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.storage_stats().await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.storage_stats().await,
        }
    }
    
    async fn demonstrate_dissolution(&self, block_id: &BlockId, simulate_missing: Vec<usize>) -> Result<DissolutionDemo, FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.demonstrate_dissolution(block_id, simulate_missing).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.demonstrate_dissolution(block_id, simulate_missing).await,
        }
    }
    
    async fn maintenance(&self) -> Result<(), FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.maintenance().await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.maintenance().await,
        }
    }
    
    fn config(&self) -> &DissolutionConfig {
        match self {
            Self::SdynErasure(backend) => backend.config(),
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.config(),
        }
    }
    
    async fn update_config(&mut self, config: DissolutionConfig) -> Result<(), FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.update_config(config).await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.update_config(config).await,
        }
    }
    
    async fn verify_device_continuity(&self) -> Result<(), FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.verify_device_continuity().await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.verify_device_continuity().await,
        }
    }
    
    async fn initialize_device_fingerprints(&self) -> Result<(), FlowError> {
        match self {
            Self::SdynErasure(backend) => backend.initialize_device_fingerprints().await,
            #[cfg(target_os = "linux")]
            Self::BcacheFS(backend) => backend.initialize_device_fingerprints().await,
        }
    }
}

/// Factory for creating dissolution storage backends
pub struct DissolutionStorageFactory;

impl DissolutionStorageFactory {
    /// Create a storage backend from configuration
    pub async fn create(config: DissolutionConfig) -> Result<DissolutionBackend, FlowError> {
        match &config.backend_config {
            BackendConfig::SdynErasure { rimsd_paths, metadata_path } => {
                let backend = SdynErasureBackend::new(
                    rimsd_paths.clone(),
                    metadata_path.clone(),
                    config.clone(),
                ).await?;
                Ok(DissolutionBackend::SdynErasure(backend))
            },
            BackendConfig::BcacheFS { .. } => {
                #[cfg(target_os = "linux")]
                {
                    let backend = bcachefs::BcacheFSBackend::new(config.clone()).await?;
                    Ok(DissolutionBackend::BcacheFS(backend))
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Err(FlowError::PersistenceError(
                        "bcachefs backend is only available on Linux".to_string()
                    ))
                }
            },
            BackendConfig::ZFS { pool_name, redundancy_level } => {
                Err(FlowError::PersistenceError(
                    format!("ZFS backend not yet implemented (pool: {}, redundancy: {})", 
                           pool_name, redundancy_level)
                ))
            },
            BackendConfig::Custom { implementation_name, .. } => {
                Err(FlowError::PersistenceError(
                    format!("Custom backend '{}' not implemented", implementation_name)
                ))
            },
        }
    }
    
    /// Detect available backends on the system
    pub async fn detect_available_backends() -> Vec<String> {
        let mut backends = vec!["sdyn_erasure".to_string()];
        
        // Check if bcachefs is available (Linux only)
        #[cfg(target_os = "linux")]
        {
            if tokio::process::Command::new("bcachefs")
                .arg("version")
                .output()
                .await
                .is_ok()
            {
                backends.push("bcachefs".to_string());
            }
        }
        
        // Check for ZFS
        if tokio::process::Command::new("zfs")
            .arg("version")
            .output()
            .await
            .is_ok()
        {
            backends.push("zfs".to_string());
        }
        
        backends
    }
    
    /// Create a configuration for sdyn erasure backend with auto-discovery
    pub async fn create_sdyn_erasure_config(
        threshold: usize,
        total_shards: usize,
        metadata_path: PathBuf,
    ) -> Result<DissolutionConfig, FlowError> {
        use crate::storage::device_identity::discover_soradyne_volumes;
        
        let rimsd_paths = discover_soradyne_volumes().await?;
        
        if rimsd_paths.is_empty() {
            return Err(FlowError::PersistenceError(
                "No Soradyne volumes found. Please initialize some SD cards first.".to_string()
            ));
        }
        
        Ok(DissolutionConfig {
            threshold,
            total_shards,
            max_direct_block_size: 32 * 1024 * 1024, // 32MB
            backend_config: BackendConfig::SdynErasure {
                rimsd_paths,
                metadata_path,
            },
        })
    }
    
    /// Create a default configuration based on available backends
    pub async fn create_default_config(
        threshold: usize,
        total_shards: usize,
        metadata_path: PathBuf,
    ) -> Result<DissolutionConfig, FlowError> {
        let available = Self::detect_available_backends().await;
        
        // Prefer bcachefs on Linux if available, otherwise use sdyn erasure
        if available.contains(&"bcachefs".to_string()) {
            // TODO: Implement bcachefs config creation
            Self::create_sdyn_erasure_config(threshold, total_shards, metadata_path).await
        } else {
            Self::create_sdyn_erasure_config(threshold, total_shards, metadata_path).await
        }
    }
}
