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

pub use conflict::{ConflictResolver, LastWriteWins};
pub use error::FlowError;

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
    resolver: Arc<dyn ConflictResolver<T> + Send + Sync>
}

impl<T> SelfDataFlow<T>
where
    T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static,
{
    /// Create a new SelfDataFlow
    pub fn new(name: &str, owner_id: Uuid, initial_value: T) -> Self {
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
        }
    }

    /// Get the current value
    pub fn get_value(&self) -> Option<T> {
        self.value.lock().ok().map(|v| v.clone())
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

    /// Persist to disk as JSON
    pub fn persist_to_disk(&self, path: &str) {
        if let Ok(guard) = self.value.lock() {
            if let Ok(json) = serde_json::to_string(&*guard) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Load from disk
    pub fn load_from_disk(path: &str) -> Option<T> {
        if let Ok(mut file) = File::open(path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(value) = serde_json::from_str(&contents) {
                    return Some(value);
                }
            }
        }
        None
    }
}

