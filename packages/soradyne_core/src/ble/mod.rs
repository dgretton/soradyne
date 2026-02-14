//! BLE transport layer for the Rim protocol
//!
//! Provides BLE abstraction traits, a simulated transport for testing,
//! encrypted advertisement payloads, and the RoutedEnvelope message format.

pub mod encrypted_adv;
pub mod gatt;
pub mod simulated;
pub mod transport;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BleError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Advertising error: {0}")]
    AdvertisingError(String),

    #[error("Scan error: {0}")]
    ScanError(String),

    #[error("GATT error: {0}")]
    GattError(String),

    #[error("Payload exceeds MTU ({size} > {mtu})")]
    MtuExceeded { size: usize, mtu: usize },

    #[error("Peer disconnected")]
    Disconnected,

    #[error("Operation timed out")]
    Timeout,
}
