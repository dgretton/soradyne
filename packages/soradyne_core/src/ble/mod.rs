//! BLE transport layer for the rim protocol
//!
//! Provides BLE abstraction traits, a simulated transport for testing,
//! encrypted advertisement payloads, and the RoutedEnvelope message format.

pub mod encrypted_adv;
pub mod framing;
pub mod gatt;
pub mod lan_transport;
pub mod mdns_transport;
pub mod session;
pub mod simulated;
pub mod transport;

#[cfg(feature = "ble-central")]
pub mod btleplug_central;

#[cfg(target_os = "android")]
pub mod android_peripheral;

#[cfg(feature = "tcp-transport")]
pub mod tcp_transport;

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
