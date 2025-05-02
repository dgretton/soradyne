//! Base implementations for Self-Data Objects
//!
//! This module defines the foundational interfaces and traits for all SDOs.

use std::collections::HashMap;
use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::core::identity::Identity;

/// Error types for SDO operations
#[derive(Error, Debug)]
pub enum SDOError {
    #[error("Access denied")]
    AccessDenied,
    
    #[error("Version conflict")]
    VersionConflict,
    
    #[error("SDO not found: {0}")]
    NotFound(Uuid),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Types of access that can be granted to an SDO
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SDOAccess {
    /// Can only read the SDO
    Read,
    
    /// Can read and write to the SDO
    Write,
    
    /// Has full control over the SDO, including permissions
    Admin,
}

/// Types of SDOs
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SDOType {
    /// Real-time data (heart rate, robot state)
    RealTime,
    
    /// Eventually consistent data (chat, files)
    EventualConsistent,
    
    /// Hybrid of real-time and eventually consistent
    Hybrid,
}

/// A vector clock for versioning SDOs
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionVector {
    /// The version counters for each identity that has modified the SDO
    pub counters: HashMap<Uuid, u64>,
}

impl VersionVector {
    /// Create a new empty version vector
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }
    
    /// Increment the counter for an identity
    pub fn increment(&mut self, identity_id: Uuid) {
        let counter = self.counters.entry(identity_id).or_insert(0);
        *counter += 1;
    }
    
    /// Check if this version vector is concurrent with another
    pub fn is_concurrent_with(&self, other: &Self) -> bool {
        // Check if neither dominates the other
        !self.dominates(other) && !other.dominates(self)
    }
    
    /// Check if this version vector dominates another
    pub fn dominates(&self, other: &Self) -> bool {
        // For each identity in the other version vector
        for (id, &other_count) in &other.counters {
            // If this version vector doesn't have the identity, or has a lower count
            if let Some(&this_count) = self.counters.get(id) {
                if this_count < other_count {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        // Check that at least one counter is strictly greater
        for (id, &this_count) in &self.counters {
            if let Some(&other_count) = other.counters.get(id) {
                if this_count > other_count {
                    return true;
                }
            } else {
                return true;
            }
        }
        
        // If we get here, the vectors are equal
        false
    }
    
    /// Merge this version vector with another
    pub fn merge(&mut self, other: &Self) {
        for (id, &other_count) in &other.counters {
            let counter = self.counters.entry(*id).or_insert(0);
            *counter = (*counter).max(other_count);
        }
    }
}

impl Default for VersionVector {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata for an SDO
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SDOMetadata {
    /// Unique identifier for this SDO
    pub id: Uuid,
    
    /// Human-readable name
    pub name: String,
    
    /// Type of SDO
    pub sdo_type: SDOType,
    
    /// When this SDO was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// When this SDO was last modified
    pub modified_at: chrono::DateTime<chrono::Utc>,
    
    /// The identity that owns this SDO
    pub owner_id: Uuid,
    
    /// The current version of this SDO
    pub version: VersionVector,
    
    /// Access permissions for this SDO
    pub access: HashMap<Uuid, SDOAccess>,
    
    /// Custom user-defined metadata
    pub custom: serde_json::Value,
}

impl SDOMetadata {
    /// Create new metadata for an SDO
    pub fn new(name: &str, sdo_type: SDOType, owner_id: Uuid) -> Self {
        let now = chrono::Utc::now();
        let mut version = VersionVector::new();
        version.increment(owner_id);
        
        let mut access = HashMap::new();
        access.insert(owner_id, SDOAccess::Admin);
        
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            sdo_type,
            created_at: now,
            modified_at: now,
            owner_id,
            version,
            access,
            custom: serde_json::Value::Null,
        }
    }
    
    /// Check if an identity has a specific access level
    pub fn has_access(&self, identity_id: Uuid, required_access: SDOAccess) -> bool {
        match self.access.get(&identity_id) {
            Some(&access) => {
                match (access, required_access) {
                    // Admin can do everything
                    (SDOAccess::Admin, _) => true,
                    // Write can read and write
                    (SDOAccess::Write, SDOAccess::Read) | 
                    (SDOAccess::Write, SDOAccess::Write) => true,
                    // Read can only read
                    (SDOAccess::Read, SDOAccess::Read) => true,
                    // Otherwise, access denied
                    _ => false,
                }
            }
            None => false,
        }
    }
    
    /// Set access for an identity
    pub fn set_access(&mut self, identity_id: Uuid, access: SDOAccess) {
        self.access.insert(identity_id, access);
    }
    
    /// Remove access for an identity
    pub fn remove_access(&mut self, identity_id: &Uuid) {
        self.access.remove(identity_id);
    }
    
    /// Update the last modified timestamp
    pub fn update_modified(&mut self) {
        self.modified_at = chrono::Utc::now();
    }
}

/// Base trait for all Self-Data Objects
#[async_trait]
pub trait SelfDataObject: Send + Sync {
    /// Get the metadata for this SDO
    fn metadata(&self) -> &SDOMetadata;
    
    /// Get mutable metadata for this SDO
    fn metadata_mut(&mut self) -> &mut SDOMetadata;
    
    /// Subscribe to changes for this SDO
    ///
    /// Returns a subscription ID that can be used to unsubscribe
    async fn subscribe(&self, callback: Box<dyn Fn() + Send + Sync>) -> Result<Uuid, SDOError>;
    
    /// Unsubscribe from changes for this SDO
    async fn unsubscribe(&self, subscription_id: Uuid) -> Result<(), SDOError>;
    
    /// Get the serialized content of this SDO
    async fn get_content(&self) -> Result<Vec<u8>, SDOError>;
    
    /// Update the content of this SDO
    async fn update_content(&mut self, 
                           identity_id: Uuid, 
                           data: &[u8]) -> Result<(), SDOError>;
    
    /// Check if this SDO is compatible with a specific type
    fn is_compatible_with(&self, sdo_type: SDOType) -> bool;
    
    /// Clone this SDO
    fn clone_box(&self) -> Box<dyn SelfDataObject>;
}
