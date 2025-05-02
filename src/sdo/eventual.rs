//! Eventually consistent Self-Data Objects
//!
//! This module implements Self-Data Objects that prioritize conflict-free
//! eventual consistency, such as chat messages or photo albums.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use uuid::Uuid;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};

use super::base::{SelfDataObject, SDOMetadata, SDOType, SDOError, VersionVector};

/// An eventually consistent Self-Data Object
///
/// This type of SDO prioritizes conflict-free eventual consistency and is suitable
/// for data that needs to be synchronized across multiple devices, such as chat
/// messages or photo albums.
pub struct EventualSDO<T: Send + Sync + Clone + 'static> {
    /// Metadata for this SDO
    metadata: SDOMetadata,
    
    /// The current value of this SDO
    value: Arc<Mutex<T>>,
    
    /// The changes to this SDO, keyed by version
    changes: Arc<Mutex<Vec<(VersionVector, T)>>>,
    
    /// Channel for broadcasting updates
    update_tx: broadcast::Sender<VersionVector>,
    
    /// Subscriptions to this SDO
    subscriptions: Arc<Mutex<HashMap<Uuid, Box<dyn Fn() + Send + Sync>>>>,
}

impl<T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static> EventualSDO<T> {
    /// Create a new eventually consistent SDO
    pub fn new(name: &str, owner_id: Uuid, initial_value: T) -> Self {
        let metadata = SDOMetadata::new(name, SDOType::EventualConsistent, owner_id);
        let (update_tx, _) = broadcast::channel(16);
        
        let mut changes = Vec::new();
        changes.push((metadata.version.clone(), initial_value.clone()));
        
        Self {
            metadata,
            value: Arc::new(Mutex::new(initial_value)),
            changes: Arc::new(Mutex::new(changes)),
            update_tx,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Get the current value of this SDO
    pub fn get_value(&self) -> Result<T, SDOError> {
        match self.value.lock() {
            Ok(value) => Ok(value.clone()),
            Err(_) => Err(SDOError::InvalidOperation("Failed to acquire lock".into())),
        }
    }
    
    /// Apply a change to this SDO
    pub fn apply_change(&mut self, identity_id: Uuid, value: T, remote_version: Option<VersionVector>) 
        -> Result<VersionVector, SDOError> {
        // Check if the identity has write access
        if !self.metadata.has_access(identity_id, super::base::SDOAccess::Write) {
            return Err(SDOError::AccessDenied);
        }
        
        // Create a new version for this change
        let mut new_version = match remote_version {
            Some(v) => v,
            None => self.metadata.version.clone(),
        };
        
        // Increment the version counter for this identity
        new_version.increment(identity_id);
        
        // Update the value
        {
            let mut value_guard = self.value.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            *value_guard = value.clone();
        }
        
        // Add to changes
        {
            let mut changes_guard = self.changes.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            changes_guard.push((new_version.clone(), value));
        }
        
        // Update metadata
        self.metadata.update_modified();
        self.metadata.version = new_version.clone();
        
        // Notify subscribers
        let _ = self.update_tx.send(new_version.clone());
        
        // Call subscription callbacks
        {
            let subscriptions = self.subscriptions.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            
            for callback in subscriptions.values() {
                callback();
            }
        }
        
        Ok(new_version)
    }
    
    /// Get changes to this SDO since a specific version
    pub fn get_changes_since(&self, since: &VersionVector) 
        -> Result<Vec<(VersionVector, T)>, SDOError> {
        let changes_guard = self.changes.lock()
            .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            
        let filtered = changes_guard.iter()
            .filter(|(version, _)| !since.dominates(version))
            .cloned()
            .collect();
            
        Ok(filtered)
    }
    
    /// Merge changes from another instance of this SDO
    pub fn merge(&mut self, changes: Vec<(VersionVector, T)>) -> Result<(), SDOError> {
        if changes.is_empty() {
            return Ok(());
        }
        
        // Check if any changes are newer than our current version
        let mut has_newer = false;
        for (version, _) in &changes {
            if !self.metadata.version.dominates(version) {
                has_newer = true;
                break;
            }
        }
        
        if !has_newer {
            return Ok(());
        }
        
        // Find the latest change
        let latest = changes.iter().max_by_key(|(version, _)| {
            version.counters.values().sum::<u64>()
        }).unwrap();
        
        // Update our value to the latest change
        {
            let mut value_guard = self.value.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            *value_guard = latest.1.clone();
        }
        
        // Merge all changes into our changes
        {
            let mut changes_guard = self.changes.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
                
            for change in changes {
                // Check if we already have this version
                if !changes_guard.iter().any(|(v, _)| v == &change.0) {
                    changes_guard.push(change);
                }
            }
        }
        
        // Update our version to include all changes
        for (version, _) in &changes {
            self.metadata.version.merge(version);
        }
        
        // Update metadata
        self.metadata.update_modified();
        
        // Notify subscribers
        let _ = self.update_tx.send(self.metadata.version.clone());
        
        // Call subscription callbacks
        {
            let subscriptions = self.subscriptions.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            
            for callback in subscriptions.values() {
                callback();
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl<T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static> SelfDataObject for EventualSDO<T> {
    fn metadata(&self) -> &SDOMetadata {
        &self.metadata
    }
    
    fn metadata_mut(&mut self) -> &mut SDOMetadata {
        &mut self.metadata
    }
    
    async fn subscribe(&self, callback: Box<dyn Fn() + Send + Sync>) -> Result<Uuid, SDOError> {
        let subscription_id = Uuid::new_v4();
        
        let mut subscriptions = self.subscriptions.lock()
            .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            
        subscriptions.insert(subscription_id, callback);
        
        Ok(subscription_id)
    }
    
    async fn unsubscribe(&self, subscription_id: Uuid) -> Result<(), SDOError> {
        let mut subscriptions = self.subscriptions.lock()
            .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            
        if subscriptions.remove(&subscription_id).is_none() {
            return Err(SDOError::NotFound(subscription_id));
        }
        
        Ok(())
    }
    
    async fn get_content(&self) -> Result<Vec<u8>, SDOError> {
        let value = self.get_value()?;
        
        serde_json::to_vec(&value)
            .map_err(|e| SDOError::SerializationError(e.to_string()))
    }
    
    async fn update_content(&mut self, identity_id: Uuid, data: &[u8]) -> Result<(), SDOError> {
        // Deserialize the value
        let value: T = serde_json::from_slice(data)
            .map_err(|e| SDOError::SerializationError(e.to_string()))?;
        
        // Apply the change
        self.apply_change(identity_id, value, None)?;
        
        Ok(())
    }
    
    fn is_compatible_with(&self, sdo_type: SDOType) -> bool {
        sdo_type == SDOType::EventualConsistent || sdo_type == SDOType::Hybrid
    }
    
    fn clone_box(&self) -> Box<dyn SelfDataObject> {
        // This is a bit of a hack, but it allows us to clone the SDO
        // We can't implement Clone for dyn SelfDataObject directly
        unimplemented!("Cloning EventualSDO is not yet supported")
    }
}
