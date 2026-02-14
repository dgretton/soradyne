//! Per-device cryptographic identity
//!
//! Each piece has:
//! - An Ed25519 signing keypair (authentication, signing operations)
//! - An X25519 key-agreement keypair (ECDH for shared secrets during pairing)
//! - A stable UUID (persisted alongside the keys)
//!
//! Together these form the cryptographic root of a piece's identity.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::Zeroize;

use super::IdentityError;

/// Serializable form of a DeviceIdentity (for persistence).
/// Secret keys are stored as raw bytes. Zeroized on drop.
#[derive(Serialize, Deserialize)]
struct DeviceIdentityStore {
    device_id: Uuid,
    signing_key_bytes: [u8; 32],
    dh_key_bytes: [u8; 32],
}

impl Drop for DeviceIdentityStore {
    fn drop(&mut self) {
        self.signing_key_bytes.zeroize();
        self.dh_key_bytes.zeroize();
    }
}

/// A device's cryptographic identity.
///
/// This is the cryptographic root of a "piece" in the Rim protocol.
/// It is distinct from the storage-device fingerprinting in `storage/device_identity.rs`,
/// which identifies physical storage media (SD cards). This identifies the device itself.
pub struct DeviceIdentity {
    /// Stable UUID for this device (persisted)
    device_id: Uuid,
    /// Ed25519 signing keypair
    signing_key: SigningKey,
    /// X25519 static secret for ECDH
    dh_secret: X25519Secret,
}

impl DeviceIdentity {
    /// Generate a new identity (first boot).
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let dh_secret = X25519Secret::random_from_rng(OsRng);

        Self {
            device_id: Uuid::new_v4(),
            signing_key,
            dh_secret,
        }
    }

    /// Get this device's UUID.
    pub fn device_id(&self) -> Uuid {
        self.device_id
    }

    /// Get the device ID as a string (for use as a `DeviceId` in the convergent module).
    pub fn device_id_string(&self) -> String {
        self.device_id.to_string()
    }

    /// Public Ed25519 verifying key (shared during pairing).
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Public X25519 key (shared during pairing for ECDH).
    pub fn dh_public(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.dh_secret)
    }

    /// Perform X25519 ECDH with a peer's public key.
    /// Returns a 32-byte shared secret.
    pub fn dh_agree(&self, peer_public: &X25519PublicKey) -> [u8; 32] {
        self.dh_secret.diffie_hellman(peer_public).to_bytes()
    }

    /// Sign arbitrary data with the Ed25519 signing key.
    pub fn sign(&self, data: &[u8]) -> ed25519_dalek::Signature {
        self.signing_key.sign(data)
    }

    /// Verify a signature against this device's public verifying key.
    pub fn verify(&self, data: &[u8], signature: &ed25519_dalek::Signature) -> bool {
        self.verifying_key().verify(data, signature).is_ok()
    }

    /// Verify a signature against an arbitrary verifying key.
    pub fn verify_with_key(
        data: &[u8],
        signature: &ed25519_dalek::Signature,
        key: &VerifyingKey,
    ) -> bool {
        key.verify(data, signature).is_ok()
    }

    /// Ed25519 verifying key as bytes (for serialization in PieceRecord, pairing, etc.).
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key().to_bytes()
    }

    /// X25519 public key as bytes (for serialization in PieceRecord, pairing, etc.).
    pub fn dh_public_bytes(&self) -> [u8; 32] {
        self.dh_public().to_bytes()
    }

    /// Load from a JSON keystore file.
    pub fn load(path: &Path) -> Result<Self, IdentityError> {
        let data = std::fs::read(path).map_err(|e| IdentityError::IoError(e.to_string()))?;
        let store: DeviceIdentityStore =
            serde_json::from_slice(&data).map_err(|e| IdentityError::DeserializationError(e.to_string()))?;

        let signing_key = SigningKey::from_bytes(&store.signing_key_bytes);
        let dh_secret = X25519Secret::from(store.dh_key_bytes);

        Ok(Self {
            device_id: store.device_id,
            signing_key,
            dh_secret,
        })
    }

    /// Persist to a JSON keystore file.
    ///
    /// Future: encrypt with platform keychain (macOS Keychain, Android Keystore).
    pub fn save(&self, path: &Path) -> Result<(), IdentityError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| IdentityError::IoError(e.to_string()))?;
        }

        let store = DeviceIdentityStore {
            device_id: self.device_id,
            signing_key_bytes: self.signing_key.to_bytes(),
            dh_key_bytes: self.dh_secret.to_bytes(),
        };

        let json = serde_json::to_string_pretty(&store)
            .map_err(|e| IdentityError::SerializationError(e.to_string()))?;

        std::fs::write(path, json).map_err(|e| IdentityError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load from path if it exists, otherwise generate and save.
    pub fn load_or_generate(path: &Path) -> Result<Self, IdentityError> {
        if path.exists() {
            Self::load(path)
        } else {
            let identity = Self::generate();
            identity.save(path)?;
            Ok(identity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let id = DeviceIdentity::generate();
        // UUID should be valid v4
        assert_eq!(id.device_id().get_version(), Some(uuid::Version::Random));
        // Keys should be non-zero
        assert_ne!(id.verifying_key_bytes(), [0u8; 32]);
        assert_ne!(id.dh_public_bytes(), [0u8; 32]);
    }

    #[test]
    fn test_sign_and_verify() {
        let id = DeviceIdentity::generate();
        let message = b"hello, rim protocol";

        let sig = id.sign(message);
        assert!(id.verify(message, &sig));

        // Wrong message should fail
        assert!(!id.verify(b"wrong message", &sig));
    }

    #[test]
    fn test_verify_with_external_key() {
        let id = DeviceIdentity::generate();
        let message = b"test data";
        let sig = id.sign(message);
        let vk = id.verifying_key();

        assert!(DeviceIdentity::verify_with_key(message, &sig, &vk));
        assert!(!DeviceIdentity::verify_with_key(b"tampered", &sig, &vk));
    }

    #[test]
    fn test_ecdh_agreement() {
        let alice = DeviceIdentity::generate();
        let bob = DeviceIdentity::generate();

        let shared_ab = alice.dh_agree(&bob.dh_public());
        let shared_ba = bob.dh_agree(&alice.dh_public());

        // Both sides should derive the same shared secret
        assert_eq!(shared_ab, shared_ba);

        // Shared secret should not be all zeros
        assert_ne!(shared_ab, [0u8; 32]);
    }

    #[test]
    fn test_ecdh_different_peers() {
        let alice = DeviceIdentity::generate();
        let bob = DeviceIdentity::generate();
        let charlie = DeviceIdentity::generate();

        let ab = alice.dh_agree(&bob.dh_public());
        let ac = alice.dh_agree(&charlie.dh_public());

        // Different peers should produce different shared secrets
        assert_ne!(ab, ac);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_identity.json");

        let original = DeviceIdentity::generate();
        original.save(&path).unwrap();

        let loaded = DeviceIdentity::load(&path).unwrap();

        assert_eq!(original.device_id(), loaded.device_id());
        assert_eq!(original.verifying_key_bytes(), loaded.verifying_key_bytes());
        assert_eq!(original.dh_public_bytes(), loaded.dh_public_bytes());

        // Verify signing still works after round-trip
        let msg = b"persistence test";
        let sig = loaded.sign(msg);
        assert!(original.verify(msg, &sig));
    }

    #[test]
    fn test_load_or_generate_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_identity.json");

        assert!(!path.exists());
        let id = DeviceIdentity::load_or_generate(&path).unwrap();
        assert!(path.exists());

        // Loading again should return the same identity
        let id2 = DeviceIdentity::load_or_generate(&path).unwrap();
        assert_eq!(id.device_id(), id2.device_id());
    }
}
