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
