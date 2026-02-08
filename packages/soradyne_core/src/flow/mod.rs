//! flow/mod.rs
//!
//! The Self-Data Flow system for peer-to-peer data synchronization.
//!
//! # Concepts
//!
//! - **Flow**: A persistent, UUID'd bundle of streams with policies. The flow
//!   is the authority - once you have one, it calls the shots for how data
//!   is sent and received.
//!
//! - **Stream**: The basic I/O abstraction. Streams are named and have a
//!   category hint (drip/jet) but the category is descriptive, not enforced.
//!   - *Drip*: Eventually consistent, authoritative data
//!   - *Jet*: Fast, possibly lossy, real-time data
//!
//! - **DataChannel**: A concrete implementation of Stream for in-memory pub/sub
//!   with conflict resolution. This is the former "SelfDataFlow" - it's now
//!   understood as one way to implement a stream, not the flow itself.
//!
//! # Bootstrap sequence
//!
//! ```text
//! UUID
//!   ↓ (lookup in storage)
//! type_name + config
//!   ↓ (type_name selects constructor)
//! constructor(config)
//!   ↓
//! Flow instance with streams
//! ```

mod conflict;
pub mod error;
mod subscription;
mod persistence;
mod routing;
pub mod traits;
pub mod examples;
pub mod stream;
pub mod flow_core;

pub use conflict::{ConflictResolver, LastWriteWins};
pub use error::FlowError;
pub use traits::{StorageBackend, FlowAuthenticator, FlowType, Diffable};
pub use stream::{Stream, StreamSpec, StreamCardinality, TypedStream};
pub use flow_core::{Flow, FlowConfig, FlowSchema, FlowRegistry, FlowConfigStorage, InMemoryConfigStorage, BasicFlow};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};


/// Metadata for a DataChannel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelMetadata {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub version: u64,
}

/// A data channel for in-memory pub/sub with conflict resolution.
///
/// This is the former `SelfDataFlow<T>`. It's now understood as one concrete
/// implementation of the Stream trait - useful for in-memory reactive data,
/// but not the only way to implement a stream.
///
/// DataChannel can be used as a building block when implementing flows.
/// For example, a flow might use a DataChannel backed by a ConvergentDocument
/// for its drip stream.
pub struct DataChannel<T: Send + Sync + Clone + 'static> {
    name: String,
    value: Arc<Mutex<T>>,
    metadata: Arc<Mutex<ChannelMetadata>>,
    subscribers: Arc<Mutex<HashMap<Uuid, Box<dyn Fn(&T) + Send + Sync>>>>,
    update_tx: broadcast::Sender<T>,
    resolver: Arc<dyn ConflictResolver<T> + Send + Sync>,
    storage: Option<Arc<dyn StorageBackend + Send + Sync>>,
    authenticator: Option<Arc<dyn FlowAuthenticator<T> + Send + Sync>>,
    flow_type: FlowType,
}

impl<T> DataChannel<T>
where
    T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static,
{
    /// Create a new DataChannel
    pub fn new(name: &str, owner_id: Uuid, initial_value: T, flow_type: FlowType) -> Self {
        let metadata = ChannelMetadata {
            id: Uuid::new_v4(),
            name: name.to_string(),
            owner_id,
            version: 1,
        };
        let (update_tx, _) = broadcast::channel(32);

        Self {
            name: name.to_string(),
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

    /// Set the storage backend for this channel
    pub fn with_storage(mut self, storage: impl StorageBackend + Send + Sync + 'static) -> Self {
        self.storage = Some(Arc::new(storage));
        self
    }

    /// Set the authenticator for this channel
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

    /// Create a new DataChannel with default settings (for backward compatibility)
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

    /// Update the value using a diff and notify subscribers
    /// This is more efficient for incremental updates as it avoids sending the entire object
    pub fn update_with_diff<D>(&self, diff: &D)
    where
        T: Diffable<Diff = D>
    {
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.version += 1;
        }

        if let Ok(mut guard) = self.value.lock() {
            let new_value = guard.apply(diff);
            *guard = new_value.clone();
            let _ = self.update_tx.send(new_value.clone());
            if let Ok(subs) = self.subscribers.lock() {
                for callback in subs.values() {
                    callback(&new_value);
                }
            }
        }
    }

    /// Broadcast a diff to subscribers without updating the local value
    /// This is useful when you want to send incremental updates to peers
    pub fn broadcast_diff<D>(&self, diff: &D)
    where
        T: Diffable<Diff = D>
    {
        if let Ok(guard) = self.value.lock() {
            let new_value = guard.apply(diff);
            let _ = self.update_tx.send(new_value);
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

    /// Merge a remote diff
    pub fn merge_diff<D>(&self, diff: &D)
    where
        T: Diffable<Diff = D>
    {
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.version += 1;
        }

        if let Ok(mut guard) = self.value.lock() {
            let remote_value = guard.apply(diff);
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

    /// Subscribe to updates (typed)
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

    /// Sign the current channel data
    pub fn sign(&self) -> Result<Vec<u8>, FlowError> {
        if let Some(authenticator) = &self.authenticator {
            if let Ok(guard) = self.value.lock() {
                return authenticator.sign(&*guard);
            }
            Err(FlowError::PersistenceError("Failed to access channel data".to_string()))
        } else {
            Err(FlowError::PersistenceError("No authenticator configured".to_string()))
        }
    }

    /// Verify a signature for the current channel data
    pub fn verify(&self, signature: &[u8]) -> bool {
        if let Some(authenticator) = &self.authenticator {
            if let Ok(guard) = self.value.lock() {
                return authenticator.verify(&*guard, signature);
            }
        }
        false
    }

    /// Persist the channel data using the configured storage backend
    pub fn persist(&self) -> Result<(), FlowError> {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                if let Ok(guard) = self.value.lock() {
                    if let Ok(json) = serde_json::to_string(&*guard) {
                        return storage.store(metadata.id, json.as_bytes());
                    }
                }
            }
            Err(FlowError::PersistenceError("Failed to serialize channel data".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }

    /// Load channel data from the configured storage backend
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
                return Err(FlowError::PersistenceError("Failed to deserialize channel data".to_string()));
            }
            Err(FlowError::PersistenceError("Failed to access channel metadata".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }

    /// Check if channel data exists in the storage backend
    pub fn exists(&self) -> bool {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                return storage.exists(metadata.id);
            }
        }
        false
    }

    /// Delete channel data from the storage backend
    pub fn delete(&self) -> Result<(), FlowError> {
        if let Some(storage) = &self.storage {
            if let Ok(metadata) = self.metadata.lock() {
                return storage.delete(metadata.id);
            }
            Err(FlowError::PersistenceError("Failed to access channel metadata".to_string()))
        } else {
            Err(FlowError::PersistenceError("No storage backend configured".to_string()))
        }
    }
}

/// Implement Stream trait for DataChannel to allow it to be used in flows.
impl<T> Stream for DataChannel<T>
where
    T: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + 'static,
{
    fn read(&self) -> Result<Option<Vec<u8>>, FlowError> {
        if let Ok(guard) = self.value.lock() {
            let bytes = serde_json::to_vec(&*guard)
                .map_err(|e| FlowError::SerializationError(e.to_string()))?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    fn write(&self, data: &[u8]) -> Result<(), FlowError> {
        let value: T = serde_json::from_slice(data)
            .map_err(|e| FlowError::SerializationError(e.to_string()))?;
        self.update(value);
        Ok(())
    }

    fn subscribe(&self, callback: Box<dyn Fn(&[u8]) + Send + Sync>) -> Uuid {
        // Wrap the bytes callback to serialize the value
        let typed_callback: Box<dyn Fn(&T) + Send + Sync> = Box::new(move |value: &T| {
            if let Ok(bytes) = serde_json::to_vec(value) {
                callback(&bytes);
            }
        });
        DataChannel::subscribe(self, typed_callback)
    }

    fn unsubscribe(&self, subscription_id: Uuid) {
        DataChannel::unsubscribe(self, subscription_id);
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// === Backward compatibility aliases ===
// These allow existing code to continue working while we migrate.

/// DEPRECATED: Use DataChannel instead.
/// This alias exists for backward compatibility during migration.
#[deprecated(since = "0.2.0", note = "Use DataChannel instead")]
pub type SelfDataFlow<T> = DataChannel<T>;

/// DEPRECATED: Use ChannelMetadata instead.
#[deprecated(since = "0.2.0", note = "Use ChannelMetadata instead")]
pub type FlowMetadata = ChannelMetadata;
