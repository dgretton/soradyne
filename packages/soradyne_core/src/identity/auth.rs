//! Flow authentication backed by device identity
//!
//! Implements `FlowAuthenticator<T>` using Ed25519 signatures from `DeviceIdentity`.
//! Data is serialized to JSON before signing, so this works with any `T: Serialize`.

use std::sync::Arc;

use serde::Serialize;

use crate::flow::FlowError;
use crate::flow::traits::FlowAuthenticator;

use super::keys::DeviceIdentity;

/// A flow authenticator backed by a device's Ed25519 signing key.
///
/// Signs flow data by serializing it to JSON and then Ed25519-signing the bytes.
/// Verification deserializes to the same canonical form and checks the signature.
pub struct DeviceAuthenticator {
    identity: Arc<DeviceIdentity>,
}

impl DeviceAuthenticator {
    pub fn new(identity: Arc<DeviceIdentity>) -> Self {
        Self { identity }
    }
}

impl<T: Serialize> FlowAuthenticator<T> for DeviceAuthenticator {
    fn sign(&self, data: &T) -> Result<Vec<u8>, FlowError> {
        let json = serde_json::to_vec(data)
            .map_err(|e| FlowError::SerializationError(e.to_string()))?;
        let sig = self.identity.sign(&json);
        Ok(sig.to_bytes().to_vec())
    }

    fn verify(&self, data: &T, signature: &[u8]) -> bool {
        let Ok(json) = serde_json::to_vec(data) else {
            return false;
        };

        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };

        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        self.identity.verify(&json, &sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: u64,
    }

    #[test]
    fn test_sign_and_verify_flow_data() {
        let identity = Arc::new(DeviceIdentity::generate());
        let auth = DeviceAuthenticator::new(identity);

        let data = TestData {
            name: "task_count".to_string(),
            value: 42,
        };

        let sig = auth.sign(&data).unwrap();
        assert!(auth.verify(&data, &sig));
    }

    #[test]
    fn test_tampered_data_fails_verification() {
        let identity = Arc::new(DeviceIdentity::generate());
        let auth = DeviceAuthenticator::new(identity);

        let data = TestData {
            name: "count".to_string(),
            value: 10,
        };

        let sig = auth.sign(&data).unwrap();

        let tampered = TestData {
            name: "count".to_string(),
            value: 11,
        };

        assert!(!auth.verify(&tampered, &sig));
    }

    #[test]
    fn test_wrong_identity_fails_verification() {
        let alice = Arc::new(DeviceIdentity::generate());
        let bob = Arc::new(DeviceIdentity::generate());

        let auth_alice = DeviceAuthenticator::new(alice);
        let auth_bob = DeviceAuthenticator::new(bob);

        let data = TestData {
            name: "test".to_string(),
            value: 1,
        };

        let sig = auth_alice.sign(&data).unwrap();

        // Bob's authenticator should not verify Alice's signature
        assert!(!auth_bob.verify(&data, &sig));
    }

    #[test]
    fn test_invalid_signature_bytes() {
        let identity = Arc::new(DeviceIdentity::generate());
        let auth = DeviceAuthenticator::new(identity);

        let data = TestData {
            name: "x".to_string(),
            value: 0,
        };

        // Too short
        assert!(!auth.verify(&data, &[0u8; 10]));

        // Right length but garbage
        assert!(!auth.verify(&data, &[0u8; 64]));
    }
}
