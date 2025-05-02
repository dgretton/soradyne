//! Node.js bindings for Soradyne identity management
//!
//! This module provides bindings for the Soradyne identity management module
//! to be used from Node.js via TypeScript.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use uuid::Uuid;

use crate::core::identity::{Identity, KeyPair, IdentityManager, DeviceType, Device};

/// JavaScript representation of an identity
#[napi(object)]
pub struct JsIdentity {
    pub id: String,
    pub name: String,
    pub public_key: String,
}

/// JavaScript representation of a device
#[napi(object)]
pub struct JsDevice {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub capabilities: Vec<String>,
    pub last_seen: String,
}

/// JavaScript representation of a key pair
#[napi(object)]
pub struct JsKeyPair {
    pub key_type: String,
    pub public_key: String,
    pub has_private_key: bool,
}

/// Wrapper for the identity manager
#[napi]
pub struct JsIdentityManager {
    inner: IdentityManager,
}

#[napi]
impl JsIdentityManager {
    /// Create a new identity manager
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: IdentityManager::new(),
        }
    }
    
    /// Create a new identity
    #[napi]
    pub fn create_identity(&mut self, name: String) -> Result<String> {
        match self.inner.create_identity(&name) {
            Ok(id) => Ok(id.to_string()),
            Err(e) => Err(Error::new(Status::GenericFailure, e.to_string())),
        }
    }
    
    /// Get the primary identity
    #[napi]
    pub fn get_primary_identity(&self) -> Option<JsIdentity> {
        self.inner.get_primary_identity().map(|identity| {
            JsIdentity {
                id: identity.id.to_string(),
                name: identity.name.clone(),
                public_key: base64::encode(identity.public_key.as_bytes()),
            }
        })
    }
    
    /// Sign data with the primary identity
    #[napi]
    pub fn sign(&self, data: Buffer) -> Result<Buffer> {
        match self.inner.sign(&data) {
            Ok(signature) => Ok(Buffer::from(signature)),
            Err(e) => Err(Error::new(Status::GenericFailure, e.to_string())),
        }
    }
}

/// Convert a Rust identity to a JavaScript identity
fn to_js_identity(identity: &Identity) -> JsIdentity {
    JsIdentity {
        id: identity.id.to_string(),
        name: identity.name.clone(),
        public_key: base64::encode(identity.public_key.as_bytes()),
    }
}

/// Convert a Rust device to a JavaScript device
fn to_js_device(device: &Device) -> JsDevice {
    JsDevice {
        id: device.id.to_string(),
        name: device.name.clone(),
        device_type: format!("{:?}", device.device_type),
        capabilities: device.capabilities.iter()
            .map(|c| format!("{:?}", c))
            .collect(),
        last_seen: device.last_seen.to_rfc3339(),
    }
}

/// Convert a Rust keypair to a JavaScript keypair
fn to_js_keypair(keypair: &KeyPair) -> JsKeyPair {
    JsKeyPair {
        key_type: format!("{:?}", keypair.key_type),
        public_key: base64::encode(keypair.public_key.as_bytes()),
        has_private_key: keypair.private_key.is_some(),
    }
}
