//! LAN transport — TCP-backed implementation of the BLE transport traits
//! with in-process service discovery.
//!
//! `LanBleNetwork` is the TCP analog of `SimBleNetwork`: it provides a shared
//! service registry that mimics mDNS semantics, while actual data flows over
//! real TCP sockets on localhost. This exercises real async I/O, framing, and
//! connection lifecycle while keeping discovery trivial.
//!
//! Each `LanBleDevice` implements both `BleCentral` and `BlePeripheral`:
//! - Advertising registers a TCP listener address + encrypted payload in the registry
//! - Scanning queries the registry and emits `BleAdvertisement`s
//! - Connections are real `TcpConnection`s from `tcp_transport.rs`
//!
//! Unlike `SimBleNetwork`, connections here traverse real kernel TCP stacks,
//! so framing correctness, backpressure, and disconnection semantics are
//! exercised at the OS level.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use super::BleError;

/// Shared service registry — the LAN equivalent of "the air" in SimBleNetwork.
///
/// Devices register their TCP listener address and advertisement payload here.
/// Scanners receive advertisements through the broadcast channel, and connect
/// by looking up the TCP address in the registry.
pub struct LanBleNetwork {
    /// Broadcast channel for advertisements (mirrors SimBleNetwork.adv_tx).
    adv_tx: broadcast::Sender<BleAdvertisement>,
    /// Registry of advertising devices: BleAddress → TCP listener SocketAddr.
    /// When a central calls connect(BleAddress), we look up the real TCP address here.
    listeners: Arc<Mutex<Vec<(BleAddress, SocketAddr)>>>,
}

impl LanBleNetwork {
    /// Create a new LAN transport network.
    pub fn new() -> Arc<Self> {
        let (adv_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            adv_tx,
            listeners: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Create a new device on this network.
    pub fn create_device(self: &Arc<Self>) -> LanBleDevice {
        let address = BleAddress::Simulated(Uuid::new_v4());
        let (conn_tx, conn_rx) = mpsc::channel(16);
        LanBleDevice {
            address,
            network: Arc::clone(self),
            conn_tx,
            conn_rx: Arc::new(Mutex::new(conn_rx)),
            local_addr: Arc::new(Mutex::new(None)),
        }
    }

    /// Register a device's TCP listener address in the service registry.
    async fn register(&self, ble_addr: BleAddress, tcp_addr: SocketAddr) {
        let mut listeners = self.listeners.lock().await;
        listeners.push((ble_addr, tcp_addr));
    }

    /// Remove a device from the service registry.
    async fn unregister(&self, ble_addr: &BleAddress) {
        let mut listeners = self.listeners.lock().await;
        listeners.retain(|(addr, _)| addr != ble_addr);
    }

    /// Look up the TCP address for a BLE address.
    async fn resolve(&self, ble_addr: &BleAddress) -> Option<SocketAddr> {
        let listeners = self.listeners.lock().await;
        listeners
            .iter()
            .find(|(addr, _)| addr == ble_addr)
            .map(|(_, tcp_addr)| *tcp_addr)
    }
}

/// A LAN-backed device that implements both `BleCentral` and `BlePeripheral`
/// using real TCP sockets with an in-process service registry for discovery.
pub struct LanBleDevice {
    /// This device's logical BLE address (used for advertisement source_address).
    address: BleAddress,
    /// Reference to the shared network (service registry + advertisement channel).
    network: Arc<LanBleNetwork>,
    /// Sender for delivering accepted connections to the `accept()` caller.
    conn_tx: mpsc::Sender<Box<dyn BleConnection>>,
    /// Receiver for accepted connections.
    conn_rx: Arc<Mutex<mpsc::Receiver<Box<dyn BleConnection>>>>,
    /// The bound TCP listener address (set after start_advertising).
    local_addr: Arc<Mutex<Option<SocketAddr>>>,
}

impl LanBleDevice {
    /// Get this device's logical BLE address.
    pub fn address(&self) -> &BleAddress {
        &self.address
    }
}

#[async_trait]
impl BleCentral for LanBleDevice {
    async fn start_scan(&self) -> Result<(), BleError> {
        Ok(()) // Discovery happens via the broadcast channel
    }

    async fn stop_scan(&self) -> Result<(), BleError> {
        Ok(())
    }

    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement> {
        self.network.adv_tx.subscribe()
    }

    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError> {
        // Resolve the logical BLE address to a real TCP address via the registry
        let tcp_addr = self
            .network
            .resolve(address)
            .await
            .ok_or_else(|| {
                BleError::ConnectionError(format!(
                    "No LAN device registered at {:?}",
                    address
                ))
            })?;

        let stream = tokio::net::TcpStream::connect(tcp_addr)
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

        // Use TcpConnection from tcp_transport for the actual connection.
        // We need to construct it here since tcp_transport is behind a feature flag.
        // Instead, we build a LanConnection that does the same thing.
        Ok(Box::new(LanConnection::from_stream(stream, address.clone())))
    }
}

#[async_trait]
impl BlePeripheral for LanBleDevice {
    async fn start_advertising(&self, data: Vec<u8>) -> Result<(), BleError> {
        // Bind a TCP listener on localhost with an OS-assigned port
        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

        let actual_addr = listener
            .local_addr()
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

        *self.local_addr.lock().await = Some(actual_addr);

        // Register in the service registry so centrals can find us
        self.network
            .register(self.address.clone(), actual_addr)
            .await;

        // Broadcast the advertisement
        let adv = BleAdvertisement {
            data,
            rssi: None,
            source_address: self.address.clone(),
        };
        let _ = self.network.adv_tx.send(adv);

        // Spawn accept loop — accepted TCP streams become LanConnections
        let conn_tx = self.conn_tx.clone();
        let ble_addr = self.address.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _peer_addr)) => {
                        let conn = LanConnection::from_stream(stream, ble_addr.clone());
                        if conn_tx
                            .send(Box::new(conn) as Box<dyn BleConnection>)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(())
    }

    async fn stop_advertising(&self) -> Result<(), BleError> {
        self.network.unregister(&self.address).await;
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
        self.conn_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(BleError::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// LanConnection — TCP connection with BLE address identity
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;

use super::framing::{build_frame, FrameReassembler};

/// A TCP connection that presents as a `BleConnection`.
///
/// Identical in structure to `TcpConnection` in `tcp_transport.rs`, but
/// unconditionally compiled (no feature flag) and uses the caller-provided
/// `BleAddress` instead of `BleAddress::Tcp`.
pub struct LanConnection {
    write_tx: TokioMutex<Option<mpsc::Sender<Vec<u8>>>>,
    msg_rx: TokioMutex<mpsc::Receiver<Vec<u8>>>,
    connected: Arc<AtomicBool>,
    peer_addr: BleAddress,
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    writer_handle: StdMutex<Option<JoinHandle<()>>>,
}

impl LanConnection {
    /// Create a `LanConnection` from an established TCP stream.
    ///
    /// The `peer_ble_addr` is the logical BLE address of the peer (used for
    /// `peer_address()`), not the TCP socket address.
    pub fn from_stream(
        stream: tokio::net::TcpStream,
        peer_ble_addr: BleAddress,
    ) -> Self {
        let _ = stream.set_nodelay(true);
        let (reader, writer) = tokio::io::split(stream);
        let connected = Arc::new(AtomicBool::new(true));

        let (msg_tx, msg_rx) = mpsc::channel::<Vec<u8>>(64);
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(64);

        let reader_handle = {
            let connected = Arc::clone(&connected);
            tokio::spawn(async move {
                let mut reader = reader;
                let mut reassembler = FrameReassembler::new();
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            reassembler.push(&buf[..n]);
                            while let Some(msg) = reassembler.try_extract() {
                                if msg_tx.send(msg).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                connected.store(false, Ordering::SeqCst);
            })
        };

        let writer_handle = {
            let connected = Arc::clone(&connected);
            tokio::spawn(async move {
                let mut writer = writer;
                while let Some(data) = write_rx.recv().await {
                    if writer.write_all(&data).await.is_err() {
                        break;
                    }
                    if writer.flush().await.is_err() {
                        break;
                    }
                }
                connected.store(false, Ordering::SeqCst);
            })
        };

        Self {
            write_tx: TokioMutex::new(Some(write_tx)),
            msg_rx: TokioMutex::new(msg_rx),
            connected,
            peer_addr: peer_ble_addr,
            reader_handle: StdMutex::new(Some(reader_handle)),
            writer_handle: StdMutex::new(Some(writer_handle)),
        }
    }

    fn abort_tasks(&self) {
        if let Ok(mut h) = self.reader_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        if let Ok(mut h) = self.writer_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
    }
}

impl Drop for LanConnection {
    fn drop(&mut self) {
        self.abort_tasks();
    }
}

#[async_trait]
impl BleConnection for LanConnection {
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        if !self.is_connected() {
            return Err(BleError::Disconnected);
        }
        let frame = build_frame(data);
        let guard = self.write_tx.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.send(frame).await.map_err(|_| BleError::Disconnected),
            None => Err(BleError::Disconnected),
        }
    }

    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        let mut rx = self.msg_rx.lock().await;
        rx.recv().await.ok_or(BleError::Disconnected)
    }

    async fn disconnect(&self) -> Result<(), BleError> {
        self.connected.store(false, Ordering::SeqCst);
        *self.write_tx.lock().await = None;
        self.abort_tasks();
        Ok(())
    }

    fn rssi(&self) -> Option<i16> {
        None
    }

    fn peer_address(&self) -> &BleAddress {
        &self.peer_addr
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // Basic LanBleNetwork tests (mirror SimBleNetwork tests)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_advertisement_broadcast() {
        let network = LanBleNetwork::new();
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
    async fn test_connection_establishment() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();

        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        assert!(conn_a.is_connected());
        assert!(conn_b.is_connected());
    }

    #[tokio::test]
    async fn test_bidirectional_data_transfer() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();

        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

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
    async fn test_disconnect_detection() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();

        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        conn_a.disconnect().await.unwrap();
        assert!(!conn_a.is_connected());

        // Give the reader task time to notice EOF
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = conn_b.recv().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_large_message() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();

        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        // 100KB — exercises TCP framing reassembly
        let large_data = vec![0xABu8; 100_000];
        conn_a.send(&large_data).await.unwrap();
        let received = conn_b.recv().await.unwrap();
        assert_eq!(received.len(), 100_000);
        assert_eq!(received, large_data);
    }

    #[tokio::test]
    async fn test_rapid_sequential_messages() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();

        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        for i in 0u32..100 {
            let msg = format!("msg-{}", i);
            conn_a.send(msg.as_bytes()).await.unwrap();
        }

        for i in 0u32..100 {
            let expected = format!("msg-{}", i);
            let received = conn_b.recv().await.unwrap();
            assert_eq!(received, expected.as_bytes(), "mismatch at message {}", i);
        }
    }

    #[tokio::test]
    async fn test_multiple_devices() {
        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let device_c = network.create_device();

        let mut rx_b = device_b.advertisements();
        let mut rx_c = device_c.advertisements();

        device_a
            .start_advertising(vec![0xAA])
            .await
            .unwrap();

        let adv_b = rx_b.recv().await.unwrap();
        let adv_c = rx_c.recv().await.unwrap();
        assert_eq!(adv_b.data, vec![0xAA]);
        assert_eq!(adv_c.data, vec![0xAA]);
    }

    // -----------------------------------------------------------------------
    // Topology integration: messenger over LAN TCP
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_messenger_over_lan_tcp() {
        use crate::ble::gatt::MessageType;
        use crate::topology::ensemble::{
            ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
        };
        use crate::topology::messenger::TopologyMessenger;
        use tokio::sync::RwLock;

        let network = LanBleNetwork::new();
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![]).await.unwrap();
        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let make_topo = || {
            let mut t = EnsembleTopology::new();
            t.add_edge(TopologyEdge {
                from: id_a,
                to: id_b,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_b,
                to: id_a,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, make_topo());
        let messenger_b = TopologyMessenger::new(id_b, make_topo());

        let mut rx_b = messenger_b.incoming();

        messenger_a
            .add_connection(id_b, Arc::from(conn_a))
            .await;
        messenger_b
            .add_connection(id_a, Arc::from(conn_b))
            .await;

        messenger_a
            .send_to(id_b, MessageType::FlowSync, b"lan-hello")
            .await
            .unwrap();

        let envelope = tokio::time::timeout(Duration::from_secs(2), rx_b.recv())
            .await
            .expect("should receive within timeout")
            .expect("recv should succeed");

        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.destination, id_b);
        assert_eq!(envelope.payload, b"lan-hello");
    }

    /// Multi-hop forwarding over LAN TCP: A→B→C.
    #[tokio::test]
    async fn test_multi_hop_over_lan_tcp() {
        use crate::ble::gatt::MessageType;
        use crate::topology::ensemble::{
            ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
        };
        use crate::topology::messenger::TopologyMessenger;
        use tokio::sync::RwLock;

        let network = LanBleNetwork::new();

        // Create three devices and two connections: A↔B, B↔C
        let device_a_central = network.create_device();
        let device_b_periph_a = network.create_device();
        let addr_b_a = device_b_periph_a.address().clone();

        let device_b_central = network.create_device();
        let device_c_periph = network.create_device();
        let addr_c = device_c_periph.address().clone();

        // A↔B connection
        device_b_periph_a.start_advertising(vec![]).await.unwrap();
        let accept_ab = tokio::spawn(async move { device_b_periph_a.accept().await.unwrap() });
        let conn_ab_a = device_a_central.connect(&addr_b_a).await.unwrap();
        let conn_ab_b = accept_ab.await.unwrap();

        // B↔C connection
        device_c_periph.start_advertising(vec![]).await.unwrap();
        let accept_bc = tokio::spawn(async move { device_c_periph.accept().await.unwrap() });
        let conn_bc_b = device_b_central.connect(&addr_c).await.unwrap();
        let conn_bc_c = accept_bc.await.unwrap();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();

        let make_topo = || {
            let mut t = EnsembleTopology::new();
            t.add_edge(TopologyEdge {
                from: id_a,
                to: id_b,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_b,
                to: id_a,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_b,
                to: id_c,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_c,
                to: id_b,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, make_topo());
        let messenger_b = TopologyMessenger::new(id_b, make_topo());
        let messenger_c = TopologyMessenger::new(id_c, make_topo());

        let mut rx_c = messenger_c.incoming();

        messenger_a.add_connection(id_b, Arc::from(conn_ab_a)).await;
        messenger_b.add_connection(id_a, Arc::from(conn_ab_b)).await;
        messenger_b.add_connection(id_c, Arc::from(conn_bc_b)).await;
        messenger_c.add_connection(id_b, Arc::from(conn_bc_c)).await;

        // Send from A to C (must hop through B)
        messenger_a
            .send_to(id_c, MessageType::FlowSync, b"lan-multi-hop")
            .await
            .unwrap();

        let envelope = tokio::time::timeout(Duration::from_secs(2), rx_c.recv())
            .await
            .expect("C should receive within timeout")
            .expect("C recv should succeed");

        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.destination, id_c);
        assert_eq!(envelope.payload, b"lan-multi-hop");
    }

    /// Mixed transport: A↔B over SimBLE, B↔C over LAN TCP.
    /// Messages from A reach C through the relay B, crossing transport boundaries.
    #[tokio::test]
    async fn test_mixed_sim_and_lan_transport() {
        use crate::ble::gatt::MessageType;
        use crate::ble::simulated::SimBleNetwork;
        use crate::topology::ensemble::{
            ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
        };
        use crate::topology::messenger::TopologyMessenger;
        use tokio::sync::RwLock;

        let sim_network = SimBleNetwork::new();
        let lan_network = LanBleNetwork::new();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();

        // A↔B: SimBLE connection
        let mut sim_device_a = sim_network.create_device();
        sim_device_a.set_mtu(4096);
        let sim_device_b = sim_network.create_device();
        let sim_addr_b = sim_device_b.address().clone();

        sim_device_b.start_advertising(vec![]).await.unwrap();
        let accept_sim = tokio::spawn(async move { sim_device_b.accept().await.unwrap() });
        let conn_ab_a = sim_device_a.connect(&sim_addr_b).await.unwrap();
        let conn_ab_b = accept_sim.await.unwrap();

        // B↔C: LAN TCP connection
        let lan_device_b = lan_network.create_device();
        let lan_device_c = lan_network.create_device();
        let lan_addr_c = lan_device_c.address().clone();

        lan_device_c.start_advertising(vec![]).await.unwrap();
        let accept_lan = tokio::spawn(async move { lan_device_c.accept().await.unwrap() });
        let conn_bc_b = lan_device_b.connect(&lan_addr_c).await.unwrap();
        let conn_bc_c = accept_lan.await.unwrap();

        let make_topo = || {
            let mut t = EnsembleTopology::new();
            t.add_edge(TopologyEdge {
                from: id_a,
                to: id_b,
                transport: TransportType::SimulatedBle,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_b,
                to: id_a,
                transport: TransportType::SimulatedBle,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_b,
                to: id_c,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            t.add_edge(TopologyEdge {
                from: id_c,
                to: id_b,
                transport: TransportType::TcpDirect,
                quality: ConnectionQuality::unknown(),
            });
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, make_topo());
        let messenger_b = TopologyMessenger::new(id_b, make_topo());
        let messenger_c = TopologyMessenger::new(id_c, make_topo());

        let mut rx_c = messenger_c.incoming();

        messenger_a.add_connection(id_b, Arc::from(conn_ab_a)).await;
        messenger_b.add_connection(id_a, Arc::from(conn_ab_b)).await;
        messenger_b.add_connection(id_c, Arc::from(conn_bc_b)).await;
        messenger_c.add_connection(id_b, Arc::from(conn_bc_c)).await;

        // A sends to C — hops from SimBLE (A→B) to LAN TCP (B→C)
        messenger_a
            .send_to(id_c, MessageType::FlowSync, b"cross-transport")
            .await
            .unwrap();

        let envelope = tokio::time::timeout(Duration::from_secs(2), rx_c.recv())
            .await
            .expect("C should receive within timeout")
            .expect("C recv should succeed");

        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.destination, id_c);
        assert_eq!(envelope.payload, b"cross-transport");
    }
}
