//! Local file system storage provider for Soradyne
//!
//! This module implements a storage provider that uses the local file system.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use async_trait::async_trait;
use tokio::fs as tokio_fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{StorageProvider, StorageError, StorageConfig};

/// A storage provider that uses the local file system
pub struct LocalStorage {
    /// Configuration for this storage provider
    config: StorageConfig,
    
    /// Base directory for storage
    base_dir: PathBuf,
}

impl LocalStorage {
    /// Create a new local storage provider
    pub async fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let base_dir = config.base_dir.clone();
        
        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            tokio_fs::create_dir_all(&base_dir).await?;
        }
        
        Ok(Self {
            config,
            base_dir,
        })
    }
    
    /// Get the path for a specific data identifier
    fn get_path(&self, id: Uuid) -> PathBuf {
        let id_str = id.to_string();
        let prefix = &id_str[0..2];
        
        let dir = self.base_dir.join(prefix);
        dir.join(id_str)
    }
    
    /// Ensure the directory for a path exists
    async fn ensure_dir(&self, path: &Path) -> Result<(), StorageError> {
        let parent = path.parent().ok_or_else(|| {
            StorageError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Invalid path",
            ))
        })?;
        
        if !parent.exists() {
            tokio_fs::create_dir_all(parent).await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl StorageProvider for LocalStorage {
    async fn store(&self, data: Vec<u8>) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();
        let path = self.get_path(id);
        
        self.ensure_dir(&path).await?;
        
        let mut file = tokio_fs::File::create(&path).await?;
        file.write_all(&data).await?;
        
        Ok(id)
    }
    
    async fn retrieve(&self, id: Uuid) -> Result<Vec<u8>, StorageError> {
        let path = self.get_path(id);
        
        if !path.exists() {
            return Err(StorageError::NotFound(id));
        }
        
        let mut file = tokio_fs::File::open(&path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        
        Ok(data)
    }
    
    async fn exists(&self, id: Uuid) -> Result<bool, StorageError> {
        let path = self.get_path(id);
        Ok(path.exists())
    }
    
    async fn delete(&self, id: Uuid) -> Result<(), StorageError> {
        let path = self.get_path(id);
        
        if !path.exists() {
            return Err(StorageError::NotFound(id));
        }
        
        tokio_fs::remove_file(&path).await?;
        
        // Try to clean up empty directory
        if let Some(parent) = path.parent() {
            if let Ok(entries) = tokio_fs::read_dir(parent).await {
                let mut count = 0;
                let mut reader = entries;
                while let Some(_) = reader.next_entry().await? {
                    count += 1;
                    if count > 0 {
                        break;
                    }
                }
                
                if count == 0 {
                    tokio_fs::remove_dir(parent).await.ok();
                }
            }
        }
        
        Ok(())
    }
    
    async fn list(&self) -> Result<Vec<Uuid>, StorageError> {
        let mut ids = Vec::new();
        
        // Ensure base directory exists
        if !self.base_dir.exists() {
            return Ok(ids);
        }
        
        // Read all prefix directories
        let mut dir_entries = tokio_fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                let prefix = path.file_name().unwrap().to_str().unwrap();
                
                // Read all files in the prefix directory
                let mut file_entries = tokio_fs::read_dir(&path).await?;
                while let Some(file_entry) = file_entries.next_entry().await? {
                    let file_path = file_entry.path();
                    
                    if file_path.is_file() {
                        if let Some(file_name) = file_path.file_name() {
                            if let Some(file_str) = file_name.to_str() {
                                if let Ok(id) = Uuid::parse_str(file_str) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(ids)
    }
}
