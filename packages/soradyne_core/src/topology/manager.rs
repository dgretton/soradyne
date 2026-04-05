//! EnsembleManager — discovery loop that ties it all together
//!
//! Manages the lifecycle of an ensemble: BLE advertisement/scanning,
//! connection establishment, topology synchronization, peer introduction
//! propagation, and stale piece detection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::ble::encrypted_adv::{
    capsule_hint_for, encrypt_advertisement, piece_hint_for, try_decrypt_advertisement,
    AdvertisementPayload,
};
use crate::ble::gatt::{MessageType, RoutedEnvelope};
use crate::ble::session::{
    establish_initiator, establish_responder, session_psk, ResponderIdentity, SessionIdentity,
};
use crate::ble::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use crate::identity::{CapsuleKeyBundle, DeviceIdentity};
use crate::topology::capsule::PieceCapabilities;
use crate::topology::ensemble::{
    ConnectionQuality, EnsembleTopology, PiecePresence, PieceReachability, TopologyEdge,
    TransportType,
};
use crate::topology::messenger::TopologyMessenger;
use crate::topology::sync::{PeerInfo, TopologySyncMessage};

/// Configuration for the ensemble discovery loop.
#[derive(Clone, Debug)]
pub struct EnsembleConfig {
    /// How often to re-advertise and check for stale pieces.
    pub scan_interval: Duration,
    /// How long before a piece is considered stale and removed.
    pub stale_timeout: Duration,
    /// Static peer addresses for when BLE/mDNS discovery isn't available
    /// (e.g. peers reachable over Tailscale or other VPN).
    /// Keyed by peer device UUID → TCP socket address.
    /// The ensemble manager will establish TCP connections to these peers
    /// automatically, using the UUID tiebreaker for role assignment, and
    /// reconnect on disconnect.
    pub static_peers: HashMap<Uuid, std::net::SocketAddr>,
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(2),
            stale_timeout: Duration::from_secs(30),
            static_peers: HashMap::new(),
        }
    }
}

/// Manages ensemble lifecycle: discovery, connection, topology sync,
/// and peer introduction propagation.
pub struct EnsembleManager {
    device_id: Uuid,
    device_identity: Arc<DeviceIdentity>,
    capsule_keys: CapsuleKeyBundle,
    known_capsules: Vec<CapsuleKeyBundle>,
    /// piece_hint -> device_id (computed from capsule piece list).
    piece_hints: RwLock<HashMap<[u8; 4], Uuid>>,
    /// Peer static X25519 public keys, keyed by device UUID.
    /// Used for Noise IKpsk2 session establishment.
    peer_static_keys: HashMap<Uuid, [u8; 32]>,
    topology: Arc<RwLock<EnsembleTopology>>,
    messenger: Arc<TopologyMessenger>,
    /// BLE addresses learned from advertisements/introductions.
    peer_addresses: Arc<RwLock<HashMap<Uuid, BleAddress>>>,
    adv_seq: AtomicU32,
    config: EnsembleConfig,
    shutdown_tx: broadcast::Sender<()>,
}

impl EnsembleManager {
    /// Create a new EnsembleManager.
    ///
    /// `device_identity` — this device's cryptographic identity (X25519 + Ed25519).
    /// `peer_static_keys` — map from peer device UUID to their X25519 public key
    ///                       (from `PieceRecord.dh_public_key`, learned during pairing).
    /// `known_piece_ids` — device_ids of all pieces in the capsule (including our own),
    ///                      used to resolve piece_hints from encrypted advertisements.
    pub fn new(
        device_identity: Arc<DeviceIdentity>,
        capsule_keys: CapsuleKeyBundle,
        known_piece_ids: Vec<Uuid>,
        peer_static_keys: HashMap<Uuid, [u8; 32]>,
        config: EnsembleConfig,
    ) -> Arc<Self> {
        let device_id = device_identity.device_id();
        let topology = Arc::new(RwLock::new(EnsembleTopology::new()));
        let messenger = TopologyMessenger::new(device_id, Arc::clone(&topology));

        let mut hints = HashMap::new();
        for id in &known_piece_ids {
            hints.insert(piece_hint_for(id), *id);
        }

        let (shutdown_tx, _) = broadcast::channel(1);

        Arc::new(Self {
            device_id,
            device_identity,
            capsule_keys: capsule_keys.clone(),
            known_capsules: vec![capsule_keys],
            piece_hints: RwLock::new(hints),
            peer_static_keys,
            topology,
            messenger,
            peer_addresses: Arc::new(RwLock::new(HashMap::new())),
            adv_seq: AtomicU32::new(0),
            config,
            shutdown_tx,
        })
    }

    pub fn messenger(&self) -> &Arc<TopologyMessenger> {
        &self.messenger
    }

    pub fn topology(&self) -> &Arc<RwLock<EnsembleTopology>> {
        &self.topology
    }

    pub fn device_id(&self) -> Uuid {
        self.device_id
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Start processing incoming TopologySync messages from the messenger.
    /// Call this before establishing connections so messages aren't missed.
    pub fn start_processing(self: &Arc<Self>) {
        let rx = self.messenger.incoming();
        let manager = Arc::clone(self);
        let mut shutdown = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut rx = rx;
            loop {
                tokio::select! {
                    result = rx.recv() => {
                        if let Ok(envelope) = result {
                            if envelope.message_type == MessageType::TopologySync {
                                manager.process_topology_message(&envelope).await;
                            }
                        }
                    }
                    _ = shutdown.recv() => break,
                }
            }
        });
    }

    /// Start the full discovery loop: advertise, scan, connect, sync.
    pub async fn start(
        self: &Arc<Self>,
        central: Arc<dyn BleCentral>,
        peripheral: Arc<dyn BlePeripheral>,
    ) {
        // Start message processing
        self.start_processing();

        // Channel for connection requests from the scan task
        let (connect_tx, mut connect_rx) = mpsc::channel::<(Uuid, BleAddress)>(16);

        // Start advertising
        let adv_data = self.build_advertisement().unwrap_or_default();
        let _ = peripheral.start_advertising(adv_data).await;

        // Start scanning
        let _ = central.start_scan().await;

        // Task: process scanned advertisements
        {
            let manager = Arc::clone(self);
            let mut adv_rx = central.advertisements();
            let connect_tx = connect_tx.clone();
            let mut shutdown = self.shutdown_tx.subscribe();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        result = adv_rx.recv() => {
                            if let Ok(adv) = result {
                                if let Some(peer_id) = manager.handle_advertisement(&adv).await {
                                    if manager.should_initiate_connection(&peer_id).await {
                                        let _ = connect_tx.send((peer_id, adv.source_address.clone())).await;
                                    }
                                }
                            }
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            });
        }

        // Task: initiate connections from scan results (central = initiator)
        {
            let manager = Arc::clone(self);
            let central = Arc::clone(&central);
            let mut shutdown = self.shutdown_tx.subscribe();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        request = connect_rx.recv() => {
                            if let Some((peer_id, address)) = request {
                                match central.connect(&address).await {
                                    Ok(conn) => {
                                        // Wrap with Noise IKpsk2 session (central = initiator)
                                        match manager.establish_initiator_session(conn, peer_id).await {
                                            Ok(secure_conn) => {
                                                let conn: Arc<dyn BleConnection> = Arc::new(secure_conn);
                                                manager.add_peer_connection(peer_id, conn, Some(address)).await;
                                            }
                                            Err(e) => {
                                                log::warn!("Session handshake failed with {}: {}", peer_id, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to connect to {}: {}", peer_id, e);
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            });
        }

        // Task: accept incoming connections (peripheral = responder)
        {
            let manager = Arc::clone(self);
            let peripheral = Arc::clone(&peripheral);
            let mut shutdown = self.shutdown_tx.subscribe();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        result = peripheral.accept() => {
                            match result {
                                Ok(conn) => {
                                    let peer_addr = conn.peer_address().clone();
                                    // Noise IKpsk2 handshake as responder — peer identity
                                    // is extracted from the handshake message, not from
                                    // address lookup.
                                    match manager.establish_responder_session(conn).await {
                                        Ok(secure_conn) => {
                                            let peer_id = secure_conn.peer_device_id();
                                            let conn: Arc<dyn BleConnection> = Arc::new(secure_conn);
                                            manager.add_peer_connection(peer_id, conn, Some(peer_addr)).await;
                                        }
                                        Err(e) => {
                                            log::warn!("Responder handshake failed: {}", e);
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            });
        }

        // Task: periodic advertisement refresh + stale cleanup
        {
            let manager = Arc::clone(self);
            let peripheral = Arc::clone(&peripheral);
            let interval = self.config.scan_interval;
            let mut shutdown = self.shutdown_tx.subscribe();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(interval) => {
                            if let Ok(adv_data) = manager.build_advertisement() {
                                let _ = peripheral.update_advertisement(adv_data).await;
                            }
                            manager.check_stale_pieces().await;
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            });
        }

        // Task: connect to static peers (e.g. Tailscale/VPN addresses)
        //
        // For each configured static peer, uses the UUID tiebreaker to decide
        // who initiates. On disconnect, waits briefly and reconnects.
        if !self.config.static_peers.is_empty() {
            let manager = Arc::clone(self);
            let static_peers = self.config.static_peers.clone();
            let mut shutdown = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                // Initial connection attempts for all static peers
                for (&peer_id, &peer_addr) in &static_peers {
                    if manager.should_initiate_connection(&peer_id).await {
                        Self::try_static_connect(&manager, peer_id, peer_addr).await;
                    }
                }

                // Reconnect loop: watch for disconnects and re-establish
                let mut disconnect_rx = manager.messenger.disconnections();
                loop {
                    tokio::select! {
                        result = disconnect_rx.recv() => {
                            if let Ok(peer_id) = result {
                                if let Some(&peer_addr) = static_peers.get(&peer_id) {
                                    if manager.should_initiate_connection(&peer_id).await {
                                        // Brief delay before reconnecting
                                        tokio::time::sleep(Duration::from_secs(2)).await;
                                        Self::try_static_connect(&manager, peer_id, peer_addr).await;
                                    }
                                }
                            }
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            });

            // For peers where *we* are the responder (higher UUID), we need a
            // TCP listener. Use a single listener for all static peers.
            let has_responder_peers = {
                let device_id = self.device_id;
                self.config.static_peers.keys().any(|peer_id| device_id >= *peer_id)
            };
            if has_responder_peers {
                // Determine our listen port from any static peer entry that
                // points to us (they all use the same port convention).
                // Fall back to the first peer's port.
                let listen_port = self.config.static_peers.values().next()
                    .map(|a| a.port())
                    .unwrap_or(7979);

                let manager = Arc::clone(self);
                let mut shutdown = self.shutdown_tx.subscribe();
                tokio::spawn(async move {
                    let listen_addr: std::net::SocketAddr =
                        ([0, 0, 0, 0], listen_port).into();
                    let listener = match tokio::net::TcpListener::bind(listen_addr).await {
                        Ok(l) => l,
                        Err(e) => {
                            log::warn!("Static peer listener failed to bind {}: {}", listen_addr, e);
                            return;
                        }
                    };
                    log::info!("Static peer listener on {}", listen_addr);

                    loop {
                        tokio::select! {
                            result = listener.accept() => {
                                match result {
                                    Ok((stream, _addr)) => {
                                        let mgr = Arc::clone(&manager);
                                        tokio::spawn(async move {
                                            Self::handle_static_accept(&mgr, stream).await;
                                        });
                                    }
                                    Err(e) => {
                                        log::warn!("Static peer accept error: {}", e);
                                    }
                                }
                            }
                            _ = shutdown.recv() => break,
                        }
                    }
                });
            }
        }
    }

    /// Attempt a TCP connection to a static peer, perform UUID handshake,
    /// and register with the ensemble.
    async fn try_static_connect(
        manager: &Arc<Self>,
        peer_id: Uuid,
        peer_addr: std::net::SocketAddr,
    ) {
        match tokio::net::TcpStream::connect(peer_addr).await {
            Ok(stream) => {
                match Self::do_static_handshake(manager, stream, peer_id, peer_addr).await {
                    Ok(()) => {
                        log::info!("Static peer connected: {}", peer_id);
                    }
                    Err(e) => {
                        log::warn!("Static peer handshake failed with {}: {}", peer_id, e);
                    }
                }
            }
            Err(e) => {
                log::debug!("Static peer {} not reachable at {}: {}", peer_id, peer_addr, e);
            }
        }
    }

    /// Handle an accepted TCP connection from a static peer.
    async fn handle_static_accept(manager: &Arc<Self>, stream: tokio::net::TcpStream) {
        // Read peer's UUID first
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = stream;

        let mut peer_bytes = [0u8; 16];
        if let Err(e) = stream.read_exact(&mut peer_bytes).await {
            log::warn!("Static accept: failed to read peer UUID: {}", e);
            return;
        }
        let peer_id = Uuid::from_bytes(peer_bytes);

        // Send our UUID
        if let Err(e) = stream.write_all(manager.device_id.as_bytes()).await {
            log::warn!("Static accept: failed to send UUID: {}", e);
            return;
        }

        // Wrap as BleConnection and register
        let peer_addr = stream.peer_addr().ok()
            .map(BleAddress::Tcp)
            .unwrap_or(BleAddress::Simulated(peer_id));
        let conn: Arc<dyn BleConnection> = Arc::new(
            crate::ble::lan_transport::LanConnection::from_stream(stream, peer_addr),
        );
        manager.add_direct_connection(peer_id, conn, TransportType::TcpDirect).await;
    }

    /// Perform UUID handshake as initiator and register the connection.
    async fn do_static_handshake(
        manager: &Arc<Self>,
        stream: tokio::net::TcpStream,
        expected_peer_id: Uuid,
        peer_addr_sock: std::net::SocketAddr,
    ) -> Result<(), String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = stream;

        // Send our UUID
        stream.write_all(manager.device_id.as_bytes()).await
            .map_err(|e| format!("write UUID: {}", e))?;

        // Read peer's UUID
        let mut peer_bytes = [0u8; 16];
        stream.read_exact(&mut peer_bytes).await
            .map_err(|e| format!("read peer UUID: {}", e))?;
        let peer_id = Uuid::from_bytes(peer_bytes);

        if peer_id != expected_peer_id {
            return Err(format!(
                "UUID mismatch: expected {}, got {}",
                expected_peer_id, peer_id
            ));
        }

        let peer_addr = BleAddress::Tcp(peer_addr_sock);
        let conn: Arc<dyn BleConnection> = Arc::new(
            crate::ble::lan_transport::LanConnection::from_stream(stream, peer_addr),
        );
        manager.add_direct_connection(peer_id, conn, TransportType::TcpDirect).await;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Session establishment
    // ------------------------------------------------------------------

    /// Build a `SessionIdentity` for initiating a Noise session with a known peer.
    fn session_identity_for(&self, peer_id: Uuid) -> Option<SessionIdentity> {
        let peer_key = self.peer_static_keys.get(&peer_id)?;
        let psk = session_psk(&self.capsule_keys);
        Some(SessionIdentity {
            local_private_key: self.device_identity.dh_secret_bytes(),
            local_public_key: self.device_identity.dh_public_bytes(),
            peer_static_public: *peer_key,
            psk,
            local_device_id: self.device_id,
            peer_device_id: peer_id,
        })
    }

    /// Build a `ResponderIdentity` for accepting a Noise session from any capsule peer.
    fn responder_identity(&self) -> ResponderIdentity {
        let psk = session_psk(&self.capsule_keys);
        let known_peers: Vec<([u8; 32], Uuid)> = self
            .peer_static_keys
            .iter()
            .map(|(id, key)| (*key, *id))
            .collect();
        ResponderIdentity {
            local_private_key: self.device_identity.dh_secret_bytes(),
            local_public_key: self.device_identity.dh_public_bytes(),
            psk,
            local_device_id: self.device_id,
            known_peers,
        }
    }

    /// Establish a Noise IKpsk2 session as initiator with a known peer.
    async fn establish_initiator_session(
        &self,
        conn: Box<dyn BleConnection>,
        peer_id: Uuid,
    ) -> Result<crate::ble::session::SecureBleConnection, crate::ble::session::SessionError> {
        let identity = self.session_identity_for(peer_id).ok_or(
            crate::ble::session::SessionError::UnknownPeer,
        )?;
        establish_initiator(conn, &identity).await
    }

    /// Establish a Noise IKpsk2 session as responder (peer is identified from handshake).
    async fn establish_responder_session(
        &self,
        conn: Box<dyn BleConnection>,
    ) -> Result<crate::ble::session::SecureBleConnection, crate::ble::session::SessionError> {
        let identity = self.responder_identity();
        establish_responder(conn, &identity).await
    }

    // ------------------------------------------------------------------
    // Connection management
    // ------------------------------------------------------------------

    /// Register a pre-established connection (e.g. TCP) directly,
    /// bypassing the scan→advertise pipeline.
    ///
    /// Creates bidirectional topology edges with the specified transport
    /// type and wires the connection into the messenger for routing.
    pub async fn add_direct_connection(
        &self,
        peer_id: Uuid,
        conn: Arc<dyn BleConnection>,
        transport_type: TransportType,
    ) {
        // 1. Register with messenger (starts recv loop)
        self.messenger.add_connection(peer_id, conn).await;

        // 2. Update topology with bidirectional edges
        {
            let mut topo = self.topology.write().await;
            topo.upsert_piece(PiecePresence {
                device_id: self.device_id,
                last_advertisement: Utc::now(),
                last_data_exchange: Some(Utc::now()),
                rssi: None,
                reachability: PieceReachability::Direct,
            });
            topo.upsert_piece(PiecePresence {
                device_id: peer_id,
                last_advertisement: Utc::now(),
                last_data_exchange: Some(Utc::now()),
                rssi: None,
                reachability: PieceReachability::Direct,
            });
            topo.add_edge(TopologyEdge {
                from: self.device_id,
                to: peer_id,
                transport: transport_type.clone(),
                quality: ConnectionQuality::unknown(),
            });
            topo.add_edge(TopologyEdge {
                from: peer_id,
                to: self.device_id,
                transport: transport_type,
                quality: ConnectionQuality::unknown(),
            });
        }

        // 3. Send our topology view to the peer
        self.send_topology_update(peer_id).await;

        // 4. Propagate peer introductions
        self.propagate_peer_info(peer_id).await;
    }

    /// Register a peer connection: update topology, start message routing,
    /// exchange topology info, and propagate peer introductions.
    pub async fn add_peer_connection(
        &self,
        peer_id: Uuid,
        conn: Arc<dyn BleConnection>,
        address: Option<BleAddress>,
    ) {
        // 1. Register with messenger (starts recv loop)
        self.messenger.add_connection(peer_id, conn).await;

        // 2. Update topology with bidirectional edges
        {
            let mut topo = self.topology.write().await;
            topo.upsert_piece(PiecePresence {
                device_id: self.device_id,
                last_advertisement: Utc::now(),
                last_data_exchange: Some(Utc::now()),
                rssi: None,
                reachability: PieceReachability::Direct,
            });
            topo.upsert_piece(PiecePresence {
                device_id: peer_id,
                last_advertisement: Utc::now(),
                last_data_exchange: Some(Utc::now()),
                rssi: None,
                reachability: PieceReachability::Direct,
            });
            topo.add_edge(TopologyEdge {
                from: self.device_id,
                to: peer_id,
                transport: TransportType::BleDirect,
                quality: ConnectionQuality::unknown(),
            });
            topo.add_edge(TopologyEdge {
                from: peer_id,
                to: self.device_id,
                transport: TransportType::BleDirect,
                quality: ConnectionQuality::unknown(),
            });
        }

        // 3. Store address
        if let Some(addr) = address {
            let mut addrs = self.peer_addresses.write().await;
            addrs.insert(peer_id, addr);
        }

        // 4. Send our topology view to the peer
        self.send_topology_update(peer_id).await;

        // 5. Bidirectional peer introductions
        self.propagate_peer_info(peer_id).await;
    }

    // ------------------------------------------------------------------
    // Topology sync
    // ------------------------------------------------------------------

    /// Send our topology view to a specific peer.
    async fn send_topology_update(&self, peer_id: Uuid) {
        let (direct, indirect, hash) = {
            let topo = self.topology.read().await;
            let hash = topo.topology_hash();
            let mut direct_peers = Vec::new();
            let mut indirect_peers = Vec::new();

            for (id, presence) in &topo.online_pieces {
                if *id == self.device_id || *id == peer_id {
                    continue;
                }
                let peer_info = PeerInfo {
                    device_id: *id,
                    ble_address: None,
                    last_seen: presence.last_advertisement,
                    capabilities: PieceCapabilities::full(),
                };
                match &presence.reachability {
                    PieceReachability::Direct => direct_peers.push(peer_info),
                    PieceReachability::Indirect { .. } => indirect_peers.push(peer_info),
                    PieceReachability::AdvertisementOnly => {}
                }
            }
            (direct_peers, indirect_peers, hash)
        };

        // Fill in addresses
        let addrs = self.peer_addresses.read().await;
        let direct: Vec<PeerInfo> = direct
            .into_iter()
            .map(|mut p| {
                p.ble_address = addrs.get(&p.device_id).cloned();
                p
            })
            .collect();
        let indirect: Vec<PeerInfo> = indirect
            .into_iter()
            .map(|mut p| {
                p.ble_address = addrs.get(&p.device_id).cloned();
                p
            })
            .collect();
        drop(addrs);

        let msg = TopologySyncMessage::TopologyUpdate {
            direct_peers: direct,
            indirect_peers: indirect,
            topology_hash: hash,
        };
        let mut payload = Vec::new();
        if ciborium::into_writer(&msg, &mut payload).is_ok() {
            let _ = self
                .messenger
                .send_to(peer_id, MessageType::TopologySync, &payload)
                .await;
        }
    }

    /// After connecting to a new peer, introduce all existing peers to
    /// the new peer and the new peer to all existing peers.
    async fn propagate_peer_info(&self, new_peer_id: Uuid) {
        let existing_peers: Vec<Uuid> = {
            let topo = self.topology.read().await;
            topo.online_pieces
                .keys()
                .filter(|id| **id != new_peer_id && **id != self.device_id)
                .cloned()
                .collect()
        };

        // Direction 1: tell the new peer about each existing peer
        for &peer_id in &existing_peers {
            let address = {
                let addrs = self.peer_addresses.read().await;
                addrs.get(&peer_id).cloned()
            };
            let intro = TopologySyncMessage::PeerIntroduction {
                piece_id: peer_id,
                ble_address: address,
                last_advertisement: None,
                quality: ConnectionQuality::unknown(),
            };
            let mut payload = Vec::new();
            if ciborium::into_writer(&intro, &mut payload).is_ok() {
                let _ = self
                    .messenger
                    .send_to(new_peer_id, MessageType::TopologySync, &payload)
                    .await;
            }
        }

        // Direction 2: tell each existing peer about the new peer
        let new_addr = {
            let addrs = self.peer_addresses.read().await;
            addrs.get(&new_peer_id).cloned()
        };
        let intro = TopologySyncMessage::PeerIntroduction {
            piece_id: new_peer_id,
            ble_address: new_addr,
            last_advertisement: None,
            quality: ConnectionQuality::unknown(),
        };
        let mut payload = Vec::new();
        if ciborium::into_writer(&intro, &mut payload).is_ok() {
            for &peer_id in &existing_peers {
                let _ = self
                    .messenger
                    .send_to(peer_id, MessageType::TopologySync, &payload)
                    .await;
            }
        }
    }

    /// Process an incoming TopologySync message.
    async fn process_topology_message(&self, envelope: &RoutedEnvelope) {
        let msg: TopologySyncMessage = match ciborium::from_reader(&envelope.payload[..]) {
            Ok(m) => m,
            Err(_) => return,
        };

        match msg {
            TopologySyncMessage::TopologyUpdate {
                direct_peers,
                indirect_peers,
                ..
            } => {
                self.handle_topology_update_msg(envelope.source, direct_peers, indirect_peers)
                    .await;
            }
            TopologySyncMessage::PeerIntroduction {
                piece_id,
                ble_address,
                quality,
                ..
            } => {
                self.handle_peer_introduction(envelope.source, piece_id, ble_address, quality)
                    .await;
            }
        }
    }

    /// Merge a peer's topology view into ours.
    async fn handle_topology_update_msg(
        &self,
        from_peer: Uuid,
        direct_peers: Vec<PeerInfo>,
        indirect_peers: Vec<PeerInfo>,
    ) {
        for peer_info in direct_peers.iter().chain(indirect_peers.iter()) {
            if peer_info.device_id == self.device_id {
                continue;
            }
            {
                let mut topo = self.topology.write().await;
                if topo.get_piece(&peer_info.device_id).is_none() {
                    topo.upsert_piece(PiecePresence {
                        device_id: peer_info.device_id,
                        last_advertisement: peer_info.last_seen,
                        last_data_exchange: None,
                        rssi: None,
                        reachability: PieceReachability::Indirect {
                            next_hop: from_peer,
                            hop_count: 2,
                        },
                    });
                    topo.add_edge(TopologyEdge {
                        from: from_peer,
                        to: peer_info.device_id,
                        transport: TransportType::BleDirect,
                        quality: ConnectionQuality::unknown(),
                    });
                }
            }
            if let Some(addr) = &peer_info.ble_address {
                let mut addrs = self.peer_addresses.write().await;
                addrs.insert(peer_info.device_id, addr.clone());
            }
        }
    }

    /// Handle a peer introduction: add the introduced piece as indirect.
    async fn handle_peer_introduction(
        &self,
        from_peer: Uuid,
        piece_id: Uuid,
        address: Option<BleAddress>,
        quality: ConnectionQuality,
    ) {
        if piece_id == self.device_id {
            return;
        }
        {
            let mut topo = self.topology.write().await;
            if topo.get_piece(&piece_id).is_none() {
                topo.upsert_piece(PiecePresence {
                    device_id: piece_id,
                    last_advertisement: Utc::now(),
                    last_data_exchange: None,
                    rssi: None,
                    reachability: PieceReachability::Indirect {
                        next_hop: from_peer,
                        hop_count: 1,
                    },
                });
                topo.add_edge(TopologyEdge {
                    from: from_peer,
                    to: piece_id,
                    transport: TransportType::BleDirect,
                    quality,
                });
            }
        }
        if let Some(addr) = address {
            let mut addrs = self.peer_addresses.write().await;
            addrs.insert(piece_id, addr);
        }
    }

    // ------------------------------------------------------------------
    // Advertisement handling
    // ------------------------------------------------------------------

    /// Process a received BLE advertisement.
    /// Returns the peer's device_id if it was a valid capsule member.
    pub async fn handle_advertisement(&self, adv: &BleAdvertisement) -> Option<Uuid> {
        let (_capsule_id, payload) =
            try_decrypt_advertisement(&adv.data, &self.known_capsules)?;

        // Skip our own advertisements
        let our_hint = piece_hint_for(&self.device_id);
        if payload.piece_hint == our_hint {
            return None;
        }

        // Resolve piece_hint -> device_id
        let peer_id = {
            let hints = self.piece_hints.read().await;
            hints.get(&payload.piece_hint).cloned()
        }?;

        // Update presence
        {
            let mut topo = self.topology.write().await;
            if let Some(presence) = topo.get_piece_mut(&peer_id) {
                presence.last_advertisement = Utc::now();
                presence.rssi = adv.rssi;
            } else {
                topo.upsert_piece(PiecePresence {
                    device_id: peer_id,
                    last_advertisement: Utc::now(),
                    last_data_exchange: None,
                    rssi: adv.rssi,
                    reachability: PieceReachability::AdvertisementOnly,
                });
            }
        }

        // Store their BLE address
        {
            let mut addrs = self.peer_addresses.write().await;
            addrs.insert(peer_id, adv.source_address.clone());
        }

        Some(peer_id)
    }

    /// Build an encrypted advertisement payload.
    fn build_advertisement(&self) -> Result<Vec<u8>, String> {
        let seq = self.adv_seq.fetch_add(1, Ordering::Relaxed);
        let topology_hash = match self.topology.try_read() {
            Ok(topo) => topo.topology_hash(),
            Err(_) => 0,
        };

        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&self.capsule_keys.capsule_id),
            piece_hint: piece_hint_for(&self.device_id),
            seq,
            topology_hash,
            known_pieces: vec![],
        };

        encrypt_advertisement(&payload, &self.capsule_keys).map_err(|e| e.to_string())
    }

    /// Whether we should initiate a connection to this peer.
    /// Uses device_id comparison as a tiebreaker to avoid duplicates.
    async fn should_initiate_connection(&self, peer_id: &Uuid) -> bool {
        if self.device_id >= *peer_id {
            return false; // The other side initiates
        }
        let topo = self.topology.read().await;
        match topo.get_piece(peer_id) {
            Some(p) => matches!(p.reachability, PieceReachability::AdvertisementOnly),
            None => true,
        }
    }

    // ------------------------------------------------------------------
    // Stale piece detection
    // ------------------------------------------------------------------

    /// Remove pieces that haven't been seen within `stale_timeout`.
    pub async fn check_stale_pieces(&self) {
        let now = Utc::now();
        let stale_timeout = chrono::Duration::from_std(self.config.stale_timeout)
            .unwrap_or(chrono::Duration::seconds(30));

        let stale_ids: Vec<Uuid> = {
            let topo = self.topology.read().await;
            topo.online_pieces
                .iter()
                .filter(|(id, presence)| {
                    **id != self.device_id
                        && (now - presence.last_advertisement) > stale_timeout
                        && presence
                            .last_data_exchange
                            .map(|t| (now - t) > stale_timeout)
                            .unwrap_or(true)
                })
                .map(|(id, _)| *id)
                .collect()
        };

        if !stale_ids.is_empty() {
            let mut topo = self.topology.write().await;
            for id in &stale_ids {
                topo.remove_piece(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::simulated::SimBleNetwork;
    use crate::ble::transport::{BleCentral, BlePeripheral};
    use crate::identity::DeviceIdentity;
    use std::sync::Arc;
    use std::time::Duration;

    /// Create a DeviceIdentity with a specific UUID (for test determinism).
    /// The identity's actual device_id is random; we override it by generating
    /// fresh and using the identity's own UUID. Tests that need a specific UUID
    /// pass connections via `add_peer_connection` (no handshake), so the identity
    /// UUID just needs to match what we pass to the manager constructor.
    fn make_test_identity() -> Arc<DeviceIdentity> {
        Arc::new(DeviceIdentity::generate())
    }

    /// Build an EnsembleManager for tests using real DeviceIdentity keys.
    /// The device_id comes from the identity; peer_static_keys are populated
    /// from the peer identities passed in.
    fn make_manager(
        identity: &Arc<DeviceIdentity>,
        peer_identities: &[&Arc<DeviceIdentity>],
        capsule_keys: CapsuleKeyBundle,
        config: EnsembleConfig,
    ) -> Arc<EnsembleManager> {
        let all_ids: Vec<Uuid> = std::iter::once(identity.device_id())
            .chain(peer_identities.iter().map(|p| p.device_id()))
            .collect();
        let peer_keys: HashMap<Uuid, [u8; 32]> = peer_identities
            .iter()
            .map(|p| (p.device_id(), p.dh_public_bytes()))
            .collect();
        EnsembleManager::new(
            Arc::clone(identity),
            capsule_keys,
            all_ids,
            peer_keys,
            config,
        )
    }

    /// Establish a BLE connection pair via the simulated network.
    /// MTU set to 512: topology sync messages (CBOR-encoded RoutedEnvelope
    /// wrapping TopologySyncMessage) exceed the BLE minimum 247 bytes.
    /// Real BLE stacks negotiate 512+ on modern devices.
    async fn make_connection(
        network: &Arc<SimBleNetwork>,
    ) -> (Arc<dyn BleConnection>, Arc<dyn BleConnection>) {
        let mut device_a = network.create_device();
        let device_b = network.create_device();
        device_a.set_mtu(512);
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();
        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        (Arc::from(conn_a), Arc::from(conn_b))
    }

    #[tokio::test(start_paused = true)]
    async fn test_direct_connection_topology() {
        let network = SimBleNetwork::new();
        let (conn_a, conn_b) = make_connection(&network).await;

        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let identity_a = make_test_identity();
        let identity_b = make_test_identity();
        let id_a = identity_a.device_id();
        let id_b = identity_b.device_id();
        let config = EnsembleConfig::default();

        let mgr_a = make_manager(&identity_a, &[&identity_b], capsule_keys.clone(), config.clone());
        let mgr_b = make_manager(&identity_b, &[&identity_a], capsule_keys, config);

        mgr_a.start_processing();
        mgr_b.start_processing();

        mgr_a.add_peer_connection(id_b, conn_a, None).await;
        mgr_b.add_peer_connection(id_a, conn_b, None).await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Both topologies should have both pieces with Direct reachability
        {
            let topo_a = mgr_a.topology().read().await;
            assert!(topo_a.get_piece(&id_a).is_some());
            assert!(topo_a.get_piece(&id_b).is_some());
            assert!(matches!(
                topo_a.get_piece(&id_b).unwrap().reachability,
                PieceReachability::Direct
            ));
        }
        {
            let topo_b = mgr_b.topology().read().await;
            assert!(topo_b.get_piece(&id_a).is_some());
            assert!(topo_b.get_piece(&id_b).is_some());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_peer_introduction_propagation() {
        let network = SimBleNetwork::new();
        let (conn_ab_a, conn_ab_b) = make_connection(&network).await;
        let (conn_ac_a, conn_ac_c) = make_connection(&network).await;

        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let identity_a = make_test_identity();
        let identity_b = make_test_identity();
        let identity_c = make_test_identity();
        let id_a = identity_a.device_id();
        let id_b = identity_b.device_id();
        let id_c = identity_c.device_id();
        let config = EnsembleConfig::default();

        let mgr_a = make_manager(&identity_a, &[&identity_b, &identity_c], capsule_keys.clone(), config.clone());
        let mgr_b = make_manager(&identity_b, &[&identity_a, &identity_c], capsule_keys.clone(), config.clone());
        let mgr_c = make_manager(&identity_c, &[&identity_a, &identity_b], capsule_keys, config);

        mgr_a.start_processing();
        mgr_b.start_processing();
        mgr_c.start_processing();

        // Step 1: A connects to B
        mgr_a.add_peer_connection(id_b, conn_ab_a, None).await;
        mgr_b.add_peer_connection(id_a, conn_ab_b, None).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Step 2: A connects to C — A now introduces B↔C to each other
        mgr_a.add_peer_connection(id_c, conn_ac_a, None).await;
        mgr_c.add_peer_connection(id_a, conn_ac_c, None).await;
        // Let the recv loops pick up PeerIntroductions and process them
        tokio::time::sleep(Duration::from_millis(50)).await;

        // B should know about C (introduced by A)
        {
            let topo_b = mgr_b.topology().read().await;
            let c_presence = topo_b.get_piece(&id_c);
            assert!(c_presence.is_some(), "B should know about C");
            assert!(
                matches!(
                    c_presence.unwrap().reachability,
                    PieceReachability::Indirect { next_hop, .. } if next_hop == id_a
                ),
                "C should be reachable through A from B's perspective"
            );
        }

        // C should know about B (introduced by A)
        {
            let topo_c = mgr_c.topology().read().await;
            let b_presence = topo_c.get_piece(&id_b);
            assert!(b_presence.is_some(), "C should know about B");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_advertisement_updates_presence() {
        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let identity_a = make_test_identity();
        let identity_b = make_test_identity();
        let id_b = identity_b.device_id();

        let mgr = make_manager(&identity_a, &[&identity_b], capsule_keys.clone(), EnsembleConfig::default());

        // Simulate an encrypted advertisement from B
        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&capsule_keys.capsule_id),
            piece_hint: piece_hint_for(&id_b),
            seq: 1,
            topology_hash: 0,
            known_pieces: vec![],
        };
        let encrypted = encrypt_advertisement(&payload, &capsule_keys).unwrap();
        let adv = BleAdvertisement {
            data: encrypted,
            rssi: Some(-60),
            source_address: BleAddress::Simulated(Uuid::new_v4()),
        };

        let result = mgr.handle_advertisement(&adv).await;
        assert_eq!(result, Some(id_b));

        let topo = mgr.topology().read().await;
        let presence = topo.get_piece(&id_b).unwrap();
        assert!(matches!(
            presence.reachability,
            PieceReachability::AdvertisementOnly
        ));
        assert_eq!(presence.rssi, Some(-60));
    }

    #[tokio::test(start_paused = true)]
    async fn test_stale_piece_removal() {
        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let identity_a = make_test_identity();
        let identity_b = make_test_identity();
        let id_b = identity_b.device_id();

        let config = EnsembleConfig {
            stale_timeout: Duration::from_secs(5),
            ..Default::default()
        };
        let mgr = make_manager(&identity_a, &[&identity_b], capsule_keys, config);

        // Add a piece with a timestamp far in the past
        {
            let mut topo = mgr.topology().write().await;
            topo.upsert_piece(PiecePresence {
                device_id: id_b,
                last_advertisement: Utc::now() - chrono::Duration::seconds(60),
                last_data_exchange: None,
                rssi: None,
                reachability: PieceReachability::AdvertisementOnly,
            });
        }

        mgr.check_stale_pieces().await;
        assert!(
            mgr.topology().read().await.get_piece(&id_b).is_none(),
            "Stale piece should have been removed"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_own_advertisement_ignored() {
        let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let identity_a = make_test_identity();
        let id_a = identity_a.device_id();

        let mgr = make_manager(&identity_a, &[], capsule_keys.clone(), EnsembleConfig::default());

        // Simulate our own advertisement
        let payload = AdvertisementPayload {
            capsule_hint: capsule_hint_for(&capsule_keys.capsule_id),
            piece_hint: piece_hint_for(&id_a),
            seq: 1,
            topology_hash: 0,
            known_pieces: vec![],
        };
        let encrypted = encrypt_advertisement(&payload, &capsule_keys).unwrap();
        let adv = BleAdvertisement {
            data: encrypted,
            rssi: None,
            source_address: BleAddress::Simulated(Uuid::new_v4()),
        };

        let result = mgr.handle_advertisement(&adv).await;
        assert_eq!(result, None, "Own advertisement should be ignored");
    }
}
