//! Pairing protocol engine
//!
//! Implements the BLE pairing flow that allows a new device (joiner) to join
//! an existing capsule by exchanging key material with an existing member (inviter).
//!
//! The protocol uses X25519 ECDH for key agreement, a 6-digit numeric PIN for
//! MITM protection (passkey-entry model: inviter displays, joiner types), and
//! AES-256-GCM encryption (via `CapsuleKeyBundle::encrypt_for_transfer`) for
//! the capsule material transfer.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;
use x25519_dalek::PublicKey as X25519PublicKey;

use crate::ble::transport::{BleCentral, BleConnection, BlePeripheral};
use crate::ble::BleError;
use crate::identity::{CapsuleKeyBundle, DeviceIdentity};
use crate::topology::capsule::{Capsule, PieceCapabilities, PieceRecord, PieceRole};
use crate::topology::capsule_store::CapsuleStore;

/// Magic prefix for pairing advertisements, distinguishing them from
/// normal ensemble advertisements.
pub const PAIRING_ADV_MARKER: &[u8] = b"SORADYNE-PAIR-V1";

// ---------------------------------------------------------------------------
// Wire messages
// ---------------------------------------------------------------------------

/// Messages exchanged over a BLE connection during pairing.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PairingMessage {
    /// Both sides send this first.
    KeyExchange {
        device_id: Uuid,
        dh_public_key: [u8; 32],
        verifying_key: [u8; 32],
    },
    /// Joiner confirms the entered PIN matched.
    PinConfirmed,
    /// Inviter sends the capsule material (encrypted with the shared secret).
    CapsuleTransfer { encrypted_capsule: Vec<u8> },
    /// Joiner sends its PieceRecord (encrypted with the shared secret).
    JoinerPieceInfo { encrypted_piece: Vec<u8> },
    /// Both sides send this when done.
    PairingComplete,
    /// Either side can reject.
    Rejected { reason: String },
}

// ---------------------------------------------------------------------------
// State & errors
// ---------------------------------------------------------------------------

/// Observable state of the pairing engine, returned to the UI layer.
#[derive(Clone, Debug)]
pub enum PairingState {
    Idle,
    AwaitingVerification { pin: String },
    Transferring,
    Complete { capsule_id: Uuid, peer_device_id: Uuid },
    Failed { reason: String },
}

/// Errors that can occur during pairing.
#[derive(Error, Debug)]
pub enum PairingError {
    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Transport error: {0}")]
    TransportError(#[from] BleError),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Rejected: {0}")]
    Rejected(String),

    #[error("Capsule not found: {0}")]
    CapsuleNotFound(Uuid),
}

/// Successful pairing result.
#[derive(Clone, Debug)]
pub struct PairingResult {
    pub capsule_id: Uuid,
    pub peer_device_id: Uuid,
}

// ---------------------------------------------------------------------------
// PIN verification
// ---------------------------------------------------------------------------

/// Trait for deriving a human-readable verification code from the ECDH shared
/// secret.  Extracted as a trait so tests can inject deterministic verifiers.
pub trait PairingVerifier: Send + Sync {
    fn derive_pin(&self, shared_secret: &[u8; 32]) -> String;
}

/// Default verifier: SHA-256 of `(secret || domain-tag)` → first 4 bytes →
/// u32 mod 1 000 000 → zero-padded 6-digit string.
pub struct NumericPinVerifier;

impl PairingVerifier for NumericPinVerifier {
    fn derive_pin(&self, shared_secret: &[u8; 32]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret);
        hasher.update(b"soradyne-pin-v1");
        let hash = hasher.finalize();
        let raw = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
        format!("{:06}", raw % 1_000_000)
    }
}

// ---------------------------------------------------------------------------
// Internal session state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum PairingRole {
    Inviter,
    Joiner,
}

struct PairingSession {
    connection: Box<dyn BleConnection>,
    shared_secret: [u8; 32],
    peer_device_id: Uuid,
    role: PairingRole,
    capsule_id: Uuid,
    /// Inviter: the full capsule to transfer.  Joiner: set after receiving.
    capsule_data: Option<Capsule>,
    /// Joiner only: info for constructing its PieceRecord.
    joiner_piece_info: Option<(String, PieceCapabilities, PieceRole)>,
}

// ---------------------------------------------------------------------------
// CBOR helpers
// ---------------------------------------------------------------------------

fn cbor_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, PairingError> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)
        .map_err(|e| PairingError::SerializationError(e.to_string()))?;
    Ok(buf)
}

fn cbor_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, PairingError> {
    ciborium::from_reader(data).map_err(|e| PairingError::SerializationError(e.to_string()))
}

/// Send a `PairingMessage` over a BLE connection.
async fn send_msg(conn: &dyn BleConnection, msg: &PairingMessage) -> Result<(), PairingError> {
    let bytes = cbor_serialize(msg)?;
    conn.send(&bytes).await.map_err(PairingError::TransportError)
}

/// Receive a `PairingMessage` from a BLE connection.
async fn recv_msg(conn: &dyn BleConnection) -> Result<PairingMessage, PairingError> {
    let bytes = conn.recv().await.map_err(PairingError::TransportError)?;
    cbor_deserialize(&bytes)
}

// ---------------------------------------------------------------------------
// PairingEngine
// ---------------------------------------------------------------------------

/// Self-contained state machine that drives the pairing protocol.
///
/// Any UI layer (Flutter, Tauri, CLI) drives it through the public API:
///
/// **Inviter:** `invite()` → read `verification_pin()` → display to user →
///   `confirm_pin()` → done.
///
/// **Joiner:** `join()` → user enters PIN → `submit_pin()` → done.
pub struct PairingEngine {
    identity: Arc<DeviceIdentity>,
    verifier: Box<dyn PairingVerifier>,
    state: RwLock<PairingState>,
    session: RwLock<Option<PairingSession>>,
}

impl PairingEngine {
    /// Create a new engine with the default `NumericPinVerifier`.
    pub fn new(identity: Arc<DeviceIdentity>) -> Self {
        Self {
            identity,
            verifier: Box::new(NumericPinVerifier),
            state: RwLock::new(PairingState::Idle),
            session: RwLock::new(None),
        }
    }

    /// Create a new engine with a custom verifier (useful for testing).
    pub fn with_verifier(
        identity: Arc<DeviceIdentity>,
        verifier: Box<dyn PairingVerifier>,
    ) -> Self {
        Self {
            identity,
            verifier,
            state: RwLock::new(PairingState::Idle),
            session: RwLock::new(None),
        }
    }

    /// Current observable state.
    pub async fn state(&self) -> PairingState {
        self.state.read().await.clone()
    }

    // -----------------------------------------------------------------------
    // Inviter path
    // -----------------------------------------------------------------------

    /// Start inviting: advertise, accept a connection, perform key exchange,
    /// and derive the verification PIN.
    ///
    /// On success the state moves to `AwaitingVerification`.  The caller
    /// should then read `verification_pin()` and display it.
    pub async fn invite(
        &self,
        capsule_id: Uuid,
        capsule_store: &CapsuleStore,
        peripheral: &dyn BlePeripheral,
    ) -> Result<(), PairingError> {
        // Must be idle
        {
            let st = self.state.read().await;
            if !matches!(*st, PairingState::Idle) {
                return Err(PairingError::InvalidState(
                    "invite() requires Idle state".into(),
                ));
            }
        }

        // Read capsule
        let capsule = capsule_store
            .get_capsule(&capsule_id)
            .ok_or(PairingError::CapsuleNotFound(capsule_id))?
            .clone();

        // Advertise with the pairing marker
        peripheral
            .start_advertising(PAIRING_ADV_MARKER.to_vec())
            .await
            .map_err(PairingError::TransportError)?;

        // Accept incoming connection from joiner
        let conn = peripheral
            .accept()
            .await
            .map_err(PairingError::TransportError)?;

        // Stop advertising
        peripheral
            .stop_advertising()
            .await
            .map_err(PairingError::TransportError)?;

        // Key exchange: inviter sends first, then receives
        let my_ke = PairingMessage::KeyExchange {
            device_id: self.identity.device_id(),
            dh_public_key: self.identity.dh_public_bytes(),
            verifying_key: self.identity.verifying_key_bytes(),
        };
        send_msg(conn.as_ref(), &my_ke).await?;

        let peer_ke = recv_msg(conn.as_ref()).await?;
        let (peer_device_id, peer_dh_pub) = match peer_ke {
            PairingMessage::KeyExchange {
                device_id,
                dh_public_key,
                ..
            } => {
                let pub_key = X25519PublicKey::from(dh_public_key);
                (device_id, pub_key)
            }
            PairingMessage::Rejected { reason } => {
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::Rejected(reason));
            }
            other => {
                return Err(PairingError::InvalidState(format!(
                    "Expected KeyExchange, got {:?}",
                    msg_name(&other),
                )));
            }
        };

        // ECDH
        let shared_secret = self.identity.dh_agree(&peer_dh_pub);
        let pin = self.verifier.derive_pin(&shared_secret);

        // Store session
        *self.session.write().await = Some(PairingSession {
            connection: conn,
            shared_secret,
            peer_device_id,
            role: PairingRole::Inviter,
            capsule_id,
            capsule_data: Some(capsule),
            joiner_piece_info: None,
        });

        *self.state.write().await = PairingState::AwaitingVerification { pin };

        Ok(())
    }

    /// Returns the 6-digit PIN for the inviter to display on screen.
    /// Only returns `Some` when state is `AwaitingVerification` and role is Inviter.
    pub async fn verification_pin(&self) -> Option<String> {
        let st = self.state.read().await;
        if let PairingState::AwaitingVerification { pin } = &*st {
            let sess = self.session.read().await;
            if let Some(s) = sess.as_ref() {
                if s.role == PairingRole::Inviter {
                    return Some(pin.clone());
                }
            }
        }
        None
    }

    /// Inviter: wait for the joiner's confirmation, then exchange capsule
    /// material.  Returns the `PairingResult` on success.
    pub async fn confirm_pin(
        &self,
        capsule_store: &mut CapsuleStore,
    ) -> Result<PairingResult, PairingError> {
        // Validate state
        {
            let st = self.state.read().await;
            if !matches!(*st, PairingState::AwaitingVerification { .. }) {
                return Err(PairingError::InvalidState(
                    "confirm_pin() requires AwaitingVerification state".into(),
                ));
            }
            let sess = self.session.read().await;
            match sess.as_ref() {
                Some(s) if s.role == PairingRole::Inviter => {}
                _ => {
                    return Err(PairingError::InvalidState(
                        "confirm_pin() is only valid for the inviter".into(),
                    ));
                }
            }
        }

        *self.state.write().await = PairingState::Transferring;

        // Wait for joiner's PinConfirmed (or Rejected)
        let msg = {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            recv_msg(conn.as_ref()).await?
        };

        match msg {
            PairingMessage::PinConfirmed => { /* proceed */ }
            PairingMessage::Rejected { reason } => {
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::Rejected(reason));
            }
            other => {
                let reason = format!("Expected PinConfirmed, got {:?}", msg_name(&other));
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::InvalidState(reason));
            }
        }

        // Send capsule
        let (capsule_bytes, shared_secret, capsule_id, peer_device_id) = {
            let sess = self.session.read().await;
            let s = sess.as_ref().unwrap();
            let bytes = s
                .capsule_data
                .as_ref()
                .unwrap()
                .to_gossip_bytes()
                .map_err(|e| PairingError::SerializationError(e.to_string()))?;
            (bytes, s.shared_secret, s.capsule_id, s.peer_device_id)
        };

        let encrypted_capsule =
            CapsuleKeyBundle::encrypt_for_transfer(&capsule_bytes, &shared_secret)
                .map_err(|e| PairingError::CryptoError(e.to_string()))?;

        {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            send_msg(
                conn.as_ref(),
                &PairingMessage::CapsuleTransfer { encrypted_capsule },
            )
            .await?;
        }

        // Receive joiner's PieceRecord
        let piece_record: PieceRecord = {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            let msg = recv_msg(conn.as_ref()).await?;
            match msg {
                PairingMessage::JoinerPieceInfo { encrypted_piece } => {
                    let decrypted =
                        CapsuleKeyBundle::decrypt_for_transfer(&encrypted_piece, &shared_secret)
                            .map_err(|e| PairingError::CryptoError(e.to_string()))?;
                    cbor_deserialize(&decrypted)?
                }
                PairingMessage::Rejected { reason } => {
                    *self.state.write().await = PairingState::Failed {
                        reason: reason.clone(),
                    };
                    return Err(PairingError::Rejected(reason));
                }
                other => {
                    let reason =
                        format!("Expected JoinerPieceInfo, got {:?}", msg_name(&other));
                    *self.state.write().await = PairingState::Failed {
                        reason: reason.clone(),
                    };
                    return Err(PairingError::InvalidState(reason));
                }
            }
        };

        // Add joiner's piece to our capsule
        capsule_store
            .add_piece(&capsule_id, piece_record)
            .map_err(|e| PairingError::SerializationError(e.to_string()))?;

        // Exchange PairingComplete
        {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            send_msg(conn.as_ref(), &PairingMessage::PairingComplete).await?;
            let final_msg = recv_msg(conn.as_ref()).await?;
            if let PairingMessage::Rejected { reason } = final_msg {
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::Rejected(reason));
            }
        }

        let result = PairingResult {
            capsule_id,
            peer_device_id,
        };

        *self.state.write().await = PairingState::Complete {
            capsule_id,
            peer_device_id,
        };

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Joiner path
    // -----------------------------------------------------------------------

    /// Start joining: scan for pairing advertisements, connect, perform key
    /// exchange, and derive the verification PIN.
    ///
    /// On success the state moves to `AwaitingVerification`.  The caller
    /// should then prompt the user to enter the PIN displayed on the inviter.
    pub async fn join(
        &self,
        piece_name: String,
        piece_capabilities: PieceCapabilities,
        piece_role: PieceRole,
        central: &dyn BleCentral,
    ) -> Result<(), PairingError> {
        // Must be idle
        {
            let st = self.state.read().await;
            if !matches!(*st, PairingState::Idle) {
                return Err(PairingError::InvalidState(
                    "join() requires Idle state".into(),
                ));
            }
        }

        // Subscribe to advertisements BEFORE starting the scan, so we
        // don't miss any that arrive between start_scan and subscribe.
        let mut rx = central.advertisements();

        // Start scanning
        central
            .start_scan()
            .await
            .map_err(PairingError::TransportError)?;
        let source_address = loop {
            let adv = rx
                .recv()
                .await
                .map_err(|_| PairingError::TransportError(BleError::ScanError(
                    "Advertisement channel closed".into(),
                )))?;
            if adv.data.starts_with(PAIRING_ADV_MARKER) {
                break adv.source_address;
            }
        };

        // Stop scanning
        central
            .stop_scan()
            .await
            .map_err(PairingError::TransportError)?;

        // Connect to the inviter
        let conn = central
            .connect(&source_address)
            .await
            .map_err(PairingError::TransportError)?;

        // Key exchange: joiner receives first, then sends
        let peer_ke = recv_msg(conn.as_ref()).await?;
        let (peer_device_id, peer_dh_pub) = match peer_ke {
            PairingMessage::KeyExchange {
                device_id,
                dh_public_key,
                ..
            } => {
                let pub_key = X25519PublicKey::from(dh_public_key);
                (device_id, pub_key)
            }
            PairingMessage::Rejected { reason } => {
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::Rejected(reason));
            }
            other => {
                return Err(PairingError::InvalidState(format!(
                    "Expected KeyExchange, got {:?}",
                    msg_name(&other),
                )));
            }
        };

        let my_ke = PairingMessage::KeyExchange {
            device_id: self.identity.device_id(),
            dh_public_key: self.identity.dh_public_bytes(),
            verifying_key: self.identity.verifying_key_bytes(),
        };
        send_msg(conn.as_ref(), &my_ke).await?;

        // ECDH
        let shared_secret = self.identity.dh_agree(&peer_dh_pub);
        let pin = self.verifier.derive_pin(&shared_secret);

        // Store session
        *self.session.write().await = Some(PairingSession {
            connection: conn,
            shared_secret,
            peer_device_id,
            role: PairingRole::Joiner,
            capsule_id: Uuid::nil(), // will be set after receiving capsule
            capsule_data: None,
            joiner_piece_info: Some((piece_name, piece_capabilities, piece_role)),
        });

        *self.state.write().await = PairingState::AwaitingVerification { pin };

        Ok(())
    }

    /// Joiner: verify the PIN entered by the user against the locally-derived
    /// PIN, then exchange capsule material.  Returns the `PairingResult` on
    /// success.
    pub async fn submit_pin(
        &self,
        entered_pin: &str,
        capsule_store: &mut CapsuleStore,
    ) -> Result<PairingResult, PairingError> {
        // Validate state
        let expected_pin = {
            let st = self.state.read().await;
            match &*st {
                PairingState::AwaitingVerification { pin } => pin.clone(),
                _ => {
                    return Err(PairingError::InvalidState(
                        "submit_pin() requires AwaitingVerification state".into(),
                    ));
                }
            }
        };
        {
            let sess = self.session.read().await;
            match sess.as_ref() {
                Some(s) if s.role == PairingRole::Joiner => {}
                _ => {
                    return Err(PairingError::InvalidState(
                        "submit_pin() is only valid for the joiner".into(),
                    ));
                }
            }
        }

        // Verify PIN
        if entered_pin != expected_pin {
            // Send rejection to inviter
            {
                let sess = self.session.read().await;
                let conn = &sess.as_ref().unwrap().connection;
                let _ = send_msg(
                    conn.as_ref(),
                    &PairingMessage::Rejected {
                        reason: "PIN mismatch".into(),
                    },
                )
                .await;
            }
            *self.state.write().await = PairingState::Failed {
                reason: "PIN mismatch".into(),
            };
            return Err(PairingError::Rejected("PIN mismatch".into()));
        }

        *self.state.write().await = PairingState::Transferring;

        // Send PinConfirmed
        {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            send_msg(conn.as_ref(), &PairingMessage::PinConfirmed).await?;
        }

        // Receive capsule
        let shared_secret = {
            let sess = self.session.read().await;
            sess.as_ref().unwrap().shared_secret
        };

        let capsule: Capsule = {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            let msg = recv_msg(conn.as_ref()).await?;
            match msg {
                PairingMessage::CapsuleTransfer { encrypted_capsule } => {
                    let decrypted =
                        CapsuleKeyBundle::decrypt_for_transfer(&encrypted_capsule, &shared_secret)
                            .map_err(|e| PairingError::CryptoError(e.to_string()))?;
                    Capsule::from_gossip_bytes(&decrypted)
                        .map_err(|e| PairingError::SerializationError(e.to_string()))?
                }
                PairingMessage::Rejected { reason } => {
                    *self.state.write().await = PairingState::Failed {
                        reason: reason.clone(),
                    };
                    return Err(PairingError::Rejected(reason));
                }
                other => {
                    let reason =
                        format!("Expected CapsuleTransfer, got {:?}", msg_name(&other));
                    *self.state.write().await = PairingState::Failed {
                        reason: reason.clone(),
                    };
                    return Err(PairingError::InvalidState(reason));
                }
            }
        };

        let capsule_id = capsule.id;
        let peer_device_id = {
            let sess = self.session.read().await;
            sess.as_ref().unwrap().peer_device_id
        };

        // Build joiner's PieceRecord
        let (piece_name, piece_capabilities, piece_role) = {
            let sess = self.session.read().await;
            sess.as_ref()
                .unwrap()
                .joiner_piece_info
                .clone()
                .unwrap()
        };
        let my_piece = PieceRecord::from_identity(
            &self.identity,
            piece_name,
            piece_capabilities,
            piece_role,
        );

        // Store capsule locally (add our own piece first)
        let mut capsule = capsule;
        capsule.add_piece(my_piece.clone());
        capsule_store
            .insert_capsule(capsule)
            .map_err(|e| PairingError::SerializationError(e.to_string()))?;

        // Send our PieceRecord to the inviter (encrypted)
        let piece_bytes = cbor_serialize(&my_piece)?;
        let encrypted_piece =
            CapsuleKeyBundle::encrypt_for_transfer(&piece_bytes, &shared_secret)
                .map_err(|e| PairingError::CryptoError(e.to_string()))?;

        {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            send_msg(
                conn.as_ref(),
                &PairingMessage::JoinerPieceInfo { encrypted_piece },
            )
            .await?;
        }

        // Exchange PairingComplete
        {
            let sess = self.session.read().await;
            let conn = &sess.as_ref().unwrap().connection;
            send_msg(conn.as_ref(), &PairingMessage::PairingComplete).await?;
            let final_msg = recv_msg(conn.as_ref()).await?;
            if let PairingMessage::Rejected { reason } = final_msg {
                *self.state.write().await = PairingState::Failed {
                    reason: reason.clone(),
                };
                return Err(PairingError::Rejected(reason));
            }
        }

        // Update session with capsule_id
        {
            let mut sess = self.session.write().await;
            if let Some(s) = sess.as_mut() {
                s.capsule_id = capsule_id;
            }
        }

        let result = PairingResult {
            capsule_id,
            peer_device_id,
        };

        *self.state.write().await = PairingState::Complete {
            capsule_id,
            peer_device_id,
        };

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Cancellation
    // -----------------------------------------------------------------------

    /// Cancel an in-progress pairing from either side.
    pub async fn cancel(&self) -> Result<(), PairingError> {
        let sess = self.session.write().await;
        if let Some(s) = sess.as_ref() {
            let _ = send_msg(
                s.connection.as_ref(),
                &PairingMessage::Rejected {
                    reason: "Cancelled by user".into(),
                },
            )
            .await;
            let _ = s.connection.disconnect().await;
        }
        drop(sess);

        *self.state.write().await = PairingState::Failed {
            reason: "Cancelled by user".into(),
        };
        *self.session.write().await = None;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Simulated accessory helper (4.2)
// ---------------------------------------------------------------------------

/// Shortcut: add a simulated accessory to a capsule without BLE or ECDH.
///
/// Generates a fresh identity, creates a `PieceRecord` with `PieceRole::Accessory`
/// and `PieceCapabilities::accessory()`, and adds it to the capsule.
/// Returns the identity and piece record so the caller can use the identity
/// to create a `SimBleDevice` and participate in the ensemble.
pub async fn pair_simulated_accessory(
    capsule_store: &mut CapsuleStore,
    capsule_id: Uuid,
    accessory_name: &str,
) -> Result<(DeviceIdentity, PieceRecord), PairingError> {
    let identity = DeviceIdentity::generate();
    let piece = PieceRecord::from_identity(
        &identity,
        accessory_name.to_string(),
        PieceCapabilities::accessory(),
        PieceRole::Accessory,
    );

    capsule_store
        .add_piece(&capsule_id, piece.clone())
        .map_err(|e| PairingError::SerializationError(e.to_string()))?;

    Ok((identity, piece))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Human-readable name for a `PairingMessage` variant (for error messages).
fn msg_name(msg: &PairingMessage) -> &'static str {
    match msg {
        PairingMessage::KeyExchange { .. } => "KeyExchange",
        PairingMessage::PinConfirmed => "PinConfirmed",
        PairingMessage::CapsuleTransfer { .. } => "CapsuleTransfer",
        PairingMessage::JoinerPieceInfo { .. } => "JoinerPieceInfo",
        PairingMessage::PairingComplete => "PairingComplete",
        PairingMessage::Rejected { .. } => "Rejected",
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::simulated::SimBleNetwork;
    #[allow(unused_imports)]
    use crate::ble::transport::{BleCentral, BlePeripheral};
    use crate::identity::CapsuleKeyBundle;
    use std::sync::Arc;

    /// Helper: create a CapsuleStore backed by a temp dir.
    fn temp_capsule_store() -> (tempfile::TempDir, CapsuleStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capsules");
        let store = CapsuleStore::new(path);
        (dir, store)
    }

    /// Helper: create a capsule in the store and return its id + the inviter identity.
    fn setup_inviter(
        store: &mut CapsuleStore,
    ) -> (Uuid, Arc<DeviceIdentity>) {
        let identity = Arc::new(DeviceIdentity::generate());
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let capsule_id = store.create_capsule("test capsule", keys).unwrap();

        // Add inviter as a piece
        let piece = PieceRecord::from_identity(
            &identity,
            "inviter".to_string(),
            PieceCapabilities::full(),
            PieceRole::Full,
        );
        store.add_piece(&capsule_id, piece).unwrap();

        (capsule_id, identity)
    }

    // -----------------------------------------------------------------------
    // 1. PIN derivation
    // -----------------------------------------------------------------------

    #[test]
    fn test_pin_derivation_deterministic() {
        let verifier = NumericPinVerifier;

        let secret_a = [42u8; 32];
        let secret_b = [99u8; 32];

        let pin_a1 = verifier.derive_pin(&secret_a);
        let pin_a2 = verifier.derive_pin(&secret_a);
        let pin_b = verifier.derive_pin(&secret_b);

        // Same secret → same PIN
        assert_eq!(pin_a1, pin_a2);
        // Different secrets → different PINs
        assert_ne!(pin_a1, pin_b);
        // 6 digits
        assert_eq!(pin_a1.len(), 6);
        assert!(pin_a1.chars().all(|c| c.is_ascii_digit()));
    }

    // -----------------------------------------------------------------------
    // 2. PairingMessage CBOR round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_pairing_messages_cbor_round_trip() {
        let variants: Vec<PairingMessage> = vec![
            PairingMessage::KeyExchange {
                device_id: Uuid::new_v4(),
                dh_public_key: [1u8; 32],
                verifying_key: [2u8; 32],
            },
            PairingMessage::PinConfirmed,
            PairingMessage::CapsuleTransfer {
                encrypted_capsule: vec![0xCA, 0xFE],
            },
            PairingMessage::JoinerPieceInfo {
                encrypted_piece: vec![0xBE, 0xEF],
            },
            PairingMessage::PairingComplete,
            PairingMessage::Rejected {
                reason: "test rejection".into(),
            },
        ];

        for msg in &variants {
            let bytes = cbor_serialize(msg).unwrap();
            let restored: PairingMessage = cbor_deserialize(&bytes).unwrap();

            // Verify round-trip by re-serializing
            let bytes2 = cbor_serialize(&restored).unwrap();
            assert_eq!(bytes, bytes2, "CBOR round-trip failed for {:?}", msg_name(msg));
        }
    }

    // -----------------------------------------------------------------------
    // 3. Full pairing flow (happy path)
    // -----------------------------------------------------------------------

    #[tokio::test(start_paused = true)]
    async fn test_full_pairing_flow() {
        let network = SimBleNetwork::new();
        let mut inviter_device = network.create_device();
        let mut joiner_device = network.create_device();
        // Capsule transfer messages are ~1.2 KB when encrypted, so use a
        // larger MTU than the 512-byte manager tests.
        inviter_device.set_mtu(4096);
        joiner_device.set_mtu(4096);

        // Set up inviter
        let (_inv_dir, mut inv_store) = temp_capsule_store();
        let (capsule_id, inv_identity) = setup_inviter(&mut inv_store);
        let inv_engine = PairingEngine::new(inv_identity);

        // Set up joiner
        let (_join_dir, mut join_store) = temp_capsule_store();
        let join_identity = Arc::new(DeviceIdentity::generate());
        let join_engine = PairingEngine::new(join_identity.clone());

        // Spawn joiner FIRST so it subscribes to advertisements before
        // the inviter starts advertising (current_thread runtime).
        let join_handle = tokio::spawn(async move {
            join_engine
                .join(
                    "joiner-phone".to_string(),
                    PieceCapabilities::full(),
                    PieceRole::Full,
                    &joiner_device,
                )
                .await
                .unwrap();

            // Read the PIN from the engine's state (in a real app the user
            // would read it from the inviter's screen and type it in)
            let pin = match join_engine.state().await {
                PairingState::AwaitingVerification { pin } => pin,
                other => panic!("Expected AwaitingVerification, got {:?}", format!("{:?}", other)),
            };

            let result = join_engine
                .submit_pin(&pin, &mut join_store)
                .await
                .unwrap();

            // Verify joiner got the capsule
            let capsule = join_store.get_capsule(&result.capsule_id).unwrap();
            assert_eq!(capsule.name, "test capsule");
            // Joiner's local copy: inviter piece + joiner piece
            assert_eq!(capsule.pieces.len(), 2);

            result
        });

        let inv_handle = tokio::spawn(async move {
            inv_engine
                .invite(capsule_id, &inv_store, &inviter_device)
                .await
                .unwrap();

            let pin = inv_engine.verification_pin().await.unwrap();
            assert_eq!(pin.len(), 6);

            let result = inv_engine.confirm_pin(&mut inv_store).await.unwrap();
            assert_eq!(result.capsule_id, capsule_id);

            // Verify joiner was added to inviter's capsule
            let capsule = inv_store.get_capsule(&capsule_id).unwrap();
            assert_eq!(capsule.pieces.len(), 2); // inviter + joiner

            result
        });

        let (join_result, inv_result) = tokio::join!(join_handle, inv_handle);
        let join_result = join_result.unwrap();
        let inv_result = inv_result.unwrap();

        // Both sides agree on the capsule
        assert_eq!(inv_result.capsule_id, join_result.capsule_id);
    }

    // -----------------------------------------------------------------------
    // 4. Wrong PIN rejected
    // -----------------------------------------------------------------------

    #[tokio::test(start_paused = true)]
    async fn test_wrong_pin_rejected() {
        let network = SimBleNetwork::new();
        let mut inviter_device = network.create_device();
        let mut joiner_device = network.create_device();
        // Capsule transfer messages are ~1.2 KB when encrypted, so use a
        // larger MTU than the 512-byte manager tests.
        inviter_device.set_mtu(4096);
        joiner_device.set_mtu(4096);

        let (_inv_dir, mut inv_store) = temp_capsule_store();
        let (capsule_id, inv_identity) = setup_inviter(&mut inv_store);
        let inv_engine = PairingEngine::new(inv_identity);

        let (_join_dir, mut join_store) = temp_capsule_store();
        let join_identity = Arc::new(DeviceIdentity::generate());
        let join_engine = PairingEngine::new(join_identity);

        // Spawn joiner first
        let join_handle = tokio::spawn(async move {
            join_engine
                .join(
                    "joiner".to_string(),
                    PieceCapabilities::full(),
                    PieceRole::Full,
                    &joiner_device,
                )
                .await
                .unwrap();

            // Submit a WRONG pin
            let result = join_engine
                .submit_pin("000000", &mut join_store)
                .await;

            assert!(result.is_err());
            match result.unwrap_err() {
                PairingError::Rejected(reason) => {
                    assert!(reason.contains("PIN mismatch"));
                }
                other => panic!("Expected Rejected, got {:?}", other),
            }

            assert!(matches!(join_engine.state().await, PairingState::Failed { .. }));
        });

        let inv_handle = tokio::spawn(async move {
            inv_engine
                .invite(capsule_id, &inv_store, &inviter_device)
                .await
                .unwrap();

            // Inviter waits for confirmation — should get rejection
            let result = inv_engine.confirm_pin(&mut inv_store).await;
            assert!(result.is_err());
            match result.unwrap_err() {
                PairingError::Rejected(reason) => {
                    assert!(reason.contains("PIN mismatch"));
                }
                other => panic!("Expected Rejected, got {:?}", other),
            }

            // State should be Failed
            assert!(matches!(inv_engine.state().await, PairingState::Failed { .. }));
        });

        let (join_res, inv_res) = tokio::join!(join_handle, inv_handle);
        join_res.unwrap();
        inv_res.unwrap();
    }

    // -----------------------------------------------------------------------
    // 5. Cancel pairing
    // -----------------------------------------------------------------------

    #[tokio::test(start_paused = true)]
    async fn test_cancel_pairing() {
        let network = SimBleNetwork::new();
        let mut inviter_device = network.create_device();
        let mut joiner_device = network.create_device();
        // Capsule transfer messages are ~1.2 KB when encrypted, so use a
        // larger MTU than the 512-byte manager tests.
        inviter_device.set_mtu(4096);
        joiner_device.set_mtu(4096);

        let (_inv_dir, mut inv_store) = temp_capsule_store();
        let (capsule_id, inv_identity) = setup_inviter(&mut inv_store);
        let inv_engine = Arc::new(PairingEngine::new(inv_identity));

        let (_join_dir, _join_store) = temp_capsule_store();
        let join_identity = Arc::new(DeviceIdentity::generate());
        let join_engine = PairingEngine::new(join_identity);

        let inv_engine_cancel = Arc::clone(&inv_engine);

        // Spawn joiner first
        let join_handle = tokio::spawn(async move {
            join_engine
                .join(
                    "joiner".to_string(),
                    PieceCapabilities::full(),
                    PieceRole::Full,
                    &joiner_device,
                )
                .await
                .unwrap();

            // The joiner is now in AwaitingVerification, but the inviter
            // will cancel. We just verify the state is AwaitingVerification.
            assert!(matches!(
                join_engine.state().await,
                PairingState::AwaitingVerification { .. }
            ));
        });

        let inv_handle = tokio::spawn(async move {
            inv_engine
                .invite(capsule_id, &inv_store, &inviter_device)
                .await
                .unwrap();

            // State should be AwaitingVerification
            assert!(matches!(
                inv_engine.state().await,
                PairingState::AwaitingVerification { .. }
            ));

            // Inviter cancels
            inv_engine_cancel.cancel().await.unwrap();

            assert!(matches!(
                inv_engine_cancel.state().await,
                PairingState::Failed { .. }
            ));
        });

        let (join_res, inv_res) = tokio::join!(join_handle, inv_handle);
        join_res.unwrap();
        inv_res.unwrap();
    }

    // -----------------------------------------------------------------------
    // 6. Invalid state errors
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_invalid_state_errors() {
        let identity = Arc::new(DeviceIdentity::generate());
        let engine = PairingEngine::new(identity);

        let (_dir, mut store) = temp_capsule_store();

        // confirm_pin before invite
        let result = engine.confirm_pin(&mut store).await;
        assert!(matches!(result, Err(PairingError::InvalidState(_))));

        // submit_pin before join
        let result = engine.submit_pin("123456", &mut store).await;
        assert!(matches!(result, Err(PairingError::InvalidState(_))));

        // verification_pin before invite
        assert!(engine.verification_pin().await.is_none());
    }

    // -----------------------------------------------------------------------
    // 7. Simulated accessory pairing
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_simulated_accessory_pairing() {
        let (_dir, mut store) = temp_capsule_store();
        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let capsule_id = store.create_capsule("test", keys).unwrap();

        let (identity, piece) = pair_simulated_accessory(
            &mut store,
            capsule_id,
            "temperature sensor",
        )
        .await
        .unwrap();

        // Verify accessory appears in the capsule
        let capsule = store.get_capsule(&capsule_id).unwrap();
        assert_eq!(capsule.pieces.len(), 1);

        let stored_piece = capsule.find_piece(&identity.device_id()).unwrap();
        assert_eq!(stored_piece.name, "temperature sensor");
        assert_eq!(stored_piece.role, PieceRole::Accessory);
        assert_eq!(stored_piece.capabilities, PieceCapabilities::accessory());
        assert_eq!(stored_piece.device_id, piece.device_id);
    }
}
