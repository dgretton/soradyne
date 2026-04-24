//! Integration test: two-device sync over TCP static peers.
//!
//! Exercises the full path that `soradyne-cli sync` uses:
//!   DeviceIdentity → CapsuleKeyBundle → EnsembleManager (with static peers)
//!   → ConvergentFlow → set_ensemble → start → horizon exchange → ops sync
//!
//! Both "devices" run in the same process on localhost, eliminating the
//! need for two machines and manual binary copying during development.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use soradyne::ble::simulated::SimBleNetwork;
use soradyne::convergent::{DeviceId, Operation, Value};
use soradyne::ffi::convergent_flow_ffi::ConvergentFlow;
use soradyne::identity::{CapsuleKeyBundle, DeviceIdentity};
use soradyne::topology::manager::{EnsembleConfig, EnsembleManager};

/// Create two devices in one capsule, each with an EnsembleManager using
/// TCP static peers on localhost, open the same flow on both, write ops
/// on device A, and verify they arrive on device B.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_tcp_static_peer_flow_sync() {
    // --- Setup: two identities, one capsule ---

    let identity_a = DeviceIdentity::generate();
    let identity_b = DeviceIdentity::generate();
    let id_a = identity_a.device_id();
    let id_b = identity_b.device_id();

    let capsule_id = Uuid::new_v4();
    let keys = CapsuleKeyBundle::generate(capsule_id);

    let piece_ids = vec![id_a, id_b];

    // Peer static keys: each side needs the other's DH public key
    let peer_keys_a: HashMap<Uuid, [u8; 32]> =
        [(id_b, identity_b.dh_public_bytes())].into();
    let peer_keys_b: HashMap<Uuid, [u8; 32]> =
        [(id_a, identity_a.dh_public_bytes())].into();

    // Static peer addresses: both sides use port 17171. The UUID
    // tiebreaker (higher UUID listens) determines roles — only one
    // side binds the listener, the other connects.
    let port: u16 = 17171;
    let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();

    let static_peers_a: HashMap<Uuid, std::net::SocketAddr> =
        [(id_b, addr)].into();
    let static_peers_b: HashMap<Uuid, std::net::SocketAddr> =
        [(id_a, addr)].into();

    let config_a = EnsembleConfig {
        static_peers: static_peers_a,
        ..EnsembleConfig::default()
    };
    let config_b = EnsembleConfig {
        static_peers: static_peers_b,
        ..EnsembleConfig::default()
    };

    // --- Create EnsembleManagers ---

    // Separate sim networks so the two managers can't see each other via BLE —
    // forces all communication through TCP static peers.
    let sim_a = SimBleNetwork::new();
    let sim_b = SimBleNetwork::new();

    let manager_a = EnsembleManager::new(
        Arc::new(identity_a),
        keys.clone(),
        piece_ids.clone(),
        peer_keys_a,
        config_a,
    );
    let manager_b = EnsembleManager::new(
        Arc::new(identity_b),
        keys.clone(),
        piece_ids.clone(),
        peer_keys_b,
        config_b,
    );

    // Start ensembles (isolated sim BLE + TCP static peers)
    let (central_a, peripheral_a) = (sim_a.create_device(), sim_a.create_device());
    let (central_b, peripheral_b) = (sim_b.create_device(), sim_b.create_device());
    manager_a.start(Arc::new(central_a), Arc::new(peripheral_a)).await;
    manager_b.start(Arc::new(central_b), Arc::new(peripheral_b)).await;

    // Give TCP listeners time to bind and connect
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify TCP connection established
    eprintln!("Manager A topology: {:?}",
        manager_a.topology().read().await.online_pieces.keys().collect::<Vec<_>>());
    eprintln!("Manager B topology: {:?}",
        manager_b.topology().read().await.online_pieces.keys().collect::<Vec<_>>());

    // --- Create flows ---

    let flow_uuid = Uuid::new_v4();
    let device_id_a: DeviceId = id_a.to_string();
    let device_id_b: DeviceId = id_b.to_string();

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();

    let mut flow_a = ConvergentFlow::new_persistent(
        device_id_a.clone(),
        tmp_a.path().to_path_buf(),
        flow_uuid,
    );

    let mut flow_b = ConvergentFlow::new_persistent(
        device_id_b.clone(),
        tmp_b.path().to_path_buf(),
        flow_uuid,
    );

    // Wire flows to their respective ensembles
    flow_a.set_ensemble(
        Arc::clone(manager_a.messenger()),
        Arc::clone(manager_a.topology()),
    );
    flow_b.set_ensemble(
        Arc::clone(manager_b.messenger()),
        Arc::clone(manager_b.topology()),
    );

    // Start sync tasks
    flow_a.start().expect("failed to start flow A");
    flow_b.start().expect("failed to start flow B");

    // --- Write operations on A ---

    flow_a.apply_operation(Operation::add_item("task_1", "GianttItem"));
    flow_a.apply_operation(Operation::set_field(
        "task_1",
        "title",
        Value::string("Cross-machine sync test"),
    ));
    flow_a.apply_operation(Operation::set_field(
        "task_1",
        "status",
        Value::string("NOT_STARTED"),
    ));

    // --- Wait for sync ---
    // Topology watcher polls every 2s, then horizon exchange, then op batch.
    // Give it enough time for the full round trip.
    tokio::time::sleep(Duration::from_secs(6)).await;

    // --- Verify B has the data ---

    let state_b = flow_b.read_drip();
    eprintln!("Flow B state:\n{}", state_b);

    assert!(
        state_b.contains("task_1"),
        "Flow B should have task_1 after sync. Got: {}",
        state_b,
    );
    assert!(
        state_b.contains("Cross-machine sync test"),
        "Flow B should have task_1's title. Got: {}",
        state_b,
    );

    // --- Verify bidirectional: write on B, check A ---

    flow_b.apply_operation(Operation::add_item("task_2", "GianttItem"));
    flow_b.apply_operation(Operation::set_field(
        "task_2",
        "title",
        Value::string("Bidirectional sync"),
    ));

    tokio::time::sleep(Duration::from_secs(4)).await;

    let state_a = flow_a.read_drip();
    eprintln!("Flow A state:\n{}", state_a);

    assert!(
        state_a.contains("task_2"),
        "Flow A should have task_2 after bidirectional sync. Got: {}",
        state_a,
    );

    // Cleanup
    manager_a.stop();
    manager_b.stop();
}
