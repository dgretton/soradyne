//! Stream trait - the basic I/O abstraction for flows
//!
//! A Stream is an abstract interface for reading and writing data.
//! Streams are named and registered with flows. The flow's schema
//! determines what streams must exist; implementations are injected.
//!
//! "Drip" and "jet" are descriptive names for stream types in flow
//! definitions, not separate traits. A drip is a stream that provides
//! eventually consistent data; a jet is a fast, possibly lossy stream.

use uuid::Uuid;
use serde::{Serialize, Deserialize};
use super::FlowError;

/// The basic Stream trait for reading and writing data.
///
/// Implementations include:
/// - DataChannel<T> (the former SelfDataFlow) for in-memory pub/sub
/// - Network streams over TCP/UDP
/// - Convergent document backed streams
pub trait Stream: Send + Sync {
    /// Read the current value as bytes.
    /// Returns None if no data is available yet.
    fn read(&self) -> Result<Option<Vec<u8>>, FlowError>;

    /// Write data to the stream.
    fn write(&self, data: &[u8]) -> Result<(), FlowError>;

    /// Subscribe to updates. Returns a subscription ID.
    fn subscribe(&self, callback: Box<dyn Fn(&[u8]) + Send + Sync>) -> Uuid;

    /// Unsubscribe from updates.
    fn unsubscribe(&self, subscription_id: Uuid);

    /// Stream name (as defined in the flow schema).
    fn name(&self) -> &str;
}

/// A typed wrapper around Stream for convenience.
/// Handles serialization/deserialization automatically.
pub struct TypedStream<T> {
    inner: Box<dyn Stream>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedStream<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(inner: Box<dyn Stream>) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn read(&self) -> Result<Option<T>, FlowError> {
        match self.inner.read()? {
            Some(bytes) => {
                let value: T = serde_json::from_slice(&bytes)
                    .map_err(|e| FlowError::SerializationError(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    pub fn write(&self, value: &T) -> Result<(), FlowError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| FlowError::SerializationError(e.to_string()))?;
        self.inner.write(&bytes)
    }

    pub fn subscribe(&self, callback: Box<dyn Fn(&T) + Send + Sync>) -> Uuid
    where
        T: 'static,
    {
        self.inner.subscribe(Box::new(move |bytes| {
            if let Ok(value) = serde_json::from_slice::<T>(bytes) {
                callback(&value);
            }
        }))
    }

    pub fn unsubscribe(&self, subscription_id: Uuid) {
        self.inner.unsubscribe(subscription_id);
    }
}

/// Metadata about a stream as declared in a flow schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamSpec {
    /// Name of the stream (e.g., "giantt_state", "alice_contributions")
    pub name: String,

    /// Human-readable description
    pub description: Option<String>,

    /// Stream category hint (for documentation, not enforced)
    /// e.g., "drip" for convergent, "jet" for fast/lossy
    pub category: Option<String>,

    /// Cardinality: "singleton", "per_party", "unbounded"
    pub cardinality: StreamCardinality,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamCardinality {
    /// Exactly one stream with this name
    Singleton,
    /// One stream per party (device/user)
    PerParty,
    /// Unbounded number (e.g., threads, topics)
    Unbounded,
}

impl StreamSpec {
    pub fn singleton(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            category: None,
            cardinality: StreamCardinality::Singleton,
        }
    }

    pub fn drip(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            category: Some("drip".to_string()),
            cardinality: StreamCardinality::Singleton,
        }
    }

    pub fn jet(name: impl Into<String>, cardinality: StreamCardinality) -> Self {
        Self {
            name: name.into(),
            description: None,
            category: Some("jet".to_string()),
            cardinality,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}
