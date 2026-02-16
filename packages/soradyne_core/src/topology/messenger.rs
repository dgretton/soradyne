//! TopologyMessenger — logical routing layer
//!
//! Routes messages between pieces using BLE connections and the
//! topology graph. Handles multi-hop forwarding, broadcast delivery,
//! and connection lifecycle management.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use serde::{de::DeserializeOwned, Serialize};

use crate::ble::gatt::{MessageType, RoutedEnvelope};
use crate::ble::transport::BleConnection;
use crate::ble::BleError;
use crate::topology::ensemble::{EnsembleTopology, PieceReachability};

/// Serialize a value to CBOR bytes.
fn cbor_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Deserialize a value from CBOR bytes.
fn cbor_deserialize<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    ciborium::from_reader(data).map_err(|e| e.to_string())
}

/// Errors that can occur during message routing.
#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("No route to destination {0}")]
    Unreachable(Uuid),

    #[error("TTL exceeded — routing loop broken")]
    TtlExceeded,

    #[error("Transport error: {0}")]
    TransportError(#[from] BleError),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Logical routing layer for multi-hop mesh communication.
///
/// Manages BLE connections, routes unicast messages via topology-aware
/// next-hop selection, and delivers locally-addressed messages to subscribers.
pub struct TopologyMessenger {
    /// Our device identity.
    device_id: Uuid,
    /// Shared topology graph (also used by future EnsembleManager).
    topology: Arc<RwLock<EnsembleTopology>>,
    /// Active BLE connections keyed by peer device ID.
    connections: Arc<RwLock<HashMap<Uuid, Arc<dyn BleConnection>>>>,
    /// Channel for delivering messages addressed to us.
    incoming_tx: broadcast::Sender<RoutedEnvelope>,
}

impl TopologyMessenger {
    /// Create a new TopologyMessenger.
    ///
    /// Returns `Arc<Self>` because `add_connection` spawns tasks that
    /// hold a reference.
    pub fn new(device_id: Uuid, topology: Arc<RwLock<EnsembleTopology>>) -> Arc<Self> {
        let (incoming_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            device_id,
            topology,
            connections: Arc::new(RwLock::new(HashMap::new())),
            incoming_tx,
        })
    }

    /// Send a unicast message to a specific destination.
    pub async fn send_to(
        &self,
        destination: Uuid,
        message_type: MessageType,
        payload: &[u8],
    ) -> Result<(), RoutingError> {
        let envelope = RoutedEnvelope::new_unicast(
            self.device_id,
            destination,
            message_type,
            payload.to_vec(),
        );

        let next_hop = {
            let topo = self.topology.read().await;
            self.next_hop_for(&topo, &destination)
        };
        let next_hop = next_hop.ok_or(RoutingError::Unreachable(destination))?;

        let serialized = cbor_serialize(&envelope)
            .map_err(|e| RoutingError::SerializationError(e.to_string()))?;

        let conn = {
            let conns = self.connections.read().await;
            conns.get(&next_hop).cloned()
        };

        match conn {
            Some(c) => c.send(&serialized).await.map_err(RoutingError::TransportError),
            None => Err(RoutingError::Unreachable(destination)),
        }
    }

    /// Broadcast a message to all direct connections.
    pub async fn broadcast(
        &self,
        message_type: MessageType,
        payload: &[u8],
    ) -> Result<(), RoutingError> {
        let envelope = RoutedEnvelope::new_broadcast(self.device_id, message_type, payload.to_vec());

        let serialized = cbor_serialize(&envelope)
            .map_err(|e| RoutingError::SerializationError(e.to_string()))?;

        let connections: Vec<Arc<dyn BleConnection>> = {
            let conns = self.connections.read().await;
            conns.values().cloned().collect()
        };

        for conn in connections {
            let _ = conn.send(&serialized).await;
        }

        Ok(())
    }

    /// Subscribe to locally-delivered messages.
    pub fn incoming(&self) -> broadcast::Receiver<RoutedEnvelope> {
        self.incoming_tx.subscribe()
    }

    /// Check whether a destination is reachable.
    pub fn is_reachable(&self, destination: &Uuid) -> bool {
        match self.topology.try_read() {
            Ok(topo) => topo.is_reachable(&self.device_id, destination),
            Err(_) => false,
        }
    }

    /// Register a BLE connection and start its receive loop.
    pub async fn add_connection(
        self: &Arc<Self>,
        peer_id: Uuid,
        conn: Arc<dyn BleConnection>,
    ) {
        {
            let mut conns = self.connections.write().await;
            conns.insert(peer_id, Arc::clone(&conn));
        }

        let messenger = Arc::clone(self);
        let peer = peer_id;
        tokio::spawn(async move {
            loop {
                match conn.recv().await {
                    Ok(data) => match cbor_deserialize::<RoutedEnvelope>(&data) {
                        Ok(envelope) => {
                            messenger.handle_incoming(envelope).await;
                        }
                        Err(e) => {
                            log::warn!("Failed to deserialize envelope from {}: {}", peer, e);
                        }
                    },
                    Err(BleError::Disconnected) => {
                        messenger.remove_connection(&peer).await;
                        break;
                    }
                    Err(e) => {
                        log::warn!("Recv error from {}: {}", peer, e);
                        messenger.remove_connection(&peer).await;
                        break;
                    }
                }
            }
        });
    }

    /// Remove a connection and its topology edges.
    pub async fn remove_connection(&self, peer_id: &Uuid) {
        {
            let mut conns = self.connections.write().await;
            conns.remove(peer_id);
        }
        {
            let mut topo = self.topology.write().await;
            topo.remove_edges_between(&self.device_id, peer_id);
            topo.remove_edges_between(peer_id, &self.device_id);
        }
    }

    /// Number of active connections.
    pub fn connection_count(&self) -> usize {
        match self.connections.try_read() {
            Ok(conns) => conns.len(),
            Err(_) => 0,
        }
    }

    /// Handle an incoming envelope: deliver locally and/or forward.
    async fn handle_incoming(&self, envelope: RoutedEnvelope) {
        let is_broadcast = envelope.destination == Uuid::nil();
        let is_for_us = envelope.destination == self.device_id;

        // Deliver locally if addressed to us or broadcast
        if is_for_us || is_broadcast {
            let _ = self.incoming_tx.send(envelope.clone());
        }

        // Forward if needed: broadcasts always forward, unicast only if not for us
        if is_broadcast || !is_for_us {
            if let Some(forwarded) = envelope.forwarded() {
                if is_broadcast {
                    // Broadcast: forward to all connections except source
                    let serialized = match cbor_serialize(&forwarded) {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let connections: Vec<(Uuid, Arc<dyn BleConnection>)> = {
                        let conns = self.connections.read().await;
                        conns.iter().map(|(id, c)| (*id, Arc::clone(c))).collect()
                    };
                    for (peer_id, conn) in connections {
                        if forwarded.should_forward_to(&peer_id) {
                            let _ = conn.send(&serialized).await;
                        }
                    }
                } else {
                    // Unicast not for us: route via next hop
                    let next_hop = {
                        let topo = self.topology.read().await;
                        self.next_hop_for(&topo, &forwarded.destination)
                    };
                    if let Some(hop) = next_hop {
                        let serialized = match cbor_serialize(&forwarded) {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                        let conn = {
                            let conns = self.connections.read().await;
                            conns.get(&hop).cloned()
                        };
                        if let Some(c) = conn {
                            let _ = c.send(&serialized).await;
                        }
                    }
                }
            }
        }
    }

    /// Compute the next hop toward a destination.
    fn next_hop_for(&self, topology: &EnsembleTopology, destination: &Uuid) -> Option<Uuid> {
        match topology.compute_reachability(&self.device_id, destination) {
            Some(PieceReachability::Direct) => Some(*destination),
            Some(PieceReachability::Indirect { next_hop, .. }) => Some(next_hop),
            Some(PieceReachability::AdvertisementOnly) | None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::simulated::SimBleNetwork;
    use crate::ble::transport::{BleCentral, BlePeripheral};
    use crate::topology::ensemble::{ConnectionQuality, TopologyEdge, TransportType};
    use std::sync::Arc;
    use std::time::Duration;

    fn make_sim_edge(from: Uuid, to: Uuid) -> TopologyEdge {
        TopologyEdge {
            from,
            to,
            transport: TransportType::SimulatedBle,
            quality: ConnectionQuality::unknown(),
        }
    }

    /// Establish a BLE connection pair via the simulated network.
    async fn make_connection(
        network: &Arc<SimBleNetwork>,
    ) -> (Arc<dyn BleConnection>, Arc<dyn BleConnection>) {
        let device_a = network.create_device();
        let device_b = network.create_device();
        let addr_b = device_b.address().clone();

        device_b.start_advertising(vec![0x01]).await.unwrap();
        let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

        let conn_a = device_a.connect(&addr_b).await.unwrap();
        let conn_b = accept_handle.await.unwrap();

        (Arc::from(conn_a), Arc::from(conn_b))
    }

    #[tokio::test(start_paused = true)]
    async fn test_direct_send() {
        let network = SimBleNetwork::new();
        let (conn_a, conn_b) = make_connection(&network).await;

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let make_topo = || {
            let mut t = EnsembleTopology::new();
            t.add_edge(make_sim_edge(id_a, id_b));
            t.add_edge(make_sim_edge(id_b, id_a));
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, make_topo());
        let messenger_b = TopologyMessenger::new(id_b, make_topo());

        let mut rx_b = messenger_b.incoming();

        messenger_a.add_connection(id_b, conn_a).await;
        messenger_b.add_connection(id_a, conn_b).await;

        messenger_a
            .send_to(id_b, MessageType::FlowSync, b"hello")
            .await
            .unwrap();

        let envelope = rx_b.recv().await.unwrap();
        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.destination, id_b);
        assert_eq!(envelope.payload, b"hello");
        assert_eq!(envelope.message_type, MessageType::FlowSync);
    }

    #[tokio::test(start_paused = true)]
    async fn test_broadcast() {
        let network = SimBleNetwork::new();
        let (conn_ab_a, conn_ab_b) = make_connection(&network).await;
        let (conn_ac_a, conn_ac_c) = make_connection(&network).await;

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();

        let topo_a = Arc::new(RwLock::new(EnsembleTopology::new()));
        let topo_b = Arc::new(RwLock::new(EnsembleTopology::new()));
        let topo_c = Arc::new(RwLock::new(EnsembleTopology::new()));

        let messenger_a = TopologyMessenger::new(id_a, topo_a);
        let messenger_b = TopologyMessenger::new(id_b, topo_b);
        let messenger_c = TopologyMessenger::new(id_c, topo_c);

        let mut rx_b = messenger_b.incoming();
        let mut rx_c = messenger_c.incoming();

        messenger_a.add_connection(id_b, conn_ab_a).await;
        messenger_b.add_connection(id_a, conn_ab_b).await;
        messenger_a.add_connection(id_c, conn_ac_a).await;
        messenger_c.add_connection(id_a, conn_ac_c).await;

        messenger_a
            .broadcast(MessageType::TopologySync, b"sync")
            .await
            .unwrap();

        let env_b = rx_b.recv().await.unwrap();
        let env_c = rx_c.recv().await.unwrap();
        assert_eq!(env_b.source, id_a);
        assert_eq!(env_c.source, id_a);
        assert_eq!(env_b.payload, b"sync");
        assert_eq!(env_c.payload, b"sync");
    }

    #[tokio::test(start_paused = true)]
    async fn test_multi_hop_forward() {
        let network = SimBleNetwork::new();
        let (conn_ab_a, conn_ab_b) = make_connection(&network).await;
        let (conn_bc_b, conn_bc_c) = make_connection(&network).await;

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();

        // All topologies have the full graph: A↔B↔C
        let make_topo = || {
            let mut t = EnsembleTopology::new();
            t.add_edge(make_sim_edge(id_a, id_b));
            t.add_edge(make_sim_edge(id_b, id_a));
            t.add_edge(make_sim_edge(id_b, id_c));
            t.add_edge(make_sim_edge(id_c, id_b));
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, make_topo());
        let messenger_b = TopologyMessenger::new(id_b, make_topo());
        let messenger_c = TopologyMessenger::new(id_c, make_topo());

        let mut rx_c = messenger_c.incoming();

        // A↔B
        messenger_a.add_connection(id_b, conn_ab_a).await;
        messenger_b.add_connection(id_a, conn_ab_b).await;
        // B↔C
        messenger_b.add_connection(id_c, conn_bc_b).await;
        messenger_c.add_connection(id_b, conn_bc_c).await;

        // A sends unicast to C — must be forwarded by B
        messenger_a
            .send_to(id_c, MessageType::FlowSync, b"multi-hop")
            .await
            .unwrap();

        let envelope = rx_c.recv().await.unwrap();
        assert_eq!(envelope.source, id_a);
        assert_eq!(envelope.destination, id_c);
        assert_eq!(envelope.payload, b"multi-hop");
    }

    #[tokio::test(start_paused = true)]
    async fn test_unreachable_returns_error() {
        let id_a = Uuid::new_v4();
        let unknown = Uuid::new_v4();

        let topo = Arc::new(RwLock::new(EnsembleTopology::new()));
        let messenger = TopologyMessenger::new(id_a, topo);

        let result = messenger
            .send_to(unknown, MessageType::FlowSync, b"test")
            .await;
        assert!(matches!(result, Err(RoutingError::Unreachable(_))));
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_drop_cleanup() {
        let network = SimBleNetwork::new();
        let (conn_a, conn_b) = make_connection(&network).await;

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        let topo_a = {
            let mut t = EnsembleTopology::new();
            t.add_edge(make_sim_edge(id_a, id_b));
            t.add_edge(make_sim_edge(id_b, id_a));
            Arc::new(RwLock::new(t))
        };

        let messenger_a = TopologyMessenger::new(id_a, topo_a.clone());

        messenger_a.add_connection(id_b, conn_a).await;
        assert_eq!(messenger_a.connection_count(), 1);

        // Drop the peer's connection — the underlying mpsc Sender is dropped,
        // causing conn_a's recv to return None (Disconnected).
        drop(conn_b);

        // Give the recv loop time to detect disconnection and clean up.
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(messenger_a.connection_count(), 0);

        let topo = topo_a.read().await;
        assert_eq!(topo.edge_count(), 0);
    }
}
