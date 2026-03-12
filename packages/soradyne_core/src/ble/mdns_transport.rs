//! mDNS/DNS-SD transport — LAN service discovery with TCP data transport.
//!
//! Tier 2 in the transport tier architecture. Uses multicast DNS (RFC 6762)
//! with DNS-SD (RFC 6763) for zero-configuration service discovery on the
//! local network. Data flows over TCP connections (reusing `LanConnection`
//! from `lan_transport.rs`).
//!
//! Each `MdnsBleDevice` implements both `BleCentral` and `BlePeripheral`:
//! - Advertising registers a `_rim._tcp.local.` mDNS service with the
//!   encrypted advertisement payload in a TXT record
//! - Scanning browses for `_rim._tcp.local.` services and emits
//!   `BleAdvertisement`s from resolved service entries
//! - Connections are real TCP connections via `LanConnection`
//!
//! The encrypted advertisement payload (capsule hint + encrypted piece hint +
//! topology hash) goes into the mDNS TXT record byte-for-byte. The same
//! `try_decrypt_advertisement` logic works unchanged — an eavesdropper on
//! the LAN sees the same opaque bytes as they would from a BLE advertisement.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

use super::lan_transport::LanConnection;
use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use super::BleError;

/// The mDNS service type for rim peer discovery.
const SERVICE_TYPE: &str = "_rim._tcp.local.";

/// TXT record key for the encrypted advertisement payload.
const TXT_ADV_KEY: &str = "adv";

/// Shared mDNS daemon — one per process. Manages service registration,
/// browsing, and multicast socket I/O on a background thread.
pub struct MdnsTransport {
    daemon: ServiceDaemon,
}

impl MdnsTransport {
    /// Create a new mDNS transport.
    ///
    /// `enable_loopback`: if true, enables the loopback interface and
    /// multicast loopback so devices on the same machine can discover
    /// each other (useful for same-host testing or multi-process scenarios).
    pub fn new(enable_loopback: bool) -> Result<Arc<Self>, BleError> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| BleError::ConnectionError(format!("mDNS daemon: {e}")))?;

        if enable_loopback {
            daemon
                .enable_interface(IfKind::LoopbackV4)
                .map_err(|e| BleError::ConnectionError(format!("enable loopback: {e}")))?;
            daemon
                .set_multicast_loop_v4(true)
                .map_err(|e| BleError::ConnectionError(format!("multicast loop: {e}")))?;
        }

        Ok(Arc::new(Self { daemon }))
    }

    /// Create a new device that uses this mDNS transport for discovery.
    pub fn create_device(self: &Arc<Self>) -> MdnsBleDevice {
        let (conn_tx, conn_rx) = mpsc::channel(16);
        let (adv_tx, _) = broadcast::channel(256);
        MdnsBleDevice {
            device_id: Uuid::new_v4(),
            transport: Arc::clone(self),
            conn_tx,
            conn_rx: Arc::new(Mutex::new(conn_rx)),
            adv_tx,
            local_addr: Arc::new(Mutex::new(None)),
            service_fullname: Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for MdnsTransport {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

/// An mDNS-backed device implementing both `BleCentral` and `BlePeripheral`.
///
/// Uses real mDNS multicast for discovery and real TCP for data transport.
pub struct MdnsBleDevice {
    /// Unique identifier for this device instance.
    device_id: Uuid,
    /// Reference to the shared mDNS daemon.
    transport: Arc<MdnsTransport>,
    /// Sender for delivering accepted TCP connections.
    conn_tx: mpsc::Sender<Box<dyn BleConnection>>,
    /// Receiver for accepted connections (peripheral role).
    conn_rx: Arc<Mutex<mpsc::Receiver<Box<dyn BleConnection>>>>,
    /// Broadcast channel for discovered advertisements.
    adv_tx: broadcast::Sender<BleAdvertisement>,
    /// The bound TCP listener address (set after start_advertising).
    local_addr: Arc<Mutex<Option<SocketAddr>>>,
    /// The registered mDNS service fullname (for unregistration).
    service_fullname: Arc<Mutex<Option<String>>>,
}

impl MdnsBleDevice {
    /// Get this device's UUID (used as the mDNS instance name).
    pub fn device_id(&self) -> Uuid {
        self.device_id
    }
}

#[async_trait]
impl BleCentral for MdnsBleDevice {
    async fn start_scan(&self) -> Result<(), BleError> {
        let receiver = self
            .transport
            .daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| BleError::ScanError(format!("mDNS browse: {e}")))?;

        let adv_tx = self.adv_tx.clone();
        let own_id = self.device_id;

        // Bridge mDNS events to the BleAdvertisement broadcast channel.
        // We use spawn_blocking + recv_timeout because flume's recv_async
        // doesn't reliably wake inside a tokio runtime on all platforms.
        tokio::task::spawn_blocking(move || {
            let timeout = std::time::Duration::from_secs(60);
            loop {
                match receiver.recv_timeout(timeout) {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        // Skip our own service
                        if info.get_fullname().contains(&own_id.to_string()) {
                            continue;
                        }

                        // Extract the encrypted advertisement payload from TXT
                        let adv_data = match info.get_property_val(TXT_ADV_KEY) {
                            Some(Some(data)) => data.to_vec(),
                            _ => continue,
                        };

                        // Resolve to a TCP address
                        let port = info.get_port();
                        let addr = pick_address(info.get_addresses());
                        let Some(ip) = addr else { continue };
                        let socket_addr = SocketAddr::new(ip, port);

                        let adv = BleAdvertisement {
                            data: adv_data,
                            rssi: None,
                            source_address: BleAddress::Tcp(socket_addr),
                        };
                        if adv_tx.send(adv).is_err() {
                            break; // No receivers left
                        }
                    }
                    Ok(ServiceEvent::SearchStopped(_)) => break,
                    Ok(_) => {} // Ignore SearchStarted, ServiceFound, ServiceRemoved
                    Err(_) => break, // Timeout or disconnected
                }
            }
        });

        Ok(())
    }

    async fn stop_scan(&self) -> Result<(), BleError> {
        let _ = self.transport.daemon.stop_browse(SERVICE_TYPE);
        Ok(())
    }

    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement> {
        self.adv_tx.subscribe()
    }

    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError> {
        let tcp_addr = match address {
            BleAddress::Tcp(addr) => *addr,
            other => {
                return Err(BleError::ConnectionError(format!(
                    "MdnsBleDevice cannot connect to non-TCP address: {:?}",
                    other
                )))
            }
        };

        let stream = tokio::net::TcpStream::connect(tcp_addr)
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

        Ok(Box::new(LanConnection::from_stream(
            stream,
            address.clone(),
        )))
    }
}

#[async_trait]
impl BlePeripheral for MdnsBleDevice {
    async fn start_advertising(&self, data: Vec<u8>) -> Result<(), BleError> {
        // Bind TCP listener on all interfaces with OS-assigned port
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
            .await
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

        let actual_addr = listener
            .local_addr()
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

        *self.local_addr.lock().await = Some(actual_addr);

        // Register the mDNS service with explicit address(es)
        let instance_name = self.device_id.to_string();
        let host_name = format!("{}.local.", instance_name);
        let properties = vec![mdns_sd::TxtProperty::from((TXT_ADV_KEY, data.as_slice()))];

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "", // addresses auto-detected by enable_addr_auto()
            actual_addr.port(),
            properties,
        )
        .map_err(|e| BleError::AdvertisingError(format!("ServiceInfo: {e}")))?
        .enable_addr_auto();

        let fullname = service.get_fullname().to_string();
        *self.service_fullname.lock().await = Some(fullname);

        self.transport
            .daemon
            .register(service)
            .map_err(|e| BleError::AdvertisingError(format!("mDNS register: {e}")))?;

        // Spawn TCP accept loop
        let conn_tx = self.conn_tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let conn = LanConnection::from_stream(
                            stream,
                            BleAddress::Tcp(peer_addr),
                        );
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
        if let Some(fullname) = self.service_fullname.lock().await.take() {
            let _ = self.transport.daemon.unregister(&fullname);
        }
        Ok(())
    }

    async fn update_advertisement(&self, data: Vec<u8>) -> Result<(), BleError> {
        let local_addr = self.local_addr.lock().await;
        let Some(addr) = *local_addr else {
            return Err(BleError::AdvertisingError("not advertising".into()));
        };

        let instance_name = self.device_id.to_string();
        let host_name = format!("{}.local.", instance_name);
        let properties = vec![mdns_sd::TxtProperty::from((TXT_ADV_KEY, data.as_slice()))];

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            addr.port(),
            properties,
        )
        .map_err(|e| BleError::AdvertisingError(format!("ServiceInfo: {e}")))?
        .enable_addr_auto();

        self.transport
            .daemon
            .register(service)
            .map_err(|e| BleError::AdvertisingError(format!("mDNS re-register: {e}")))?;

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

/// Pick the best IP address from a set of resolved addresses.
/// Prefers IPv4 loopback, then any IPv4, then anything.
fn pick_address(addrs: &std::collections::HashSet<mdns_sd::ScopedIp>) -> Option<IpAddr> {
    let ips: Vec<IpAddr> = addrs.iter().map(|s| s.to_ip_addr()).collect();
    // Prefer IPv4 loopback for localhost testing
    if let Some(addr) = ips.iter().find(|a| a.is_loopback() && a.is_ipv4()) {
        return Some(*addr);
    }
    // Then any IPv4
    if let Some(addr) = ips.iter().find(|a| a.is_ipv4()) {
        return Some(*addr);
    }
    // Then any address
    ips.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Minimal test: does the mdns-sd daemon work at all on this machine?
    /// Uses blocking recv_timeout on a spawned blocking task, just like
    /// the mdns-sd crate's own integration tests.
    #[tokio::test]
    async fn test_mdns_raw_daemon_discovery() {
        let daemon = ServiceDaemon::new().expect("daemon");

        // Register a service with explicit host IP
        let instance_name = format!("test-{}", Uuid::new_v4());
        let host_name = format!("{}.local.", instance_name);

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            9999u16,
            vec![mdns_sd::TxtProperty::from(("k", b"v" as &[u8]))],
        )
        .unwrap()
        .enable_addr_auto();

        daemon.register(service).unwrap();

        // Browse using blocking recv_timeout in a spawn_blocking
        let browse_rx = daemon.browse(SERVICE_TYPE).unwrap();
        let search_name = instance_name.clone();

        let found = tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_secs(5);
            while let Ok(event) = browse_rx.recv_timeout(timeout) {
                if let ServiceEvent::ServiceResolved(info) = event {
                    eprintln!("Resolved: {}", info.get_fullname());
                    if info.get_fullname().contains(&search_name) {
                        return true;
                    }
                }
            }
            false
        })
        .await
        .unwrap();

        assert!(found, "should discover our own service via mDNS");

        daemon.shutdown().unwrap();
    }

    /// Full lifecycle test: mDNS discovery → TCP connect → data exchange → messenger.
    /// Uses a single daemon to avoid port 5353 contention.
    #[tokio::test]
    async fn test_mdns_discovery_connect_and_messenger() {
        use crate::ble::gatt::MessageType;
        use crate::topology::ensemble::{
            ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
        };
        use crate::topology::messenger::TopologyMessenger;
        use tokio::sync::RwLock;

        let transport =
            MdnsTransport::new(false).expect("mDNS daemon should start");

        // --- Phase 1: Service registration and discovery ---
        let device_a = transport.create_device();
        let device_b = transport.create_device();

        // start_scan uses spawn_blocking internally for the browse bridge
        device_b.start_scan().await.unwrap();
        let mut rx = device_b.advertisements();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let adv_payload = vec![0xCA, 0xFE, 0xBA, 0xBE];
        device_a
            .start_advertising(adv_payload.clone())
            .await
            .unwrap();

        let adv = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("should discover service within 10s")
            .expect("recv should succeed");

        assert_eq!(adv.data, adv_payload, "Phase 1: advertisement payload matches");
        assert!(
            matches!(adv.source_address, BleAddress::Tcp(_)),
            "source should be a TCP address"
        );

        // --- Phase 2: Connect and exchange data ---
        let accept_handle =
            tokio::spawn(async move { device_a.accept().await.unwrap() });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let conn_b = device_b.connect(&adv.source_address).await.unwrap();
        let conn_a = accept_handle.await.unwrap();

        conn_b.send(b"hello via mDNS").await.unwrap();
        let received = conn_a.recv().await.unwrap();
        assert_eq!(received, b"hello via mDNS", "Phase 2: data exchange works");

        conn_a.send(b"reply via mDNS").await.unwrap();
        let received = conn_b.recv().await.unwrap();
        assert_eq!(received, b"reply via mDNS");

        // --- Phase 3: Wire into TopologyMessenger ---
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
        let mut msg_rx = messenger_b.incoming();

        messenger_a.add_connection(id_b, Arc::from(conn_a)).await;
        messenger_b.add_connection(id_a, Arc::from(conn_b)).await;

        messenger_a
            .send_to(id_b, MessageType::FlowSync, b"mdns-sync")
            .await
            .unwrap();

        let envelope =
            tokio::time::timeout(Duration::from_secs(2), msg_rx.recv())
                .await
                .expect("Phase 3: should receive within timeout")
                .expect("recv should succeed");

        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.payload, b"mdns-sync");
    }
}
