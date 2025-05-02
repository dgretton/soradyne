//! Real-time Self-Data Objects
//!
//! This module implements Self-Data Objects that prioritize low-latency
//! real-time updates, such as heart rate data or robot state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use uuid::Uuid;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};

use super::base::{SelfDataObject, SDOMetadata, SDOType, SDOError, VersionVector};

/// A real-time Self-Data Object
///
/// This type of SDO prioritizes low-latency updates and is suitable for
/// data that changes frequently, such as heart rate or robot state.
pub struct RealtimeSDO<T: Send + Sync + Clone + 'static> {
    /// Metadata for this SDO
    metadata: SDOMetadata,
    
    /// The current value of this SDO
    value: Arc<Mutex<T>>,
    
    /// Historical values of this SDO, keyed by timestamp
    history: Arc<Mutex<Vec<(chrono::DateTime<chrono::Utc>, T)>>>,
    
    /// Channel for broadcasting updates
    update_tx: broadcast::Sender<()>,
    
    /// Subscriptions to this SDO
    subscriptions: Arc<Mutex<HashMap<Uuid, Box<dyn Fn() + Send + Sync>>>>,
}

impl<T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static> RealtimeSDO<T> {
    /// Create a new real-time SDO
    pub fn new(name: &str, owner_id: Uuid, initial_value: T) -> Self {
        let metadata = SDOMetadata::new(name, SDOType::RealTime, owner_id);
        let (update_tx, _) = broadcast::channel(16);
        
        Self {
            metadata,
            value: Arc::new(Mutex::new(initial_value)),
            history: Arc::new(Mutex::new(Vec::new())),
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
    
    /// Set the value of this SDO
    pub fn set_value(&self, identity_id: Uuid, value: T) -> Result<(), SDOError> {
        // Check if the identity has write access
        if !self.metadata.has_access(identity_id, super::base::SDOAccess::Write) {
            return Err(SDOError::AccessDenied);
        }
        
        // Update the value
        {
            let mut value_guard = self.value.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            *value_guard = value.clone();
        }
        
        // Add to history
        {
            let mut history_guard = self.history.lock()
                .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
            history_guard.push((chrono::Utc::now(), value));
        }
        
        // Notify subscribers
        let _ = self.update_tx.send(());
        
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
    
    /// Get the historical values of this SDO within a time range
    pub fn get_history(&self, 
                       start: chrono::DateTime<chrono::Utc>,
                       end: chrono::DateTime<chrono::Utc>) 
                       -> Result<Vec<(chrono::DateTime<chrono::Utc>, T)>, SDOError> {
        let history_guard = self.history.lock()
            .map_err(|_| SDOError::InvalidOperation("Failed to acquire lock".into()))?;
        
        let filtered = history_guard.iter()
            .filter(|(timestamp, _)| *timestamp >= start && *timestamp <= end)
            .cloned()
            .collect();
        
        Ok(filtered)
    }
}

#[async_trait]
impl<T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static> SelfDataObject for RealtimeSDO<T> {
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
        
        // Set the value
        self.set_value(identity_id, value)?;
        
        // Update metadata
        self.metadata.update_modified();
        self.metadata.version.increment(identity_id);
        
        Ok(())
    }
    
    fn is_compatible_with(&self, sdo_type: SDOType) -> bool {
        sdo_type == SDOType::RealTime || sdo_type == SDOType::Hybrid
    }
    
    fn clone_box(&self) -> Box<dyn SelfDataObject> {
        // This is a bit of a hack, but it allows us to clone the SDO
        // We can't implement Clone for dyn SelfDataObject directly
        unimplemented!("Cloning RealtimeSDO is not yet supported")
    }
}
