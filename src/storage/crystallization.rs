//! Data crystallization for Soradyne
//!
//! This module implements data crystallization, which is the process of
//! consolidating dissolved data into a single, easily accessible form.

use std::path::PathBuf;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;

use super::{StorageError, StorageConfig};
use super::dissolution::DissolutionMetadata;

/// Metadata for a crystallized data object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrystallizationMetadata {
    /// Unique identifier for the crystallized data
    pub id: Uuid,
    
    /// Original dissolution metadata
    pub dissolution: DissolutionMetadata,
    
    /// Where the crystallized data is stored
    pub path: PathBuf,
    
    /// Size of the crystallized data
    pub size: usize,
    
    /// Whether the data is encrypted
    pub encrypted: bool,
}

/// Manager for data crystallization
pub struct CrystallizationManager {
    /// Configuration for this crystallization manager
    config: StorageConfig,
    
    /// Base directory for crystallized data
    base_dir: PathBuf,
    
    /// Dissolution manager for reconstructing data
    dissolution_manager: super::dissolution::DissolutionManager,
}

impl CrystallizationManager {
    /// Create a new crystallization manager
    pub fn new(
        config: StorageConfig,
        dissolution_manager: super::dissolution::DissolutionManager,
    ) -> Self {
        let base_dir = config.base_dir.join("crystallized");
        
        Self {
            config,
            base_dir,
            dissolution_manager,
        }
    }
    
    /// Crystallize data from a dissolution
    pub async fn crystallize(
        &self,
        dissolution_metadata: &DissolutionMetadata,
    ) -> Result<CrystallizationMetadata, StorageError> {
        // Ensure base directory exists
        if !self.base_dir.exists() {
            tokio_fs::create_dir_all(&self.base_dir).await?;
        }
        
        // Generate a unique ID for this crystallization
        let crystallization_id = Uuid::new_v4();
        
        // Path for the crystallized data
        let path = self.base_dir.join(crystallization_id.to_string());
        
        // Reconstruct the data from shards
        let data = self.dissolution_manager.crystallize(dissolution_metadata).await?;
        
        // Write the crystallized data to disk
        let mut file = tokio_fs::File::create(&path).await?;
        file.write_all(&data).await?;
        
        // Create and return metadata
        let metadata = CrystallizationMetadata {
            id: crystallization_id,
            dissolution: dissolution_metadata.clone(),
            path,
            size: data.len(),
            encrypted: dissolution_metadata.encrypted,
        };
        
        Ok(metadata)
    }
    
    /// Retrieve crystallized data
    pub async fn retrieve(&self, metadata: &CrystallizationMetadata) -> Result<Vec<u8>, StorageError> {
        // Check if the file exists
        if !metadata.path.exists() {
            return Err(StorageError::NotFound(metadata.id));
        }
        
        // Read the file
        let data = tokio_fs::read(&metadata.path).await?;
        
        // Decrypt if necessary (placeholder for actual decryption)
        let final_data = if metadata.encrypted {
            // In a real implementation, use proper decryption
            // This is just a placeholder
            data
        } else {
            data
        };
        
        Ok(final_data)
    }
    
    /// Delete crystallized data
    pub async fn delete(&self, metadata: &CrystallizationMetadata) -> Result<(), StorageError> {
        // Check if the file exists
        if !metadata.path.exists() {
            return Err(StorageError::NotFound(metadata.id));
        }
        
        // Delete the file
        tokio_fs::remove_file(&metadata.path).await?;
        
        Ok(())
    }
    
    /// List all crystallized data
    pub async fn list(&self) -> Result<Vec<Uuid>, StorageError> {
        let mut ids = Vec::new();
        
        // Ensure base directory exists
        if !self.base_dir.exists() {
            return Ok(ids);
        }
        
        // Read all files in the base directory
        let mut entries = tokio_fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(file_name) = path.file_name() {
                    if let Some(file_str) = file_name.to_str() {
                        if let Ok(id) = Uuid::parse_str(file_str) {
                            ids.push(id);
                        }
                    }
                }
            }
        }
        
        Ok(ids)
    }
}
