//! Device management for Soradyne
//!
//! This module handles device registration, capabilities, and authentication.

use uuid::Uuid;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

/// Types of devices that can participate in the Soradyne network
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DeviceType {
    /// A general-purpose computer (laptop, desktop)
    Computer,
    
    /// A mobile device (phone, tablet)
    Mobile,
    
    /// A smart wearable (ring, watch, etc.)
    Wearable,
    
    /// An embedded device (IoT, sensor, etc.)
    Embedded,
    
    /// A custom device type with a name
    Custom(String),
}

/// Capabilities that a device can have in the Soradyne network
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeviceCapability {
    /// Can store data for the identity
    Storage,
    
    /// Can perform cryptographic operations
    Cryptography,
    
    /// Can connect to other devices
    Networking,
    
    /// Can authenticate the user
    Authentication,
    
    /// Can perform key derivation
    KeyDerivation,
    
    /// Can dissolve data across multiple devices
    DataDissolution,
    
    /// Can crystallize data from multiple devices
    DataCrystallization,
}

/// Represents a device in the Soradyne network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    /// Unique identifier for this device
    pub id: Uuid,
    
    /// Human-readable name
    pub name: String,
    
    /// Type of device
    pub device_type: DeviceType,
    
    /// Capabilities of this device
    pub capabilities: HashSet<DeviceCapability>,
    
    /// When this device was last seen
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

impl Device {
    /// Create a new device
    pub fn new(name: &str, device_type: DeviceType) -> Self {
        let mut capabilities = HashSet::new();
        
        // Default capabilities based on device type
        match device_type {
            DeviceType::Computer => {
                capabilities.insert(DeviceCapability::Storage);
                capabilities.insert(DeviceCapability::Cryptography);
                capabilities.insert(DeviceCapability::Networking);
                capabilities.insert(DeviceCapability::Authentication);
                capabilities.insert(DeviceCapability::KeyDerivation);
                capabilities.insert(DeviceCapability::DataDissolution);
                capabilities.insert(DeviceCapability::DataCrystallization);
            }
            DeviceType::Mobile => {
                capabilities.insert(DeviceCapability::Storage);
                capabilities.insert(DeviceCapability::Cryptography);
                capabilities.insert(DeviceCapability::Networking);
                capabilities.insert(DeviceCapability::Authentication);
                capabilities.insert(DeviceCapability::KeyDerivation);
            }
            DeviceType::Wearable => {
                capabilities.insert(DeviceCapability::Cryptography);
                capabilities.insert(DeviceCapability::Networking);
                capabilities.insert(DeviceCapability::Authentication);
            }
            DeviceType::Embedded | DeviceType::Custom(_) => {
                // Minimal capabilities by default, can be added manually
                capabilities.insert(DeviceCapability::Networking);
            }
        }
        
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            device_type,
            capabilities,
            last_seen: chrono::Utc::now(),
        }
    }
    
    /// Check if this device has a specific capability
    pub fn has_capability(&self, capability: DeviceCapability) -> bool {
        self.capabilities.contains(&capability)
    }
    
    /// Add a capability to this device
    pub fn add_capability(&mut self, capability: DeviceCapability) {
        self.capabilities.insert(capability);
    }
    
    /// Remove a capability from this device
    pub fn remove_capability(&mut self, capability: &DeviceCapability) {
        self.capabilities.remove(capability);
    }
    
    /// Update the last seen timestamp to now
    pub fn update_last_seen(&mut self) {
        self.last_seen = chrono::Utc::now();
    }
}
