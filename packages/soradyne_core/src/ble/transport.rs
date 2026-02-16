//! BLE transport trait definitions and core types
//!
//! Defines the abstract BLE interface that both the simulated transport
//! and future real BLE (btleplug) implementations conform to.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::BleError;

/// A BLE device address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BleAddress {
    /// A real 6-byte BLE MAC address.
    Real([u8; 6]),
    /// A simulated address identified by UUID.
    Simulated(Uuid),
}

/// A received BLE advertisement.
#[derive(Debug, Clone)]
pub struct BleAdvertisement {
    /// Raw advertisement data.
    pub data: Vec<u8>,
    /// Received signal strength indicator (if available).
    pub rssi: Option<i16>,
    /// Address of the advertising device.
    pub source_address: BleAddress,
}

/// An active BLE connection to a peer.
#[async_trait]
pub trait BleConnection: Send + Sync {
    /// Send data to the connected peer.
    async fn send(&self, data: &[u8]) -> Result<(), BleError>;

    /// Receive data from the connected peer.
    async fn recv(&self) -> Result<Vec<u8>, BleError>;

    /// Disconnect from the peer.
    async fn disconnect(&self) -> Result<(), BleError>;

    /// Get the current RSSI (if supported).
    fn rssi(&self) -> Option<i16>;

    /// Get the peer's BLE address.
    fn peer_address(&self) -> &BleAddress;

    /// Check whether the connection is still active.
    fn is_connected(&self) -> bool;
}

/// BLE central role: scanning for advertisements and connecting to peripherals.
#[async_trait]
pub trait BleCentral: Send + Sync {
    /// Start scanning for BLE advertisements.
    async fn start_scan(&self) -> Result<(), BleError>;

    /// Stop scanning.
    async fn stop_scan(&self) -> Result<(), BleError>;

    /// Subscribe to discovered advertisements.
    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement>;

    /// Connect to a peripheral at the given address.
    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError>;
}

/// BLE peripheral role: advertising and accepting incoming connections.
///
/// Note: the plan originally specified `incoming_connections() -> broadcast::Receiver<Box<dyn BleConnection>>`,
/// but `broadcast` requires `T: Clone` which `Box<dyn BleConnection>` cannot satisfy.
/// We use an `accept()` pattern instead, which is the standard server-side idiom.
#[async_trait]
pub trait BlePeripheral: Send + Sync {
    /// Start advertising with the given data and begin accepting connections.
    async fn start_advertising(&self, data: Vec<u8>) -> Result<(), BleError>;

    /// Stop advertising.
    async fn stop_advertising(&self) -> Result<(), BleError>;

    /// Update the advertisement data while continuing to advertise.
    async fn update_advertisement(&self, data: Vec<u8>) -> Result<(), BleError>;

    /// Accept the next incoming connection from a central.
    async fn accept(&self) -> Result<Box<dyn BleConnection>, BleError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ble_address_equality() {
        let addr1 = BleAddress::Real([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let addr2 = BleAddress::Real([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let addr3 = BleAddress::Real([0xFF, 0x02, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(addr1, addr2);
        assert_ne!(addr1, addr3);

        let sim1 = BleAddress::Simulated(Uuid::nil());
        let sim2 = BleAddress::Simulated(Uuid::nil());
        assert_eq!(sim1, sim2);

        // Real and Simulated are never equal
        assert_ne!(addr1, sim1);
    }

    #[test]
    fn test_ble_advertisement_clone() {
        let adv = BleAdvertisement {
            data: vec![0x01, 0x02, 0x03],
            rssi: Some(-42),
            source_address: BleAddress::Simulated(Uuid::new_v4()),
        };
        let cloned = adv.clone();
        assert_eq!(cloned.data, adv.data);
        assert_eq!(cloned.rssi, adv.rssi);
        assert_eq!(cloned.source_address, adv.source_address);
    }
}
