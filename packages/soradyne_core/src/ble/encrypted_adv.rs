//! Encrypted advertisement payloads
//!
//! Capsule members encrypt their BLE advertisements so only other members
//! of the same capsule can decode them. A cleartext `capsule_hint` prefix
//! lets receivers quickly identify which key to try.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::identity::capsule_keys::CapsuleKeyBundle;

/// Advertisement payload broadcast by a capsule member.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvertisementPayload {
    /// Truncated hash of the capsule ID (for key lookup).
    pub capsule_hint: [u8; 4],
    /// Truncated hash of the advertising device's ID.
    pub piece_hint: [u8; 4],
    /// Sequence number (monotonically increasing per advertiser).
    pub seq: u32,
    /// Hash of the advertiser's current topology view.
    pub topology_hash: u32,
    /// Bitmap of known pieces in the capsule.
    pub known_pieces: Vec<u8>,
}

impl AdvertisementPayload {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.piece_hint);
        buf.extend_from_slice(&self.seq.to_le_bytes());
        buf.extend_from_slice(&self.topology_hash.to_le_bytes());
        buf.push(self.known_pieces.len() as u8);
        buf.extend_from_slice(&self.known_pieces);
        buf
    }

    fn from_bytes(data: &[u8]) -> Option<Self> {
        // piece_hint (4) + seq (4) + topology_hash (4) + known_pieces_len (1) = 13 minimum
        if data.len() < 13 {
            return None;
        }
        let mut piece_hint = [0u8; 4];
        piece_hint.copy_from_slice(&data[0..4]);
        let seq = u32::from_le_bytes(data[4..8].try_into().ok()?);
        let topology_hash = u32::from_le_bytes(data[8..12].try_into().ok()?);
        let kp_len = data[12] as usize;
        if data.len() < 13 + kp_len {
            return None;
        }
        let known_pieces = data[13..13 + kp_len].to_vec();

        Some(Self {
            capsule_hint: [0; 4], // Filled in by caller after decryption.
            piece_hint,
            seq,
            topology_hash,
            known_pieces,
        })
    }
}

/// Compute a deterministic 4-byte hint from a capsule ID.
pub fn capsule_hint_for(capsule_id: &Uuid) -> [u8; 4] {
    let mut hasher = Sha256::new();
    hasher.update(b"soradyne-capsule-hint-v1");
    hasher.update(capsule_id.as_bytes());
    let hash = hasher.finalize();
    let mut hint = [0u8; 4];
    hint.copy_from_slice(&hash[..4]);
    hint
}

/// Compute a deterministic 4-byte hint from a device/piece ID.
pub fn piece_hint_for(device_id: &Uuid) -> [u8; 4] {
    let mut hasher = Sha256::new();
    hasher.update(b"soradyne-piece-hint-v1");
    hasher.update(device_id.as_bytes());
    let hash = hasher.finalize();
    let mut hint = [0u8; 4];
    hint.copy_from_slice(&hash[..4]);
    hint
}

/// Encrypt an advertisement payload for broadcast.
///
/// Returns: `capsule_hint (4 bytes, cleartext) || encrypted_payload`.
/// The capsule_hint is sent in the clear so receivers know which key to try
/// without attempting decryption with every known capsule.
pub fn encrypt_advertisement(
    payload: &AdvertisementPayload,
    capsule_keys: &CapsuleKeyBundle,
) -> Result<Vec<u8>, crate::identity::IdentityError> {
    let plaintext = payload.to_bytes();
    let ciphertext = capsule_keys.encrypt(&plaintext)?;

    let hint = capsule_hint_for(&capsule_keys.capsule_id);
    let mut result = Vec::with_capacity(4 + ciphertext.len());
    result.extend_from_slice(&hint);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Try to decrypt an advertisement using a set of known capsule keys.
///
/// Reads the 4-byte capsule_hint prefix, finds the matching capsule,
/// and attempts decryption. Returns the capsule ID and decoded payload
/// on success, or `None` if no matching capsule is found or decryption fails.
pub fn try_decrypt_advertisement(
    raw: &[u8],
    known_capsules: &[CapsuleKeyBundle],
) -> Option<(Uuid, AdvertisementPayload)> {
    if raw.len() < 4 {
        return None;
    }

    let mut hint = [0u8; 4];
    hint.copy_from_slice(&raw[..4]);
    let encrypted = &raw[4..];

    // Find the capsule whose hint matches.
    let capsule = known_capsules
        .iter()
        .find(|c| capsule_hint_for(&c.capsule_id) == hint)?;

    let plaintext = capsule.decrypt(encrypted).ok()?;
    let mut payload = AdvertisementPayload::from_bytes(&plaintext)?;
    payload.capsule_hint = hint;

    Some((capsule.capsule_id, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let device_id = Uuid::new_v4();

        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&capsule_keys.capsule_id),
            piece_hint: piece_hint_for(&device_id),
            seq: 42,
            topology_hash: 0xDEADBEEF,
            known_pieces: vec![0b11010101],
        };

        let encrypted = encrypt_advertisement(&payload, &capsule_keys).unwrap();
        let (capsule_id, decrypted) =
            try_decrypt_advertisement(&encrypted, &[capsule_keys.clone()]).unwrap();

        assert_eq!(capsule_id, capsule_keys.capsule_id);
        assert_eq!(decrypted.piece_hint, payload.piece_hint);
        assert_eq!(decrypted.seq, 42);
        assert_eq!(decrypted.topology_hash, 0xDEADBEEF);
        assert_eq!(decrypted.known_pieces, vec![0b11010101]);
    }

    #[test]
    fn test_wrong_capsule_key_fails() {
        let keys_a = CapsuleKeyBundle::generate(Uuid::new_v4());
        let keys_b = CapsuleKeyBundle::generate(Uuid::new_v4());

        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&keys_a.capsule_id),
            piece_hint: [0; 4],
            seq: 1,
            topology_hash: 0,
            known_pieces: vec![],
        };

        let encrypted = encrypt_advertisement(&payload, &keys_a).unwrap();

        // Only keys_b is known — hint won't match, so None.
        let result = try_decrypt_advertisement(&encrypted, &[keys_b]);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_decrypt_selects_correct_capsule() {
        let keys_1 = CapsuleKeyBundle::generate(Uuid::new_v4());
        let keys_2 = CapsuleKeyBundle::generate(Uuid::new_v4());
        let keys_3 = CapsuleKeyBundle::generate(Uuid::new_v4());

        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&keys_2.capsule_id),
            piece_hint: [0xAB; 4],
            seq: 7,
            topology_hash: 123,
            known_pieces: vec![0xFF],
        };

        let encrypted = encrypt_advertisement(&payload, &keys_2).unwrap();

        // All three capsules are known; should find keys_2.
        let (capsule_id, decrypted) = try_decrypt_advertisement(
            &encrypted,
            &[keys_1, keys_2.clone(), keys_3],
        )
        .unwrap();

        assert_eq!(capsule_id, keys_2.capsule_id);
        assert_eq!(decrypted.seq, 7);
    }

    #[test]
    fn test_capsule_hint_deterministic() {
        let id = Uuid::new_v4();
        let hint1 = capsule_hint_for(&id);
        let hint2 = capsule_hint_for(&id);
        assert_eq!(hint1, hint2);

        // Different UUIDs produce different hints (with overwhelming probability).
        let other_id = Uuid::new_v4();
        let hint3 = capsule_hint_for(&other_id);
        assert_ne!(hint1, hint3);
    }
}
