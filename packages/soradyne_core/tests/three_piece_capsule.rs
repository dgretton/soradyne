//! Phase 6.1: Three-piece capsule integration test
//!
//! Tests the full lifecycle of a 3-device capsule with DripHostedFlow sync
//! over SimBleNetwork. Topology: Mac↔Phone, Mac↔Accessory (Phone↔Accessory
//! only via multi-hop through Mac).
//!
//! Run with:
//!   cargo test --test three_piece_capsule --no-default-features

use std::sync::Arc;
use std::time::Duration;

use soradyne::ble::gatt::MessageType;
use soradyne::ble::simulated::SimBleNetwork;
use soradyne::ble::transport::{BleConnection, BleCentral, BlePeripheral};
use soradyne::convergent::inventory::{InventorySchema, InventoryState};
use soradyne::convergent::{Horizon, OpEnvelope, Operation, Value};
use soradyne::flow::{
    DripHostPolicy, DripHostedFlow, Flow, FlowConfig, FlowSyncMessage, HostSelectionStrategy,
};
use soradyne::topology::ensemble::{
    ConnectionQuality, EnsembleTopology, PieceReachability, TopologyEdge, TransportType,
};
use soradyne::topology::messenger::TopologyMessenger;

use tokio::sync::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_sim_edge(from: Uuid, to: Uuid) -> TopologyEdge {
    TopologyEdge {
        from,
        to,
        transport: TransportType::SimulatedBle,
        quality: ConnectionQuality::unknown(),
    }
}

async fn make_connection(
    network: &Arc<SimBleNetwork>,
) -> (Arc<dyn BleConnection>, Arc<dyn BleConnection>) {
    let mut device_a = network.create_device();
    device_a.set_mtu(4096); // large enough for CBOR-encoded OpEnvelopes
    let device_b = network.create_device();
    let addr_b = device_b.address().clone();

    device_b.start_advertising(vec![0x01]).await.unwrap();
    let accept_handle = tokio::spawn(async move { device_b.accept().await.unwrap() });

    let conn_a = device_a.connect(&addr_b).await.unwrap();
    let conn_b = accept_handle.await.unwrap();

    (Arc::from(conn_a), Arc::from(conn_b))
}

fn make_flow_config(flow_id: Uuid) -> FlowConfig {
    FlowConfig {
        id: flow_id,
        type_name: "drip_hosted:inventory".into(),
        params: serde_json::json!({}),
    }
}

/// Build the 3-piece topology: Mac↔Phone, Mac↔Accessory (bidirectional).
/// Phone and Accessory are NOT directly connected.
fn make_topology(mac: Uuid, phone: Uuid, accessory: Uuid) -> EnsembleTopology {
    let mut t = EnsembleTopology::new();
    t.add_edge(make_sim_edge(mac, phone));
    t.add_edge(make_sim_edge(phone, mac));
    t.add_edge(make_sim_edge(mac, accessory));
    t.add_edge(make_sim_edge(accessory, mac));
    t
}

/// Materialize the inventory state from a flow's document.
fn read_inventory(flow: &DripHostedFlow<InventorySchema>) -> InventoryState {
    let doc = flow.document().read().unwrap();
    InventoryState::from_document_state(&doc.materialize())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Step 7: Verify that the topology correctly shows Phone↔Accessory as
/// indirect (routed through Mac).
#[tokio::test]
async fn test_topology_reachability() {
    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();
    let accessory_id = Uuid::new_v4();

    let topo = make_topology(mac_id, phone_id, accessory_id);

    // Mac can reach both directly
    assert_eq!(
        topo.compute_reachability(&mac_id, &phone_id),
        Some(PieceReachability::Direct)
    );
    assert_eq!(
        topo.compute_reachability(&mac_id, &accessory_id),
        Some(PieceReachability::Direct)
    );

    // Phone reaches Accessory indirectly via Mac
    assert_eq!(
        topo.compute_reachability(&phone_id, &accessory_id),
        Some(PieceReachability::Indirect {
            next_hop: mac_id,
            hop_count: 2,
        })
    );

    // Accessory reaches Phone indirectly via Mac
    assert_eq!(
        topo.compute_reachability(&accessory_id, &phone_id),
        Some(PieceReachability::Indirect {
            next_hop: mac_id,
            hop_count: 2,
        })
    );
}

/// Step 10: Full data sync across 3 pieces.
/// Phone applies an edit → broadcasts → Mac receives → Accessory receives
/// (via Mac forwarding).
#[tokio::test]
async fn test_three_piece_data_sync() {
    let network = SimBleNetwork::new();
    let flow_id = Uuid::new_v4();

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();
    let accessory_id = Uuid::new_v4();

    // Each piece gets its own topology view (same content)
    let topo_mac = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_phone = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_acc = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));

    // Create messengers
    let messenger_mac = TopologyMessenger::new(mac_id, topo_mac.clone());
    let messenger_phone = TopologyMessenger::new(phone_id, topo_phone.clone());
    let messenger_acc = TopologyMessenger::new(accessory_id, topo_acc.clone());

    // Subscribe to incoming BEFORE wiring connections
    let mut rx_mac = messenger_mac.incoming();
    let mut rx_acc = messenger_acc.incoming();

    // Wire BLE connections: Mac↔Phone, Mac↔Accessory
    let (conn_mp_mac, conn_mp_phone) = make_connection(&network).await;
    let (conn_ma_mac, conn_ma_acc) = make_connection(&network).await;

    messenger_mac.add_connection(phone_id, conn_mp_mac).await;
    messenger_phone.add_connection(mac_id, conn_mp_phone).await;
    messenger_mac.add_connection(accessory_id, conn_ma_mac).await;
    messenger_acc.add_connection(mac_id, conn_ma_acc).await;

    // Create flows (manual message processing — no start())
    let flow_mac = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        mac_id,
        "Mac".into(),
    );
    let flow_phone = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        phone_id,
        "Phone".into(),
    );
    let flow_acc = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        accessory_id,
        "Accessory".into(),
    );

    // Phone applies a local edit
    let envelope = {
        let mut doc = flow_phone.document().write().unwrap();
        let e1 = doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        let e2 = doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        let e3 = doc.apply_local(Operation::set_field(
            "item_1",
            "category",
            Value::string("Tools"),
        ));
        let e4 = doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Garage"),
        ));
        vec![e1, e2, e3, e4]
    };

    // Verify Phone has the item locally
    let phone_state = read_inventory(&flow_phone);
    assert_eq!(phone_state.items.len(), 1);
    assert_eq!(
        phone_state.items.get("item_1").unwrap().description,
        "Hammer"
    );

    // Phone broadcasts its operations
    let msg = FlowSyncMessage::OperationBatch {
        flow_id,
        ops: envelope,
    };
    let payload = msg.to_cbor().unwrap();
    messenger_phone
        .broadcast(MessageType::FlowSync, &payload)
        .await
        .unwrap();

    // Mac receives (direct connection from Phone)
    let env_mac = tokio::time::timeout(Duration::from_millis(500), rx_mac.recv())
        .await
        .expect("Mac should receive within timeout")
        .expect("Mac recv should succeed");

    let msg_mac = FlowSyncMessage::from_cbor(&env_mac.payload).unwrap();
    flow_mac.handle_flow_sync(env_mac.source, msg_mac);

    // Verify Mac now has the item
    let mac_state = read_inventory(&flow_mac);
    assert_eq!(mac_state.items.len(), 1, "Mac should have 1 item");
    assert_eq!(
        mac_state.items.get("item_1").unwrap().description,
        "Hammer"
    );

    // Accessory receives (via Mac forwarding the broadcast)
    let env_acc = tokio::time::timeout(Duration::from_millis(500), rx_acc.recv())
        .await
        .expect("Accessory should receive within timeout")
        .expect("Accessory recv should succeed");

    assert_eq!(
        env_acc.source, phone_id,
        "Source should be Phone (original sender)"
    );

    let msg_acc = FlowSyncMessage::from_cbor(&env_acc.payload).unwrap();
    flow_acc.handle_flow_sync(env_acc.source, msg_acc);

    // Verify Accessory now has the same item
    let acc_state = read_inventory(&flow_acc);
    assert_eq!(acc_state.items.len(), 1, "Accessory should have 1 item");
    assert_eq!(
        acc_state.items.get("item_1").unwrap().description,
        "Hammer"
    );
    assert_eq!(
        acc_state.items.get("item_1").unwrap().category,
        "Tools"
    );
    assert_eq!(
        acc_state.items.get("item_1").unwrap().location,
        "Garage"
    );
}

/// Step 13: Multi-hop unicast. Phone sends directly to Accessory — the
/// message is routed through Mac transparently.
#[tokio::test]
async fn test_multi_hop_unicast() {
    let network = SimBleNetwork::new();
    let flow_id = Uuid::new_v4();

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();
    let accessory_id = Uuid::new_v4();

    let topo_mac = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_phone = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_acc = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));

    let messenger_mac = TopologyMessenger::new(mac_id, topo_mac);
    let messenger_phone = TopologyMessenger::new(phone_id, topo_phone);
    let messenger_acc = TopologyMessenger::new(accessory_id, topo_acc);

    let mut rx_acc = messenger_acc.incoming();

    // Wire: Mac↔Phone, Mac↔Accessory
    let (conn_mp_mac, conn_mp_phone) = make_connection(&network).await;
    let (conn_ma_mac, conn_ma_acc) = make_connection(&network).await;

    messenger_mac.add_connection(phone_id, conn_mp_mac).await;
    messenger_phone.add_connection(mac_id, conn_mp_phone).await;
    messenger_mac.add_connection(accessory_id, conn_ma_mac).await;
    messenger_acc.add_connection(mac_id, conn_ma_acc).await;

    // Create flow on Accessory
    let flow_acc = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        accessory_id,
        "Accessory".into(),
    );

    // Phone sends a unicast message to Accessory (routed through Mac)
    let op_env = OpEnvelope::new(
        "Phone".into(),
        1,
        Horizon::new(),
        Operation::add_item("item_hop", "InventoryItem"),
    );
    let msg = FlowSyncMessage::OperationBatch {
        flow_id,
        ops: vec![op_env],
    };
    let payload = msg.to_cbor().unwrap();

    messenger_phone
        .send_to(accessory_id, MessageType::FlowSync, &payload)
        .await
        .unwrap();

    // Accessory should receive the message via Mac's forwarding
    let env_acc = tokio::time::timeout(Duration::from_millis(500), rx_acc.recv())
        .await
        .expect("Accessory should receive unicast via Mac")
        .expect("Accessory recv should succeed");

    assert_eq!(env_acc.source, phone_id);
    assert_eq!(env_acc.destination, accessory_id);

    let msg_acc = FlowSyncMessage::from_cbor(&env_acc.payload).unwrap();
    flow_acc.handle_flow_sync(env_acc.source, msg_acc);

    let acc_state = read_inventory(&flow_acc);
    assert!(
        acc_state.items.contains_key("item_hop"),
        "Accessory should have item_hop from multi-hop unicast"
    );
}

/// Step 9: Host assignment via BestConnected. Mac is the hub (connected to
/// both Phone and Accessory), so it should be selected as host.
#[tokio::test]
async fn test_host_assignment_best_connected() {
    use soradyne::topology::capsule::PieceCapabilities;
    use std::collections::HashMap;

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();
    let accessory_id = Uuid::new_v4();
    let flow_id = Uuid::new_v4();

    let topo = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));

    let flow = DripHostedFlow::<InventorySchema>::new_with_ensemble(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy {
            selection: HostSelectionStrategy::BestConnected,
            ..Default::default()
        },
        mac_id,
        "Mac".into(),
        TopologyMessenger::new(mac_id, topo.clone()),
        topo,
    );

    let mut caps = HashMap::new();
    caps.insert(
        mac_id,
        PieceCapabilities {
            can_host_drip: true,
            can_memorize: true,
            can_route: true,
            has_ui: true,
            battery_aware: false,
            storage_bytes: 1_000_000_000,
        },
    );
    caps.insert(
        phone_id,
        PieceCapabilities {
            can_host_drip: true,
            can_memorize: true,
            can_route: true,
            has_ui: true,
            battery_aware: true,
            storage_bytes: 500_000_000,
        },
    );
    caps.insert(
        accessory_id,
        PieceCapabilities {
            can_host_drip: false, // Accessory can't host
            can_memorize: true,
            can_route: true,
            has_ui: false,
            battery_aware: false,
            storage_bytes: 100_000_000,
        },
    );

    let selected = flow.evaluate_host_assignment(&caps);
    assert_eq!(
        selected,
        Some(mac_id),
        "Mac should be selected as host (most edges: 4 vs Phone's 2)"
    );

    // Verify Accessory is never selected (can_host_drip = false)
    let mut caps_no_mac = caps.clone();
    caps_no_mac.remove(&mac_id);
    let selected_fallback = flow.evaluate_host_assignment(&caps_no_mac);
    assert_eq!(
        selected_fallback,
        Some(phone_id),
        "Without Mac, Phone should be selected (only eligible piece)"
    );
}

/// Step 11: Offline merge failover — all pieces continue accepting edits
/// independently and converge when ops are exchanged.
#[tokio::test]
async fn test_offline_merge_failover() {
    let flow_id = Uuid::new_v4();

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();

    // Both flows start disconnected (simulating host going offline)
    let flow_mac = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        mac_id,
        "Mac".into(),
    );
    let flow_phone = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        phone_id,
        "Phone".into(),
    );

    // Mac is host, then goes offline. Both make independent edits.
    flow_mac.become_host().unwrap();

    // Mac adds item_A
    flow_mac.apply_edit(Operation::add_item("item_A", "InventoryItem")).unwrap();
    flow_mac
        .apply_edit(Operation::set_field("item_A", "description", Value::string("Wrench")))
        .unwrap();

    // Phone adds item_B (offline, OfflineMerge allows this)
    flow_phone.apply_edit(Operation::add_item("item_B", "InventoryItem")).unwrap();
    flow_phone
        .apply_edit(Operation::set_field("item_B", "description", Value::string("Pliers")))
        .unwrap();

    // Verify they have different items
    let mac_state = read_inventory(&flow_mac);
    let phone_state = read_inventory(&flow_phone);
    assert_eq!(mac_state.items.len(), 1);
    assert_eq!(phone_state.items.len(), 1);
    assert!(mac_state.items.contains_key("item_A"));
    assert!(phone_state.items.contains_key("item_B"));

    // Simulate reconnection: exchange all operations
    let mac_ops: Vec<OpEnvelope> = flow_mac
        .document()
        .read()
        .unwrap()
        .all_operations()
        .cloned()
        .collect();
    let phone_ops: Vec<OpEnvelope> = flow_phone
        .document()
        .read()
        .unwrap()
        .all_operations()
        .cloned()
        .collect();

    // Mac applies Phone's ops
    {
        let mut doc = flow_mac.document().write().unwrap();
        for op in &phone_ops {
            doc.apply_remote(op.clone());
        }
    }
    // Phone applies Mac's ops
    {
        let mut doc = flow_phone.document().write().unwrap();
        for op in &mac_ops {
            doc.apply_remote(op.clone());
        }
    }

    // Both should now have both items
    let mac_state = read_inventory(&flow_mac);
    let phone_state = read_inventory(&flow_phone);

    assert_eq!(mac_state.items.len(), 2, "Mac should have 2 items after merge");
    assert_eq!(phone_state.items.len(), 2, "Phone should have 2 items after merge");
    assert_eq!(
        mac_state.items.get("item_A").unwrap().description,
        "Wrench"
    );
    assert_eq!(
        mac_state.items.get("item_B").unwrap().description,
        "Pliers"
    );
    assert_eq!(
        phone_state.items.get("item_A").unwrap().description,
        "Wrench"
    );
    assert_eq!(
        phone_state.items.get("item_B").unwrap().description,
        "Pliers"
    );
}

/// Step 12: Accessory serves cached state to a reconnecting piece.
/// Accessory caches operations it received; when Mac reconnects, it gets
/// the full state from Accessory.
#[tokio::test]
async fn test_accessory_cached_state() {
    use soradyne::flow::AccessoryMemorizer;

    let flow_id = Uuid::new_v4();
    let _accessory_id = Uuid::new_v4();
    let mac_id = Uuid::new_v4();

    // Accessory has a memorizer that caches ops
    let memorizer = AccessoryMemorizer::new(flow_id, 1000);

    // Simulate ops that the accessory received and cached
    let op1 = OpEnvelope::new(
        "Phone".into(),
        1,
        Horizon::new(),
        Operation::add_item("item_cached", "InventoryItem"),
    );
    let op2 = OpEnvelope::new(
        "Phone".into(),
        2,
        Horizon::new(),
        Operation::set_field("item_cached", "description", Value::string("Cached Hammer")),
    );

    memorizer.cache_operation(op1.clone());
    memorizer.cache_operation(op2.clone());

    assert_eq!(memorizer.cached_count(), 2);

    // Mac reconnects with an empty horizon (it missed everything)
    let mac_horizon = Horizon::new();
    let missing_ops = memorizer.operations_since(&mac_horizon);
    assert_eq!(missing_ops.len(), 2, "Mac should need 2 ops from cache");

    // Mac creates a flow and applies the cached ops
    let flow_mac = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        mac_id,
        "Mac".into(),
    );
    {
        let mut doc = flow_mac.document().write().unwrap();
        for op in missing_ops {
            doc.apply_remote(op);
        }
    }

    let mac_state = read_inventory(&flow_mac);
    assert_eq!(mac_state.items.len(), 1);
    assert_eq!(
        mac_state.items.get("item_cached").unwrap().description,
        "Cached Hammer"
    );

    // Deduplication: caching the same ops again should not increase count
    memorizer.cache_operation(op1);
    memorizer.cache_operation(op2);
    assert_eq!(memorizer.cached_count(), 2, "Deduplication should prevent double-caching");
}

/// Full lifecycle: Host announcement propagation through the ensemble.
/// Mac claims host → broadcasts → Phone and Accessory see the announcement.
#[tokio::test]
async fn test_host_announcement_propagation() {
    let network = SimBleNetwork::new();
    let flow_id = Uuid::new_v4();

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();
    let accessory_id = Uuid::new_v4();

    let topo_mac = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_phone = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));
    let topo_acc = Arc::new(RwLock::new(make_topology(mac_id, phone_id, accessory_id)));

    let messenger_mac = TopologyMessenger::new(mac_id, topo_mac.clone());
    let messenger_phone = TopologyMessenger::new(phone_id, topo_phone.clone());
    let messenger_acc = TopologyMessenger::new(accessory_id, topo_acc.clone());

    let mut rx_phone = messenger_phone.incoming();
    let mut rx_acc = messenger_acc.incoming();

    // Wire connections
    let (conn_mp_mac, conn_mp_phone) = make_connection(&network).await;
    let (conn_ma_mac, conn_ma_acc) = make_connection(&network).await;

    messenger_mac.add_connection(phone_id, conn_mp_mac).await;
    messenger_phone.add_connection(mac_id, conn_mp_phone).await;
    messenger_mac.add_connection(accessory_id, conn_ma_mac).await;
    messenger_acc.add_connection(mac_id, conn_ma_acc).await;

    // Create flows with ensemble wiring
    let mut flow_mac = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        mac_id,
        "Mac".into(),
    );
    flow_mac.set_ensemble(messenger_mac.clone(), topo_mac);

    let flow_phone = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        phone_id,
        "Phone".into(),
    );
    let flow_acc = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        accessory_id,
        "Accessory".into(),
    );

    // Mac claims host — this broadcasts a HostAnnouncement
    flow_mac.become_host().unwrap();

    // Give the spawned broadcast task time to execute
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Phone receives the host announcement
    let env_phone = tokio::time::timeout(Duration::from_millis(500), rx_phone.recv())
        .await
        .expect("Phone should receive host announcement")
        .expect("Phone recv should succeed");

    let msg_phone = FlowSyncMessage::from_cbor(&env_phone.payload).unwrap();
    flow_phone.handle_flow_sync(env_phone.source, msg_phone);

    // Accessory receives (forwarded by Mac)
    let env_acc = tokio::time::timeout(Duration::from_millis(500), rx_acc.recv())
        .await
        .expect("Accessory should receive host announcement")
        .expect("Accessory recv should succeed");

    let msg_acc = FlowSyncMessage::from_cbor(&env_acc.payload).unwrap();
    flow_acc.handle_flow_sync(env_acc.source, msg_acc);

    // Both Phone and Accessory should now know Mac is the host
    let phone_ha = flow_phone.host_assignment().read().unwrap();
    assert_eq!(
        phone_ha.current_host,
        Some(mac_id),
        "Phone should see Mac as host"
    );
    assert_eq!(phone_ha.epoch, 1);

    let acc_ha = flow_acc.host_assignment().read().unwrap();
    assert_eq!(
        acc_ha.current_host,
        Some(mac_id),
        "Accessory should see Mac as host"
    );
    assert_eq!(acc_ha.epoch, 1);

    // Mac should know it's the host
    assert!(flow_mac.is_current_host(), "Mac should be current host");
}

/// Bidirectional sync: Mac edits, broadcasts; Phone edits, broadcasts.
/// Both converge to the same state.
#[tokio::test]
async fn test_bidirectional_convergence() {
    let network = SimBleNetwork::new();
    let flow_id = Uuid::new_v4();

    let mac_id = Uuid::new_v4();
    let phone_id = Uuid::new_v4();

    let make_topo = || {
        let mut t = EnsembleTopology::new();
        t.add_edge(make_sim_edge(mac_id, phone_id));
        t.add_edge(make_sim_edge(phone_id, mac_id));
        Arc::new(RwLock::new(t))
    };

    let messenger_mac = TopologyMessenger::new(mac_id, make_topo());
    let messenger_phone = TopologyMessenger::new(phone_id, make_topo());

    let mut rx_mac = messenger_mac.incoming();
    let mut rx_phone = messenger_phone.incoming();

    let (conn_mac, conn_phone) = make_connection(&network).await;
    messenger_mac.add_connection(phone_id, conn_mac).await;
    messenger_phone.add_connection(mac_id, conn_phone).await;

    let flow_mac = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        mac_id,
        "Mac".into(),
    );
    let flow_phone = DripHostedFlow::<InventorySchema>::new(
        make_flow_config(flow_id),
        InventorySchema,
        DripHostPolicy::default(),
        phone_id,
        "Phone".into(),
    );

    // Mac creates item_X
    let mac_ops = {
        let mut doc = flow_mac.document().write().unwrap();
        vec![
            doc.apply_local(Operation::add_item("item_X", "InventoryItem")),
            doc.apply_local(Operation::set_field(
                "item_X",
                "description",
                Value::string("Drill"),
            )),
        ]
    };

    // Phone creates item_Y
    let phone_ops = {
        let mut doc = flow_phone.document().write().unwrap();
        vec![
            doc.apply_local(Operation::add_item("item_Y", "InventoryItem")),
            doc.apply_local(Operation::set_field(
                "item_Y",
                "description",
                Value::string("Saw"),
            )),
        ]
    };

    // Mac broadcasts its ops
    let msg_mac = FlowSyncMessage::OperationBatch {
        flow_id,
        ops: mac_ops,
    };
    messenger_mac
        .broadcast(MessageType::FlowSync, &msg_mac.to_cbor().unwrap())
        .await
        .unwrap();

    // Phone broadcasts its ops
    let msg_phone = FlowSyncMessage::OperationBatch {
        flow_id,
        ops: phone_ops,
    };
    messenger_phone
        .broadcast(MessageType::FlowSync, &msg_phone.to_cbor().unwrap())
        .await
        .unwrap();

    // Phone receives Mac's broadcast
    let env = tokio::time::timeout(Duration::from_millis(500), rx_phone.recv())
        .await
        .unwrap()
        .unwrap();
    flow_phone.handle_flow_sync(env.source, FlowSyncMessage::from_cbor(&env.payload).unwrap());

    // Mac receives Phone's broadcast
    let env = tokio::time::timeout(Duration::from_millis(500), rx_mac.recv())
        .await
        .unwrap()
        .unwrap();
    flow_mac.handle_flow_sync(env.source, FlowSyncMessage::from_cbor(&env.payload).unwrap());

    // Both should have both items
    let mac_state = read_inventory(&flow_mac);
    let phone_state = read_inventory(&flow_phone);

    assert_eq!(mac_state.items.len(), 2, "Mac should have 2 items");
    assert_eq!(phone_state.items.len(), 2, "Phone should have 2 items");

    assert_eq!(mac_state.items.get("item_X").unwrap().description, "Drill");
    assert_eq!(mac_state.items.get("item_Y").unwrap().description, "Saw");
    assert_eq!(phone_state.items.get("item_X").unwrap().description, "Drill");
    assert_eq!(phone_state.items.get("item_Y").unwrap().description, "Saw");
}
