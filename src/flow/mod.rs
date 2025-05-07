//! flow/mod.rs
//!
//! The core SelfDataFlow abstraction, extended for a live heartrate sync demo.
//! This module defines the SelfDataFlow type, metadata, and full operations
//! for live distributed data flows with eventual consistency.

mod conflict;
mod error;
mod subscription;
mod persistence;
mod routing;
pub mod traits;

pub use conflict::{ConflictResolver, LastWriteWins};
pub use error::FlowError;
pub use traits::{StorageBackend, FlowAuthenticator, FlowType};

use std::collections::HashMap;
use std::fs::{File};
use std::io::{Read};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};


/// Metadata for a SelfDataFlow
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowMetadata {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub version: u64,
}

/// The core SelfDataFlow type
pub struct SelfDataFlow<T: Send + Sync + Clone + 'static> {
    value: Arc<Mutex<T>>,
    metadata: Arc<Mutex<FlowMetadata>>,
    subscribers: Arc<Mutex<HashMap<Uuid, Box<dyn Fn(&T) + Send + Sync>>>>,
    update_tx: broadcast::Sender<T>,
    resolver: Arc<dyn ConflictResolver<T> + Send + Sync>,
    storage: Option<Arc<dyn StorageBackend + Send + Sync>>,
    authenticator: Option<Arc<dyn FlowAuthenticator<T> + Send + Sync>>,
    flow_type: FlowType,
}

impl<T> SelfDataFlow<T>
where
    T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static,
{
    /// Create a new SelfDataFlow
    pub fn new(name: &str, owner_id: Uuid, initial_value: T, flow_type: FlowType) -> Self {
        let metadata = FlowMetadata {
            id: Uuid::new_v4(),
            name: name.to_string(),
            owner_id,
            version: 1,
        };
        let (update_tx, _) = broadcast::channel(32);

        Self {
            metadata: Arc::new(Mutex::new(metadata)),
            value: Arc::new(Mutex::new(initial_value)),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            update_tx,
            resolver: Arc::new(LastWriteWins),
            storage: None,
            authenticator: None,
            flow_type,
        }
    }

    /// Set the storage backend for this flow
    pub fn with_storage(mut self, storage: impl StorageBackend + Send + Sync + 'static) -> Self {
        self.storage = Some(Arc::new(storage));
        self
    }

    /// Set the authenticator for this flow
    pub fn with_authenticator(mut self, authenticator: impl FlowAuthenticator<T> + Send + Sync + 'static) -> Self {
        self.authenticator = Some(Arc::new(authenticator));
        self
    }
    
    /// Get the flow type
    pub fn flow_type(&self) -> FlowType {
        self.flow_type
    }

    /// Get the current value
    pub fn get_value(&self) -> Option<T> {
        self.value.lock().ok().map(|v| v.clone())
    }
    
    /// Create a new SelfDataFlow with default settings (for backward compatibility)
    pub fn new_default(name: &str, owner_id: Uuid, initial_value: T) -> Self {
        Self::new(name, owner_id, initial_value, FlowType::Custom)
    }

    /// Update the value and notify subscribers
    pub fn update(&self, new_value: T) {
        {
            let mut value = self.value.lock().unwrap();
            *value = new_value.clone();
        }
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.version += 1;
        }
        if let Ok(mut guard) = self.value.lock() {
            *guard = new_value.clone();
            let _ = self.update_tx.send(new_value.clone());
            if let Ok(subs) = self.subscribers.lock() {
                for callback in subs.values() {
                    callback(&new_value);
                }
            }
        }
    }

    /// Merge a remote value
    pub fn merge(&self, remote_value: T) {
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.version += 1;
        }
        if let Ok(mut guard) = self.value.lock() {
            let resolved = self.resolver.resolve(&*guard, &remote_value);
            *guard = resolved.clone();
            let _ = self.update_tx.send(resolved.clone());
            if let Ok(subs) = self.subscribers.lock() {
                for callback in subs.values() {
                    callback(&resolved);
                }
            }
        }
    }

    /// Subscribe to updates
    pub fn subscribe(&self, callback: Box<dyn Fn(&T) + Send + Sync>) -> Uuid {
        let id = Uuid::new_v4();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.insert(id, callback);
        }
        id
    }

    /// Unsubscribe
    pub fn unsubscribe(&self, id: Uuid) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.remove(&id);
        }
    }
    
    /// Sign the current flow data
    pub fn sign(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(authenticator) = &self.authenticator {
            if let Ok(guard) = self.value.lock() {
                return authenticator.sign(&*guard);
            }
            Err(FlowError::PersistenceError("Failed to access flow data".to_string()))
        } else {
            Err(FlowError::PersistenceError("No authenticator configured".to_string()))
        }
    }
    
    /// Verify a signature for the current flow data
    pub fn verify(&self, signature: &[u8]) -> bool {
        if let Some(authenticator) = &self.authenticator {
            if let Ok(guard) = self.value.lock() {
                return authenticator.verify(&*guard, signature);
            }
        }
        false
    }

    /// Persist the flow data using the configured storage backend
    pub fn persist(&self) -> Result<(), FlowError> {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                if let Ok(guard) = self.value.lock() {
                    if let Ok(json) = serde_json::to_string(&*guard) {
                        return storage.store(metadata.id, json.as_bytes());
                    }
                }
            }
            Err(FlowError::PersistenceError("Failed to serialize flow data".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }

    /// Load flow data from the configured storage backend
    pub fn load(&mut self) -> Result<(), FlowError> {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                let data = storage.load(metadata.id)?;
                if let Ok(value) = serde_json::from_slice::<T>(&data) {
                    if let Ok(mut guard) = self.value.lock() {
                        *guard = value;
                        return Ok(());
                    }
                }
                return Err(FlowError::PersistenceError("Failed to deserialize flow data".to_string()));
            }
            Err(FlowError::PersistenceError("Failed to access flow metadata".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }
    
    /// Check if flow data exists in the storage backend
    pub fn exists(&self) -> bool {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                return storage.exists(metadata.id);
            }
        }
        false
    }
    
    /// Delete flow data from the storage backend
    pub fn delete(&self) -> Result<(), FlowError> {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                return storage.delete(metadata.id);
            }
            Err(FlowError::PersistenceError("Failed to access flow metadata".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }
}

