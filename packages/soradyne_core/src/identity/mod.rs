//! Cryptographic identity for devices and capsules
//!
//! This module provides the cryptographic foundation for the Rim protocol:
//! - `DeviceIdentity`: Per-device Ed25519 signing + X25519 key-agreement keypairs
//! - `CapsuleKeyBundle`: Shared symmetric key material for capsule membership
//! - `DeviceAuthenticator`: `FlowAuthenticator` implementation backed by device identity

pub mod auth;
pub mod capsule_keys;
pub mod keys;

pub use auth::DeviceAuthenticator;
pub use capsule_keys::CapsuleKeyBundle;
pub use keys::DeviceIdentity;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Invalid key material: {0}")]
    InvalidKeyMaterial(String),
}
