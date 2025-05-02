//! Storage mechanisms for Soradyne
//!
//! This module handles data storage, dissolution, and crystallization.

pub mod local;
pub mod dissolution;
pub mod crystallization;

use std::path::PathBuf;
use uuid::Uuid;
use thiserror::Error;

pub use local::LocalStorage;
pub use dissolution::DissolutionManager;
pub use crystallization::CrystallizationManager;

/// Error types for storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("SDO not found: {0}")]
    NotFound(Uuid),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Invalid dissolution: {0}")]
    InvalidDissolution(String),
    
    #[error("Insufficient shards: {0}")]
    InsufficientShards(String),
}

/// Trait for storage providers
#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    /// Store data and return a unique identifier
    async fn store(&self, data: Vec<u8>) -> Result<Uuid, StorageError>;
    
    /// Retrieve data by its identifier
    async fn retrieve(&self, id: Uuid) -> Result<Vec<u8>, StorageError>;
    
    /// Check if data exists
    async fn exists(&self, id: Uuid) -> Result<bool, StorageError>;
    
    /// Delete data by its identifier
    async fn delete(&self, id: Uuid) -> Result<(), StorageError>;
    
    /// List all stored data identifiers
    async fn list(&self) -> Result<Vec<Uuid>, StorageError>;
}

/// Configuration for storage providers
#[derive(Clone, Debug)]
pub struct StorageConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,
    
    /// Number of shards to use for dissolution
    pub shard_count: usize,
    
    /// Minimum number of shards needed for reconstruction
    pub threshold: usize,
    
    /// Whether to encrypt data before storage
    pub encrypt: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("./soradyne_data"),
            shard_count: 5,
            threshold: 3,
            encrypt: true,
        }
    }
}
