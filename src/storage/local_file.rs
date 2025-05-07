use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::flow::error::FlowError;
use crate::flow::traits::StorageBackend;

/// A storage backend that persists data to the local filesystem
pub struct LocalFileStorage {
    /// Base directory for storing flow data
    base_dir: PathBuf,
}

impl LocalFileStorage {
    /// Create a new local file storage with the specified base directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self, FlowError> {
        let path = PathBuf::from(base_dir.as_ref());
        
        // Create the directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| {
                FlowError::PersistenceError(format!("Failed to create directory: {}", e))
            })?;
        }
        
        Ok(Self { base_dir: path })
    }
    
    /// Get the file path for a specific flow ID
    fn get_file_path(&self, flow_id: Uuid) -> PathBuf {
        self.base_dir.join(format!("{}.json", flow_id))
    }
}

impl StorageBackend for LocalFileStorage {
    fn store(&self, flow_id: Uuid, data: &[u8]) -> Result<(), FlowError> {
        let file_path = self.get_file_path(flow_id);
        
        let mut file = File::create(&file_path).map_err(|e| {
            FlowError::PersistenceError(format!("Failed to create file: {}", e))
        })?;
        
        file.write_all(data).map_err(|e| {
            FlowError::PersistenceError(format!("Failed to write data: {}", e))
        })?;
        
        Ok(())
    }
    
    fn load(&self, flow_id: Uuid) -> Result<Vec<u8>, FlowError> {
        let file_path = self.get_file_path(flow_id);
        
        if !file_path.exists() {
            return Err(FlowError::PersistenceError(format!(
                "Flow data not found for ID: {}", flow_id
            )));
        }
        
        let mut file = File::open(&file_path).map_err(|e| {
            FlowError::PersistenceError(format!("Failed to open file: {}", e))
        })?;
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| {
            FlowError::PersistenceError(format!("Failed to read data: {}", e))
        })?;
        
        Ok(buffer)
    }
    
    fn exists(&self, flow_id: Uuid) -> bool {
        self.get_file_path(flow_id).exists()
    }
    
    fn delete(&self, flow_id: Uuid) -> Result<(), FlowError> {
        let file_path = self.get_file_path(flow_id);
        
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(|e| {
                FlowError::PersistenceError(format!("Failed to delete file: {}", e))
            })?;
        }
        
        Ok(())
    }
}

/// A no-op implementation of FlowAuthenticator that doesn't actually perform any
/// cryptographic operations. Useful as a placeholder until real authentication is implemented.
pub struct NoOpAuthenticator;

impl<T> crate::flow::traits::FlowAuthenticator<T> for NoOpAuthenticator {
    fn sign(&self, _data: &T) -> Result<Vec<u8>, FlowError> {
        // Return a dummy signature
        Ok(vec![0, 1, 2, 3])
    }
    
    fn verify(&self, _data: &T, _signature: &[u8]) -> bool {
        // Always verify as true
        true
    }
}
