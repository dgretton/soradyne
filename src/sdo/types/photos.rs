//! Photo Album Self-Data Object for Soradyne
//!
//! This module implements an eventually consistent SDO for photo albums.

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashSet;

use crate::sdo::{EventualSDO, SDOType, SDOError};

/// Photo metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhotoMetadata {
    /// Unique identifier for this photo
    pub id: Uuid,
    
    /// The identity that uploaded this photo
    pub uploader_id: Uuid,
    
    /// The filename of the photo
    pub filename: String,
    
    /// The MIME type of the photo
    pub mime_type: String,
    
    /// The size of the photo in bytes
    pub size: usize,
    
    /// When this photo was uploaded
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
    
    /// Optional description of the photo
    pub description: Option<String>,
    
    /// Tags associated with the photo
    pub tags: Vec<String>,
}

/// A photo album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhotoAlbum {
    /// Photos in this album
    pub photos: Vec<PhotoMetadata>,
    
    /// Participants who can view this album
    pub participants: HashSet<Uuid>,
    
    /// The name of this album
    pub name: String,
    
    /// Optional description of the album
    pub description: Option<String>,
}

impl PhotoAlbum {
    /// Create a new photo album
    pub fn new(owner_id: Uuid, name: String, description: Option<String>) -> Self {
        let mut participants = HashSet::new();
        participants.insert(owner_id);
        
        Self {
            photos: Vec::new(),
            participants,
            name,
            description,
        }
    }
    
    /// Add a photo to this album
    pub fn add_photo(&mut self, uploader_id: Uuid, metadata: PhotoMetadata) -> Result<(), SDOError> {
        // Check if the uploader is a participant
        if !self.participants.contains(&uploader_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Add the photo to the album
        self.photos.push(metadata);
        
        Ok(())
    }
    
    /// Remove a photo from this album
    pub fn remove_photo(&mut self, remover_id: Uuid, photo_id: Uuid) -> Result<(), SDOError> {
        // Check if the remover is a participant
        if !self.participants.contains(&remover_id) {
            return Err(SDOError::AccessDenied);
        }
        
        // Find the photo
        let index = self.photos.iter()
            .position(|p| p.id == photo_id)
            .ok_or_else(|| SDOError::NotFound(photo_id))?;
        
        // Remove the photo
        self.photos.remove(index);
        
        Ok(())
    }
    
    /// Add a participant to this album
    pub fn add_participant(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.participants.insert(identity_id);
        Ok(())
    }
    
    /// Remove a participant from this album
    pub fn remove_participant(&mut self, identity_id: Uuid) -> Result<(), SDOError> {
        self.participants.remove(&identity_id);
        Ok(())
    }
}

/// Photo Album Self-Data Object
///
/// This SDO is used to share photo albums between devices.
pub type PhotoAlbumSDO = EventualSDO<PhotoAlbum>;

impl PhotoAlbumSDO {
    /// Create a new photo album SDO
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        // Create an initial empty album
        let initial_data = PhotoAlbum::new(owner_id, name.to_string(), None);
        
        // Create the SDO
        EventualSDO::new(name, owner_id, initial_data)
    }
    
    /// Get the album
    pub fn get_album(&self) -> Result<PhotoAlbum, SDOError> {
        self.get_value()
    }
    
    // Additional methods would be implemented here
}
