//! TCP transport integration test
//!
//! Tests 2-piece CRDT sync over TCP connections, wired through
//! TopologyMessenger with TcpDirect transport edges.
//!
//! Run with:
//!   cargo test --test tcp_transport_sync --features tcp-transport --no-default-features

#![cfg(feature = "tcp-transport")]

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use soradyne::ble::gatt::MessageType;
use soradyne::ble::tcp_transport::{TcpCentral, TcpPeripheral};
use soradyne::ble::transport::{BleAddress, BleCentral, BleConnection, BlePeripheral};
use soradyne::topology::ensemble::{
    ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
};
use soradyne::topology::messenger::TopologyMessenger;

use tokio::sync::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn localhost_any_port() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
}

fn make_tcp_edge(from: Uuid, to: Uuid) -> TopologyEdge {
    TopologyEdge {
        from,
        to,
        transport: TransportType::TcpDirect,
        quality: ConnectionQuality::unknown(),
    }
}

/// Establish a TCP connection pair: central connects to peripheral.
async fn make_tcp_connection() -> (Arc<dyn BleConnection>, Arc<dyn BleConnection>) {
    let peripheral = TcpPeripheral::new(localhost_any_port());
    peripheral.start_advertising(vec![]).await.unwrap();
    let addr = peripheral.local_addr().await.unwrap();

    let central = TcpCentral::new();
    let tcp_addr = BleAddress::Tcp(addr);

    let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

    let conn_c = central.connect(&tcp_addr).await.unwrap();
    let conn_p = accept_handle.await.unwrap();

    (Arc::from(conn_c), Arc::from(conn_p))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Two messengers exchanging RoutedEnvelope messages over TCP.
#[tokio::test]
async fn test_tcp_messenger_direct_send() {
    let (conn_a, conn_b) = make_tcp_connection().await;

    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();

    let make_topo = || {
        let mut t = EnsembleTopology::new();
        t.add_edge(make_tcp_edge(id_a, id_b));
        t.add_edge(make_tcp_edge(id_b, id_a));
        Arc::new(RwLock::new(t))
    };

    let messenger_a = TopologyMessenger::new(id_a, make_topo());
    let messenger_b = TopologyMessenger::new(id_b, make_topo());

    let mut rx_b = messenger_b.incoming();

    messenger_a.add_connection(id_b, conn_a).await;
    messenger_b.add_connection(id_a, conn_b).await;

    messenger_a
        .send_to(id_b, MessageType::FlowSync, b"tcp-hello")
        .await
        .unwrap();

    let envelope = rx_b.recv().await.unwrap();
    assert_eq!(envelope.source, id_a);
    assert_eq!(envelope.destination, id_b);
    assert_eq!(envelope.payload, b"tcp-hello");
    assert_eq!(envelope.message_type, MessageType::FlowSync);
}

/// Broadcast over TCP connections.
#[tokio::test]
async fn test_tcp_messenger_broadcast() {
    let (conn_ab_a, conn_ab_b) = make_tcp_connection().await;
    let (conn_ac_a, conn_ac_c) = make_tcp_connection().await;

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
        .broadcast(MessageType::TopologySync, b"tcp-sync")
        .await
        .unwrap();

    let env_b = rx_b.recv().await.unwrap();
    let env_c = rx_c.recv().await.unwrap();
    assert_eq!(env_b.source, id_a);
    assert_eq!(env_c.source, id_a);
    assert_eq!(env_b.payload, b"tcp-sync");
    assert_eq!(env_c.payload, b"tcp-sync");
}

/// Multi-hop forwarding over TCP: A→B→C.
#[tokio::test]
async fn test_tcp_multi_hop_forward() {
    let (conn_ab_a, conn_ab_b) = make_tcp_connection().await;
    let (conn_bc_b, conn_bc_c) = make_tcp_connection().await;

    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let id_c = Uuid::new_v4();

    let make_topo = || {
        let mut t = EnsembleTopology::new();
        t.add_edge(make_tcp_edge(id_a, id_b));
        t.add_edge(make_tcp_edge(id_b, id_a));
        t.add_edge(make_tcp_edge(id_b, id_c));
        t.add_edge(make_tcp_edge(id_c, id_b));
        Arc::new(RwLock::new(t))
    };

    let messenger_a = TopologyMessenger::new(id_a, make_topo());
    let messenger_b = TopologyMessenger::new(id_b, make_topo());
    let messenger_c = TopologyMessenger::new(id_c, make_topo());

    let mut rx_c = messenger_c.incoming();

    messenger_a.add_connection(id_b, conn_ab_a).await;
    messenger_b.add_connection(id_a, conn_ab_b).await;
    messenger_b.add_connection(id_c, conn_bc_b).await;
    messenger_c.add_connection(id_b, conn_bc_c).await;

    messenger_a
        .send_to(id_c, MessageType::FlowSync, b"tcp-multi-hop")
        .await
        .unwrap();

    let envelope = rx_c.recv().await.unwrap();
    assert_eq!(envelope.source, id_a);
    assert_eq!(envelope.destination, id_c);
    assert_eq!(envelope.payload, b"tcp-multi-hop");
}

/// Connection cleanup on TCP disconnect.
#[tokio::test]
async fn test_tcp_connection_cleanup() {
    let (conn_a, conn_b) = make_tcp_connection().await;

    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();

    let topo_a = {
        let mut t = EnsembleTopology::new();
        t.add_edge(make_tcp_edge(id_a, id_b));
        t.add_edge(make_tcp_edge(id_b, id_a));
        Arc::new(RwLock::new(t))
    };

    let messenger_a = TopologyMessenger::new(id_a, topo_a.clone());
    messenger_a.add_connection(id_b, conn_a).await;
    assert_eq!(messenger_a.connection_count(), 1);

    // Drop peer side — messenger should detect disconnect and clean up.
    drop(conn_b);
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(messenger_a.connection_count(), 0);
    let topo = topo_a.read().await;
    assert_eq!(topo.edge_count(), 0);
}

/// Large CBOR-encoded payload over TCP (simulates real CRDT sync).
#[tokio::test]
async fn test_tcp_large_payload_sync() {
    let (conn_a, conn_b) = make_tcp_connection().await;

    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();

    let make_topo = || {
        let mut t = EnsembleTopology::new();
        t.add_edge(make_tcp_edge(id_a, id_b));
        t.add_edge(make_tcp_edge(id_b, id_a));
        Arc::new(RwLock::new(t))
    };

    let messenger_a = TopologyMessenger::new(id_a, make_topo());
    let messenger_b = TopologyMessenger::new(id_b, make_topo());

    let mut rx_b = messenger_b.incoming();

    messenger_a.add_connection(id_b, conn_a).await;
    messenger_b.add_connection(id_a, conn_b).await;

    // Simulate a large CRDT state payload (50KB)
    let large_payload = vec![0xCDu8; 50_000];
    messenger_a
        .send_to(id_b, MessageType::FlowSync, &large_payload)
        .await
        .unwrap();

    let envelope = rx_b.recv().await.unwrap();
    assert_eq!(envelope.payload.len(), 50_000);
    assert_eq!(envelope.payload, large_payload);
}
