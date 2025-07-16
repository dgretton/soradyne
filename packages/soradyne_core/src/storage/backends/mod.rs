//! Storage backend implementations

pub mod manual_erasure;
pub mod bcachefs;

pub use manual_erasure::ManualErasureBackend;

// Only expose bcachefs on Linux
#[cfg(target_os = "linux")]
pub use bcachefs::BcacheFSBackend;

use std::path::PathBuf;
use crate::storage::dissolution::{DissolutionStorage, DissolutionConfig, BackendConfig};
use crate::flow::FlowError;
use std::sync::Arc;

/// Factory for creating dissolution storage backends
pub struct DissolutionStorageFactory;

impl DissolutionStorageFactory {
    /// Create a storage backend from configuration
    pub async fn create(config: DissolutionConfig) -> Result<Box<dyn DissolutionStorage>, FlowError> {
        match &config.backend_config {
            BackendConfig::ManualErasure { rimsd_paths, metadata_path } => {
                let backend = ManualErasureBackend::new(
                    rimsd_paths.clone(),
                    metadata_path.clone(),
                    config.clone(),
                ).await?;
                Ok(Box::new(backend))
            },
            BackendConfig::BcacheFS { .. } => {
                #[cfg(target_os = "linux")]
                {
                    let backend = bcachefs::BcacheFSBackend::new(config.clone()).await?;
                    Ok(Box::new(backend))
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
        let mut backends = vec!["manual_erasure".to_string()];
        
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
    
    /// Create a configuration for manual erasure backend with auto-discovery
    pub async fn create_manual_erasure_config(
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
            backend_config: BackendConfig::ManualErasure {
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
        
        // Prefer bcachefs on Linux if available, otherwise use manual erasure
        if available.contains(&"bcachefs".to_string()) {
            // TODO: Implement bcachefs config creation
            Self::create_manual_erasure_config(threshold, total_shards, metadata_path).await
        } else {
            Self::create_manual_erasure_config(threshold, total_shards, metadata_path).await
        }
    }
}
