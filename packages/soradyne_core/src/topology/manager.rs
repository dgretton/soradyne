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
use crate::ble::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use crate::identity::CapsuleKeyBundle;
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
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(2),
            stale_timeout: Duration::from_secs(30),
        }
    }
}

/// Manages ensemble lifecycle: discovery, connection, topology sync,
/// and peer introduction propagation.
pub struct EnsembleManager {
    device_id: Uuid,
    capsule_keys: CapsuleKeyBundle,
    known_capsules: Vec<CapsuleKeyBundle>,
    /// piece_hint -> device_id (computed from capsule piece list).
    piece_hints: RwLock<HashMap<[u8; 4], Uuid>>,
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
    /// `known_piece_ids` should contain the device_ids of all pieces in
    /// the capsule (including our own), used to resolve piece_hints from
    /// encrypted advertisements.
    pub fn new(
        device_id: Uuid,
        capsule_keys: CapsuleKeyBundle,
        known_piece_ids: Vec<Uuid>,
        config: EnsembleConfig,
    ) -> Arc<Self> {
        let topology = Arc::new(RwLock::new(EnsembleTopology::new()));
        let messenger = TopologyMessenger::new(device_id, Arc::clone(&topology));

        let mut hints = HashMap::new();
        for id in &known_piece_ids {
            hints.insert(piece_hint_for(id), *id);
        }

        let (shutdown_tx, _) = broadcast::channel(1);

        Arc::new(Self {
            device_id,
            capsule_keys: capsule_keys.clone(),
            known_capsules: vec![capsule_keys],
            piece_hints: RwLock::new(hints),
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

        // Task: initiate connections from scan results
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
                                        let conn: Arc<dyn BleConnection> = Arc::from(conn);
                                        manager.add_peer_connection(peer_id, conn, Some(address)).await;
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

        // Task: accept incoming connections
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
                                    let conn: Arc<dyn BleConnection> = Arc::from(conn);
                                    let peer_addr = conn.peer_address().clone();
                                    // Reverse-lookup: BleAddress -> device_id
                                    let peer_id = {
                                        let addrs = manager.peer_addresses.read().await;
                                        addrs.iter()
                                            .find(|(_, addr)| **addr == peer_addr)
                                            .map(|(id, _)| *id)
                                    };
                                    if let Some(peer_id) = peer_id {
                                        manager.add_peer_connection(peer_id, conn, Some(peer_addr)).await;
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
    }

    // ------------------------------------------------------------------
    // Connection management
    // ------------------------------------------------------------------

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
    use std::sync::Arc;
    use std::time::Duration;

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
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let config = EnsembleConfig::default();

        let mgr_a = EnsembleManager::new(
            id_a,
            capsule_keys.clone(),
            vec![id_a, id_b],
            config.clone(),
        );
        let mgr_b = EnsembleManager::new(id_b, capsule_keys, vec![id_a, id_b], config);

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
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();
        let config = EnsembleConfig::default();
        let ids = vec![id_a, id_b, id_c];

        let mgr_a = EnsembleManager::new(id_a, capsule_keys.clone(), ids.clone(), config.clone());
        let mgr_b = EnsembleManager::new(id_b, capsule_keys.clone(), ids.clone(), config.clone());
        let mgr_c = EnsembleManager::new(id_c, capsule_keys, ids, config);

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
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let mgr = EnsembleManager::new(
            id_a,
            capsule_keys.clone(),
            vec![id_a, id_b],
            EnsembleConfig::default(),
        );

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
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let config = EnsembleConfig {
            stale_timeout: Duration::from_secs(5),
            ..Default::default()
        };
        let mgr = EnsembleManager::new(id_a, capsule_keys, vec![id_a, id_b], config);

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
        let id_a = Uuid::new_v4();

        let mgr = EnsembleManager::new(
            id_a,
            capsule_keys.clone(),
            vec![id_a],
            EnsembleConfig::default(),
        );

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
