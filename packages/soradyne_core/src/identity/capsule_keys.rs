//! Capsule shared key material
//!
//! When a capsule is created, shared symmetric key material is established.
//! This material is distributed to new pieces during pairing, encrypted with
//! the pairwise ECDH-derived key.
//!
//! Used for:
//! - Encrypting BLE advertisements (so only capsule members can decode them)
//! - Deriving per-session encryption keys for BLE connections

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use super::IdentityError;

/// Shared key material for a capsule.
///
/// All pieces in a capsule hold a copy of this bundle. It is the "capsule secret"
/// that enables a piece to understand advertisements and participate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleKeyBundle {
    /// Capsule UUID
    pub capsule_id: Uuid,
    /// Symmetric key for advertisement encryption (AES-256)
    /// Derived during capsule creation, distributed during pairing
    pub advertisement_key: [u8; 32],
    /// Identity Resolving Key (IRK) for BLE address rotation
    pub irk: [u8; 16],
    /// Epoch counter for key rotation (future: proactive refresh)
    pub epoch: u64,
}

impl CapsuleKeyBundle {
    /// Generate fresh keys for a new capsule.
    pub fn generate(capsule_id: Uuid) -> Self {
        let mut rng = rand::thread_rng();

        let mut advertisement_key = [0u8; 32];
        rng.fill_bytes(&mut advertisement_key);

        let mut irk = [0u8; 16];
        rng.fill_bytes(&mut irk);

        Self {
            capsule_id,
            advertisement_key,
            irk,
            epoch: 0,
        }
    }

    /// Derive an advertisement encryption key for a specific epoch.
    ///
    /// Uses HKDF-SHA256 to derive a per-epoch key from the base advertisement key.
    /// This allows key rotation without distributing entirely new key material.
    pub fn adv_key_for_epoch(&self, epoch: u64) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(None, &self.advertisement_key);
        let info = format!("soradyne-adv-epoch-{}", epoch);
        let mut okm = [0u8; 32];
        hk.expand(info.as_bytes(), &mut okm)
            .expect("32 bytes is a valid HKDF-SHA256 output length");
        okm
    }

    /// Serialize for transfer during pairing.
    ///
    /// The result should be encrypted with the pairwise ECDH-derived key before
    /// sending over BLE. This function handles the inner serialization; the caller
    /// handles the encryption envelope.
    pub fn to_bytes(&self) -> Result<Vec<u8>, IdentityError> {
        serde_json::to_vec(self).map_err(|e| IdentityError::SerializationError(e.to_string()))
    }

    /// Deserialize from pairing transfer.
    pub fn from_bytes(data: &[u8]) -> Result<Self, IdentityError> {
        serde_json::from_slice(data)
            .map_err(|e| IdentityError::DeserializationError(e.to_string()))
    }

    /// Encrypt data using this capsule's advertisement key at the current epoch.
    ///
    /// Uses AES-256-GCM with a random nonce. Returns nonce || ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, IdentityError> {
        let key = self.adv_key_for_epoch(self.epoch);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        // nonce (12 bytes) || ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data using this capsule's advertisement key at the current epoch.
    ///
    /// Expects nonce (12 bytes) || ciphertext as input.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, IdentityError> {
        if data.len() < 12 {
            return Err(IdentityError::CryptoError(
                "ciphertext too short (missing nonce)".to_string(),
            ));
        }

        let key = self.adv_key_for_epoch(self.epoch);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        let nonce = Nonce::from_slice(&data[..12]);
        let plaintext = cipher
            .decrypt(nonce, &data[12..])
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        Ok(plaintext)
    }

    /// Encrypt data for transfer during pairing, using a pairwise shared secret
    /// (from ECDH) rather than the capsule advertisement key.
    ///
    /// Uses HKDF to derive an encryption key from the shared secret, then AES-256-GCM.
    pub fn encrypt_for_transfer(data: &[u8], shared_secret: &[u8; 32]) -> Result<Vec<u8>, IdentityError> {
        let hk = Hkdf::<Sha256>::new(None, shared_secret);
        let mut transfer_key = [0u8; 32];
        hk.expand(b"soradyne-pairing-transfer-v1", &mut transfer_key)
            .expect("32 bytes is a valid HKDF-SHA256 output length");

        let cipher = Aes256Gcm::new_from_slice(&transfer_key)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data received during pairing, using a pairwise shared secret.
    pub fn decrypt_for_transfer(data: &[u8], shared_secret: &[u8; 32]) -> Result<Vec<u8>, IdentityError> {
        if data.len() < 12 {
            return Err(IdentityError::CryptoError(
                "ciphertext too short (missing nonce)".to_string(),
            ));
        }

        let hk = Hkdf::<Sha256>::new(None, shared_secret);
        let mut transfer_key = [0u8; 32];
        hk.expand(b"soradyne-pairing-transfer-v1", &mut transfer_key)
            .expect("32 bytes is a valid HKDF-SHA256 output length");

        let cipher = Aes256Gcm::new_from_slice(&transfer_key)
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        let nonce = Nonce::from_slice(&data[..12]);
        let plaintext = cipher
            .decrypt(nonce, &data[12..])
            .map_err(|e| IdentityError::CryptoError(e.to_string()))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_capsule_keys() {
        let capsule_id = Uuid::new_v4();
        let keys = CapsuleKeyBundle::generate(capsule_id);

        assert_eq!(keys.capsule_id, capsule_id);
        assert_eq!(keys.epoch, 0);
        assert_ne!(keys.advertisement_key, [0u8; 32]);
        assert_ne!(keys.irk, [0u8; 16]);
    }

    #[test]
    fn test_epoch_key_derivation() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());

        let k0 = keys.adv_key_for_epoch(0);
        let k1 = keys.adv_key_for_epoch(1);
        let k0_again = keys.adv_key_for_epoch(0);

        // Different epochs produce different keys
        assert_ne!(k0, k1);
        // Same epoch is deterministic
        assert_eq!(k0, k0_again);
    }

    #[test]
    fn test_serialization_round_trip() {
        let original = CapsuleKeyBundle::generate(Uuid::new_v4());
        let bytes = original.to_bytes().unwrap();
        let restored = CapsuleKeyBundle::from_bytes(&bytes).unwrap();

        assert_eq!(original.capsule_id, restored.capsule_id);
        assert_eq!(original.advertisement_key, restored.advertisement_key);
        assert_eq!(original.irk, restored.irk);
        assert_eq!(original.epoch, restored.epoch);
    }

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let plaintext = b"capsule member advertisement payload";

        let ciphertext = keys.encrypt(plaintext).unwrap();
        let decrypted = keys.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let plaintext = b"same message twice";

        let ct1 = keys.encrypt(plaintext).unwrap();
        let ct2 = keys.encrypt(plaintext).unwrap();

        // Random nonce means different ciphertext each time
        assert_ne!(ct1, ct2);

        // Both decrypt to the same plaintext
        assert_eq!(keys.decrypt(&ct1).unwrap(), plaintext);
        assert_eq!(keys.decrypt(&ct2).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_key_fails_decrypt() {
        let keys_a = CapsuleKeyBundle::generate(Uuid::new_v4());
        let keys_b = CapsuleKeyBundle::generate(Uuid::new_v4());
        let plaintext = b"secret data";

        let ciphertext = keys_a.encrypt(plaintext).unwrap();

        // Wrong capsule key should fail
        assert!(keys_b.decrypt(&ciphertext).is_err());
    }

    #[test]
    fn test_pairing_transfer_encrypt_decrypt() {
        let shared_secret = [42u8; 32]; // simulated ECDH output
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let payload = keys.to_bytes().unwrap();

        let encrypted = CapsuleKeyBundle::encrypt_for_transfer(&payload, &shared_secret).unwrap();
        let decrypted = CapsuleKeyBundle::decrypt_for_transfer(&encrypted, &shared_secret).unwrap();

        let restored = CapsuleKeyBundle::from_bytes(&decrypted).unwrap();
        assert_eq!(keys.capsule_id, restored.capsule_id);
        assert_eq!(keys.advertisement_key, restored.advertisement_key);
    }

    #[test]
    fn test_pairing_transfer_wrong_secret_fails() {
        let secret_a = [1u8; 32];
        let secret_b = [2u8; 32];
        let data = b"capsule key bundle bytes";

        let encrypted = CapsuleKeyBundle::encrypt_for_transfer(data, &secret_a).unwrap();
        assert!(CapsuleKeyBundle::decrypt_for_transfer(&encrypted, &secret_b).is_err());
    }
}
