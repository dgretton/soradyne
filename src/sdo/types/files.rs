//! File Sharing Self-Data Object for Soradyne
//!
//! This module implements an eventually consistent SDO for file sharing.

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashSet;

use crate::sdo::{EventualSDO, SDOType, SDOError};

/// File metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Unique identifier for this file
    pub id: Uuid,
    
    /// The identity that uploaded this file
    pub uploader_id: Uuid,
    
    /// The filename
    pub filename: String,
    
    /// The MIME type of the file
    pub mime_type: String,
    
    /// The size of the file in bytes
    pub size: usize,
    
    /// When this file was uploaded
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
    
    /// Optional description of the file
    pub description: Option<String>,
    
    /// Tags associated with the file
    pub tags: Vec<String>,
    
    /// Whether this file is available for download
    pub available: bool,
}

/// A file sharing repository
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRepository {
    /// Files in this repository
    pub files: Vec<FileMetadata>,
    
    /// Users who can access this repository
    pub users: HashSet<Uuid>,
    
    /// The name of this repository
    pub name: String,
    
    /// Optional description of the repository
    pub description: Option<String>,
}

impl FileRepository {
    /// Create a new file repository
    pub fn new(owner_id: Uuid, name: String, description: Option<String>) -> Self {
        let mut users = HashSet::new();
        users.insert(owner_id);
        
        Self {
            files: Vec::new(),
            users,
            name,
            description,
        }
    }
    
    /// Add a file to this repository
    pub fn add_file(&mut self, uploader_id: Uuid, metadata: FileMetadata) -> Result<(), SDOError> {
        // Check if the uploader has access
        if !self.users.contains(&uploader_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Add the file to the repository
        self.files.push(metadata);
        
        Ok(())
    }
    
    /// Remove a file from this repository
    pub fn remove_file(&mut self, remover_id: Uuid, file_id: Uuid) -> Result<(), SDOError> {
        // Check if the remover has access
        if !self.users.contains(&remover_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Find the file
        let index = self.files.iter()
            .position(|f| f.id == file_id)
            .ok_or_else(|| SDOError::NotFound(file_id))?;
        
        // Remove the file
        self.files.remove(index);
        
        Ok(())
    }
    
    /// Add a user to this repository
    pub fn add_user(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.users.insert(identity_id);
        Ok(())
    }
    
    /// Remove a user from this repository
    pub fn remove_user(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.users.remove(&identity_id);
        Ok(())
    }
}

/// File SDO
///
/// This SDO is used to share files between devices.
pub type FileSDO = EventualSDO<FileRepository>;

impl FileSDO {
    /// Create a new file repository SDO
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        // Create an initial empty repository
        let initial_data = FileRepository::new(owner_id, name.to_string(), None);
        
        // Create the SDO
        EventualSDO::new(name, owner_id, initial_data)
    }
    
    /// Get the repository
    pub fn get_repository(&self) -> Result<FileRepository, SDOError> {
        self.get_value()
    }
    
    // Additional methods would be implemented here
}
