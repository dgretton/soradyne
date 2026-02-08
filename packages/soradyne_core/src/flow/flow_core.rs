//! Core Flow types - the main abstraction for self-data flows
//!
//! A Flow is a persistent, UUID'd, typed bundle of streams with policies.
//! Flows are loaded by UUID; the UUID maps to a type and configuration
//! which together determine what streams exist and how they behave.
//!
//! # Bootstrap sequence
//!
//! ```text
//! UUID
//!   ↓ (lookup in storage - currently a placeholder HashMap)
//! type_name + config
//!   ↓ (type_name selects constructor from registry)
//! constructor(config)
//!   ↓
//! Flow instance (right type, right streams, ready to use)
//! ```
//!
//! # Future evolution
//!
//! - Config storage: on-device → broadcast by initiator → DHT discovery
//! - Type registry: hardcoded → plugin system → dynamic loading

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use super::stream::{Stream, StreamSpec};
use super::FlowError;

/// Configuration for a flow instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Unique identifier for this flow instance
    pub id: Uuid,

    /// Type name (maps to a constructor in the registry)
    pub type_name: String,

    /// Flow-specific parameters (interpreted by the constructor)
    pub params: serde_json::Value,
}

/// Schema defining what streams a flow type must have.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowSchema {
    /// Human-readable name for this flow type
    pub name: String,

    /// Stream specifications
    pub streams: Vec<StreamSpec>,

    // Future: policies for error handling, delegation, etc.
}

impl FlowSchema {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            streams: Vec::new(),
        }
    }

    pub fn with_stream(mut self, spec: StreamSpec) -> Self {
        self.streams.push(spec);
        self
    }
}

/// The Flow trait - implemented by all flow types.
///
/// A Flow is the authority for its streams. Once you have a Flow,
/// you access data through it, not by bypassing it.
pub trait Flow: Send + Sync {
    /// Get the flow's unique identifier.
    fn id(&self) -> Uuid;

    /// Get the flow's type name.
    fn type_name(&self) -> &str;

    /// Get the flow's schema.
    fn schema(&self) -> &FlowSchema;

    /// Get a stream by name.
    /// Returns None if the stream doesn't exist or isn't implemented yet.
    fn stream(&self, name: &str) -> Option<&dyn Stream>;

    /// Get a mutable stream by name.
    fn stream_mut(&mut self, name: &str) -> Option<&mut Box<dyn Stream>>;

    /// Register/inject a stream implementation.
    /// The stream's name must match a StreamSpec in the schema.
    fn register_stream(&mut self, stream: Box<dyn Stream>) -> Result<(), FlowError>;

    /// List all registered stream names.
    fn stream_names(&self) -> Vec<String>;
}

/// Constructor function type for flow types.
pub type FlowConstructor = fn(FlowConfig) -> Result<Box<dyn Flow>, FlowError>;

/// Registry mapping type names to constructors.
///
/// Currently a simple HashMap. In the future, this could be:
/// - Populated from plugins
/// - Loaded dynamically
/// - Extended at runtime
pub struct FlowRegistry {
    constructors: HashMap<String, FlowConstructor>,
}

impl FlowRegistry {
    pub fn new() -> Self {
        Self {
            constructors: HashMap::new(),
        }
    }

    /// Register a flow type constructor.
    pub fn register(&mut self, type_name: impl Into<String>, constructor: FlowConstructor) {
        self.constructors.insert(type_name.into(), constructor);
    }

    /// Get a constructor by type name.
    pub fn get(&self, type_name: &str) -> Option<&FlowConstructor> {
        self.constructors.get(type_name)
    }

    /// Load a flow by UUID.
    ///
    /// This is the main entry point for the bootstrap sequence:
    /// UUID → config (from storage) → type → constructor → Flow
    pub fn load(&self, uuid: Uuid, storage: &dyn FlowConfigStorage) -> Result<Box<dyn Flow>, FlowError> {
        let config = storage.get_config(uuid)?;
        let constructor = self.constructors.get(&config.type_name)
            .ok_or_else(|| FlowError::UnknownFlowType(config.type_name.clone()))?;
        constructor(config)
    }
}

impl Default for FlowRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage for flow configurations.
///
/// Currently this is a placeholder trait. Implementations could be:
/// - In-memory HashMap (for testing, current default)
/// - On-device file storage
/// - Broadcast/received from initiator
/// - DHT lookup
pub trait FlowConfigStorage: Send + Sync {
    fn get_config(&self, id: Uuid) -> Result<FlowConfig, FlowError>;
    fn store_config(&self, config: &FlowConfig) -> Result<(), FlowError>;
    fn list_configs(&self) -> Result<Vec<Uuid>, FlowError>;
}

/// Simple in-memory config storage for development/testing.
///
/// PLACEHOLDER: In production, this would be replaced by on-device
/// storage that persists across restarts and can be populated by
/// receiving configs from other parties.
pub struct InMemoryConfigStorage {
    configs: RwLock<HashMap<Uuid, FlowConfig>>,
}

impl InMemoryConfigStorage {
    pub fn new() -> Self {
        Self {
            configs: RwLock::new(HashMap::new()),
        }
    }

    /// Create with pre-loaded configs (for bootstrapping).
    pub fn with_configs(configs: Vec<FlowConfig>) -> Self {
        let storage = Self::new();
        {
            let mut map = storage.configs.write().unwrap();
            for config in configs {
                map.insert(config.id, config);
            }
        }
        storage
    }
}

impl Default for InMemoryConfigStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowConfigStorage for InMemoryConfigStorage {
    fn get_config(&self, id: Uuid) -> Result<FlowConfig, FlowError> {
        self.configs
            .read()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| FlowError::FlowNotFound(id))
    }

    fn store_config(&self, config: &FlowConfig) -> Result<(), FlowError> {
        self.configs
            .write()
            .unwrap()
            .insert(config.id, config.clone());
        Ok(())
    }

    fn list_configs(&self) -> Result<Vec<Uuid>, FlowError> {
        Ok(self.configs.read().unwrap().keys().cloned().collect())
    }
}

/// A basic Flow implementation that can be used directly or as a base.
pub struct BasicFlow {
    id: Uuid,
    type_name: String,
    schema: FlowSchema,
    streams: HashMap<String, Box<dyn Stream>>,
}

impl BasicFlow {
    pub fn new(config: FlowConfig, schema: FlowSchema) -> Self {
        Self {
            id: config.id,
            type_name: config.type_name,
            schema,
            streams: HashMap::new(),
        }
    }
}

impl Flow for BasicFlow {
    fn id(&self) -> Uuid {
        self.id
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn schema(&self) -> &FlowSchema {
        &self.schema
    }

    fn stream(&self, name: &str) -> Option<&dyn Stream> {
        self.streams.get(name).map(|s| s.as_ref())
    }

    fn stream_mut(&mut self, name: &str) -> Option<&mut Box<dyn Stream>> {
        self.streams.get_mut(name)
    }

    fn register_stream(&mut self, stream: Box<dyn Stream>) -> Result<(), FlowError> {
        let name = stream.name().to_string();

        // Verify stream name matches schema
        if !self.schema.streams.iter().any(|s| s.name == name) {
            return Err(FlowError::InvalidStreamName(name));
        }

        self.streams.insert(name, stream);
        Ok(())
    }

    fn stream_names(&self) -> Vec<String> {
        self.streams.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_config_storage() {
        let storage = InMemoryConfigStorage::new();
        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "test".to_string(),
            params: serde_json::json!({}),
        };

        storage.store_config(&config).unwrap();
        let loaded = storage.get_config(config.id).unwrap();
        assert_eq!(loaded.type_name, "test");
    }

    #[test]
    fn test_flow_schema_builder() {
        let schema = FlowSchema::new("TestFlow")
            .with_stream(StreamSpec::drip("convergent_state"))
            .with_stream(StreamSpec::jet("live_updates", super::super::stream::StreamCardinality::PerParty));

        assert_eq!(schema.streams.len(), 2);
        assert_eq!(schema.streams[0].name, "convergent_state");
        assert_eq!(schema.streams[0].category, Some("drip".to_string()));
    }
}
