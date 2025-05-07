//! Identity management for Soradyne
//! 
//! This module handles cryptographic identities, key derivation,
//! device management, and authentication.

mod key_management;
mod device;
mod authentication;

use std::collections::HashMap;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use thiserror::Error;

pub use key_management::{KeyPair, KeyType, KeyDerivation};
pub use device::{Device, DeviceType, DeviceCapability};
pub use authentication::{AuthMethod, Credentials};

/// Error types for identity operations
#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Key generation failed: {0}")]
    KeyGenerationError(String),
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Device not found: {0}")]
    DeviceNotFound(Uuid),
    
    #[error("Insufficient authentication: {0}")]
    InsufficientAuth(String),
}

/// Represents a unique identity in the Soradyne network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    /// Unique identifier for this identity
    pub id: Uuid,
    
    /// Human-readable name
    pub name: String,
    
    /// Public key used for verification
    #[serde(with = "key_management::pubkey_serde")]
    pub public_key: PublicKey,
    
    /// Devices associated with this identity
    #[serde(skip)]
    devices: HashMap<Uuid, Device>,
    
    /// Current device being used
    #[serde(skip)]
    current_device: Option<Uuid>,
}

impl Identity {
    /// Create a new identity with a fresh keypair
    pub fn new(name: &str) -> Result<(Self, KeyPair), IdentityError> {
        // Generate a new keypair
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        let id = Uuid::new_v4();
        
        let identity = Self {
            id,
            name: name.to_string(),
            public_key: keypair.public,
            devices: HashMap::new(),
            current_device: None,
        };
        
        // Create key pair from the generated keypair
        let key_pair = KeyPair {
            key_type: KeyType::Ed25519,
            public_key: keypair.public,
            private_key: Some(keypair.secret),
        };
        
        Ok((identity, key_pair))
    }
    
    /// Sign data with the current device's keypair
    pub fn sign(&self, data: &[u8], keypair: &KeyPair) -> Result<Vec<u8>, IdentityError> {
        match keypair.key_type {
            KeyType::Ed25519 => {
                if let Some(private_key) = &keypair.private_key {
                    let keypair = Keypair {
                        public: keypair.public_key,
                        secret: private_key.clone(),
                    };
                    
                    Ok(keypair.sign(data).to_bytes().to_vec())
                } else {
                    Err(IdentityError::AuthenticationFailed)
                }
            }
            // Support for other key types would go here
        }
    }
    
    /// Verify a signature against this identity's public key
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
        if signature.len() != 64 {
            return Err(IdentityError::InvalidSignature);
        }
        
        // Convert to fixed-size array
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(signature);
        
        match self.public_key.verify(
            data, 
            &ed25519_dalek::Signature::from_bytes(&sig_bytes)
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(IdentityError::InvalidSignature),
        }
    }
    
    /// Add a device to this identity
    pub fn add_device(&mut self, device: Device) {
        self.devices.insert(device.id, device);
    }
    
    /// Set the current active device
    pub fn set_current_device(&mut self, device_id: Uuid) -> Result<(), IdentityError> {
        if self.devices.contains_key(&device_id) {
            self.current_device = Some(device_id);
            Ok(())
        } else {
            Err(IdentityError::DeviceNotFound(device_id))
        }
    }
}

/// Manages identities and their relationships
pub struct IdentityManager {
    /// The primary identity for this instance
    primary_identity: Option<Identity>,
    
    /// The keypair for the primary identity
    primary_keypair: Option<KeyPair>,
    
    /// Known trusted identities
    trusted_identities: HashMap<Uuid, Identity>,
}

impl IdentityManager {
    /// Create a new identity manager
    pub fn new() -> Self {
        Self {
            primary_identity: None,
            primary_keypair: None,
            trusted_identities: HashMap::new(),
        }
    }
    
    /// Create a new primary identity
    pub fn create_identity(&mut self, name: &str) -> Result<Uuid, IdentityError> {
        let (identity, keypair) = Identity::new(name)?;
        let id = identity.id;
        
        self.primary_identity = Some(identity);
        self.primary_keypair = Some(keypair);
        
        Ok(id)
    }
    
    /// Add a trusted identity
    pub fn add_trusted_identity(&mut self, identity: Identity) {
        self.trusted_identities.insert(identity.id, identity);
    }
    
    /// Get the primary identity
    pub fn get_primary_identity(&self) -> Option<&Identity> {
        self.primary_identity.as_ref()
    }
    
    /// Sign data with the primary identity
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, IdentityError> {
        match (&self.primary_identity, &self.primary_keypair) {
            (Some(identity), Some(keypair)) => identity.sign(data, keypair),
            _ => Err(IdentityError::AuthenticationFailed),
        }
    }
}

impl Default for IdentityManager {
    fn default() -> Self {
        Self::new()
    }
}
