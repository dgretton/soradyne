//! In-process BLE simulator
//!
//! Provides a simulated BLE network where multiple devices can advertise,
//! scan, and form connections entirely in-process. Used for integration
//! testing without requiring real BLE hardware.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use super::BleError;

/// The simulated "air" — a shared medium through which all simulated
/// devices communicate.
pub struct SimBleNetwork {
    /// Broadcast channel for advertisements.
    adv_tx: broadcast::Sender<BleAdvertisement>,
    /// Registry of peripherals accepting connections.
    /// Maps peripheral address -> sender for delivering connection requests.
    peripherals: Arc<Mutex<HashMap<BleAddress, mpsc::Sender<Box<dyn BleConnection>>>>>,
}

impl SimBleNetwork {
    /// Create a new simulated BLE network.
    pub fn new() -> Arc<Self> {
        let (adv_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            adv_tx,
            peripherals: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create a new simulated BLE device on this network.
    pub fn create_device(self: &Arc<Self>) -> SimBleDevice {
        let (conn_tx, conn_rx) = mpsc::channel(16);
        let address = BleAddress::Simulated(Uuid::new_v4());
        SimBleDevice {
            address,
            network: Arc::clone(self),
            mtu: 247,
            latency: Duration::ZERO,
            conn_tx,
            conn_rx: Arc::new(Mutex::new(conn_rx)),
        }
    }
}

/// A simulated BLE device that can act as both central and peripheral.
pub struct SimBleDevice {
    address: BleAddress,
    network: Arc<SimBleNetwork>,
    mtu: usize,
    /// Simulated link-layer latency applied to each send().
    latency: Duration,
    /// Sender used to deliver incoming connections (registered with the network).
    conn_tx: mpsc::Sender<Box<dyn BleConnection>>,
    /// Receiver for incoming connections (peripheral role).
    conn_rx: Arc<Mutex<mpsc::Receiver<Box<dyn BleConnection>>>>,
}

impl SimBleDevice {
    /// Get this device's BLE address.
    pub fn address(&self) -> &BleAddress {
        &self.address
    }

    /// Set the MTU for connections created by this device.
    pub fn set_mtu(&mut self, mtu: usize) {
        self.mtu = mtu;
    }

    /// Set the simulated link-layer latency for connections created by this device.
    /// Each send() will sleep for this duration before transmitting.
    /// Uses tokio virtual time — paused clocks advance instantly in tests.
    pub fn set_latency(&mut self, latency: Duration) {
        self.latency = latency;
    }
}

/// A simulated BLE connection backed by tokio mpsc channels.
pub struct SimBleConnection {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    connected: Arc<AtomicBool>,
    peer_connected: Arc<AtomicBool>,
    peer_address: BleAddress,
    mtu: usize,
    latency: Duration,
}

impl SimBleConnection {
    /// Create a symmetric pair of connections between two addresses.
    fn create_pair(
        addr_a: BleAddress,
        addr_b: BleAddress,
        mtu: usize,
        latency: Duration,
    ) -> (SimBleConnection, SimBleConnection) {
        let (tx_ab, rx_ab) = mpsc::channel(64);
        let (tx_ba, rx_ba) = mpsc::channel(64);
        let connected_a = Arc::new(AtomicBool::new(true));
        let connected_b = Arc::new(AtomicBool::new(true));

        let conn_a = SimBleConnection {
            tx: tx_ab,
            rx: Arc::new(Mutex::new(rx_ba)),
            connected: Arc::clone(&connected_a),
            peer_connected: Arc::clone(&connected_b),
            peer_address: addr_b,
            mtu,
            latency,
        };

        let conn_b = SimBleConnection {
            tx: tx_ba,
            rx: Arc::new(Mutex::new(rx_ab)),
            connected: Arc::clone(&connected_b),
            peer_connected: Arc::clone(&connected_a),
            peer_address: addr_a,
            mtu,
            latency,
        };

        (conn_a, conn_b)
    }
}

#[async_trait]
impl BleConnection for SimBleConnection {
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        if !self.is_connected() {
            return Err(BleError::Disconnected);
        }
        if data.len() > self.mtu {
            return Err(BleError::MtuExceeded {
                size: data.len(),
                mtu: self.mtu,
            });
        }
        // Simulate BLE link-layer latency.
        if !self.latency.is_zero() {
            tokio::time::sleep(self.latency).await;
        }
        self.tx
            .send(data.to_vec())
            .await
            .map_err(|_| BleError::Disconnected)
    }

    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        if !self.is_connected() {
            return Err(BleError::Disconnected);
        }
        let mut rx = self.rx.lock().await;
        match rx.recv().await {
            Some(data) => Ok(data),
            None => Err(BleError::Disconnected),
        }
    }

    async fn disconnect(&self) -> Result<(), BleError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn rssi(&self) -> Option<i16> {
        Some(-50)
    }

    fn peer_address(&self) -> &BleAddress {
        &self.peer_address
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst) && self.peer_connected.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl BleCentral for SimBleDevice {
    async fn start_scan(&self) -> Result<(), BleError> {
        Ok(())
    }

    async fn stop_scan(&self) -> Result<(), BleError> {
        Ok(())
    }

    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement> {
        self.network.adv_tx.subscribe()
    }

    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError> {
        let peripherals = self.network.peripherals.lock().await;
        let conn_sender = peripherals.get(address).ok_or_else(|| {
            BleError::ConnectionError(format!("No peripheral at {:?}", address))
        })?;

        let (conn_central, conn_peripheral) = SimBleConnection::create_pair(
            self.address.clone(),
            address.clone(),
            self.mtu,
            self.latency,
        );

        conn_sender
            .send(Box::new(conn_peripheral))
            .await
            .map_err(|_| {
                BleError::ConnectionError(
                    "Peripheral is no longer accepting connections".to_string(),
                )
            })?;

        Ok(Box::new(conn_central))
    }
}

#[async_trait]
impl BlePeripheral for SimBleDevice {
    async fn start_advertising(&self, data: Vec<u8>) -> Result<(), BleError> {
        // Register this device as a connectable peripheral.
        {
            let mut peripherals = self.network.peripherals.lock().await;
            peripherals.insert(self.address.clone(), self.conn_tx.clone());
        }
        // Send the advertisement.
        let adv = BleAdvertisement {
            data,
            rssi: None,
            source_address: self.address.clone(),
        };
        let _ = self.network.adv_tx.send(adv);
        Ok(())
    }

    async fn stop_advertising(&self) -> Result<(), BleError> {
        let mut peripherals = self.network.peripherals.lock().await;
        peripherals.remove(&self.address);
        Ok(())
    }

    async fn update_advertisement(&self, data: Vec<u8>) -> Result<(), BleError> {
        let adv = BleAdvertisement {
            data,
            rssi: None,
            source_address: self.address.clone(),
        };
        let _ = self.network.adv_tx.send(adv);
        Ok(())
    }

    async fn accept(&self) -> Result<Box<dyn BleConnection>, BleError> {
        let mut rx = self.conn_rx.lock().await;
        rx.recv().await.ok_or(BleError::Disconnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_advertisement_broadcast() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();

        let mut rx = device_b.advertisements();

        device_a
            .start_advertising(vec![0x01, 0x02, 0x03])
            .await
            .unwrap();

        let adv = rx.recv().await.unwrap();
        assert_eq!(adv.data, vec![0x01, 0x02, 0x03]);
        assert_eq!(adv.source_address, *device_a.address());
    }

    #[tokio::test]
    async fn test_no_self_advertisement() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();

        let mut rx_a = device_a.advertisements();
        let mut rx_b = device_b.advertisements();

        device_a
            .start_advertising(vec![0xAA])
            .await
            .unwrap();

        // Device B sees the advertisement.
        let adv_b = rx_b.recv().await.unwrap();
        assert_eq!(adv_b.data, vec![0xAA]);

        // Device A also sees it on the broadcast channel (the "air" doesn't filter),
        // but the source_address lets the application layer filter self-advertisements.
        let adv_a = rx_a.recv().await.unwrap();
        assert_eq!(adv_a.source_address, *device_a.address());

        // Demonstrate the filtering pattern: application code would skip own ads.
        let is_self = adv_a.source_address == *device_a.address();
        assert!(is_self, "Application should filter advertisements from self");
    }

    #[tokio::test]
    async fn test_connection_establishment() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        // Device B starts advertising (registers as connectable peripheral).
        device_b
            .start_advertising(vec![0x01])
            .await
            .unwrap();

        // Spawn the peripheral accept in a separate task.
        let accept_handle = tokio::spawn(async move {
            device_b.accept().await.unwrap()
        });

        // Device A connects to device B.
        let conn_a = device_a.connect(&addr_b).await.unwrap();

        // Device B accepts the connection.
        let conn_b = accept_handle.await.unwrap();

        assert!(conn_a.is_connected());
        assert!(conn_b.is_connected());
    }

    #[tokio::test]
    async fn test_bidirectional_data_transfer() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b
            .start_advertising(vec![0x01])
            .await
            .unwrap();

        let accept_handle = tokio::spawn(async move {
            device_b.accept().await.unwrap()
        });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        // A -> B
        conn_a.send(b"hello from A").await.unwrap();
        let received = conn_b.recv().await.unwrap();
        assert_eq!(received, b"hello from A");

        // B -> A
        conn_b.send(b"hello from B").await.unwrap();
        let received = conn_a.recv().await.unwrap();
        assert_eq!(received, b"hello from B");
    }

    #[tokio::test]
    async fn test_disconnect() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b
            .start_advertising(vec![0x01])
            .await
            .unwrap();

        let accept_handle = tokio::spawn(async move {
            device_b.accept().await.unwrap()
        });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        assert!(conn_a.is_connected());
        assert!(conn_b.is_connected());

        // A disconnects.
        conn_a.disconnect().await.unwrap();

        // A sees itself as disconnected.
        assert!(!conn_a.is_connected());

        // B also sees the connection as dead (peer_connected flag).
        assert!(!conn_b.is_connected());

        // B's recv returns Disconnected since B checks is_connected first.
        let result = conn_b.recv().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mtu_enforcement() {
        let network = SimBleNetwork::new();
        let mut device_a = network.create_device();
        device_a.set_mtu(10); // Very small MTU for testing.
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b
            .start_advertising(vec![0x01])
            .await
            .unwrap();

        let accept_handle = tokio::spawn(async move {
            device_b.accept().await.unwrap()
        });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let _conn_b = accept_handle.await.unwrap();

        // Sending data within MTU succeeds.
        conn_a.send(&[0u8; 10]).await.unwrap();

        // Sending data exceeding MTU fails.
        let result = conn_a.send(&[0u8; 11]).await;
        assert!(matches!(result, Err(BleError::MtuExceeded { size: 11, mtu: 10 })));
    }

    #[tokio::test]
    async fn test_multiple_devices_on_network() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let device_c = network.create_device();

        let mut rx_b = device_b.advertisements();
        let mut rx_c = device_c.advertisements();

        // Device A advertises.
        device_a
            .start_advertising(vec![0xAA])
            .await
            .unwrap();

        // Both B and C receive it.
        let adv_b = rx_b.recv().await.unwrap();
        let adv_c = rx_c.recv().await.unwrap();
        assert_eq!(adv_b.data, vec![0xAA]);
        assert_eq!(adv_c.data, vec![0xAA]);
        assert_eq!(adv_b.source_address, *device_a.address());
        assert_eq!(adv_c.source_address, *device_a.address());
    }

    #[tokio::test(start_paused = true)]
    async fn test_latency_delays_send() {
        let network = SimBleNetwork::new();
        let mut device_a = network.create_device();
        device_a.set_latency(Duration::from_millis(100));
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();
        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        let before = tokio::time::Instant::now();
        conn_a.send(b"hello").await.unwrap();
        let elapsed = before.elapsed();

        // Virtual time should have advanced by the configured latency.
        assert!(elapsed >= Duration::from_millis(100));
        assert!(elapsed < Duration::from_millis(200));

        // Data still arrives correctly.
        let received = conn_b.recv().await.unwrap();
        assert_eq!(received, b"hello");
    }

    #[tokio::test(start_paused = true)]
    async fn test_default_zero_latency() {
        let network = SimBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();
        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let _conn_b = accept_handle.await.unwrap();

        let before = tokio::time::Instant::now();
        conn_a.send(b"instant").await.unwrap();
        let elapsed = before.elapsed();

        // Default latency is zero — no virtual time should pass.
        assert_eq!(elapsed, Duration::ZERO);
    }
}
