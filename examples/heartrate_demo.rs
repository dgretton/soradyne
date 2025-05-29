//! examples/heartrate_demo.rs
//!
//! A demo app that streams heartrate data, subscribes to updates, persists
//! data to disk, and syncs heartrate across devices using NetworkBridge.

use soradyne::types::heartrate::{Heartrate, HeartrateFlow};
use soradyne::network::connection::NetworkBridge;
use soradyne::network::NoOpDiscovery;
use soradyne::flow::FlowType;
use soradyne::storage::LocalFileStorage;
use uuid::Uuid;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let device_id = Uuid::new_v4();
    
    // Create a local file storage backend
    let storage = LocalFileStorage::new("./data").expect("Failed to create storage directory");
    
    let flow = Arc::new(HeartrateFlow::new(
        "heartrate_demo",
        device_id,
        Heartrate::new(70.0, device_id),
        FlowType::RealTimeScalar,
    ).with_storage(storage));

    // Create a network bridge with discovery
    let bridge = Arc::new(NetworkBridge::new()
        .with_discovery(NoOpDiscovery));
    
    // Start peer discovery (this is a no-op in this example)
    if let Err(e) = bridge.start_discovery() {
        eprintln!("Failed to start discovery: {}", e);
    }

    // Start listening for incoming peer connections (adjust port as needed)
    let flow_clone = flow.clone();
    let bridge_clone = bridge.clone();
    tokio::spawn(async move {
        bridge_clone
            .listen("0.0.0.0:5001", flow_clone)
            .await
            .unwrap();
    });

    // Optionally connect to a peer (uncomment and set the correct address)
    /*
    let bridge_clone2 = bridge.clone();
    tokio::spawn(async move {
        bridge_clone2
            .connect("192.168.1.42:5001")  // replace with actual peer IP/port
            .await
            .unwrap();
    });
    */

    // Subscribe to updates
    flow.subscribe(Box::new(|value| {
        println!(
            "[Subscriber] New heartrate: {:.1} bpm from {} at {}",
            value.bpm, value.source_device_id, value.timestamp
        );
    }));

    // Simulate incoming heartrate updates
    for bpm in [72.5, 74.0, 75.3, 73.8] {
        let new_reading = Heartrate::new(bpm, device_id);
        println!(
            "[Local] Updating heartrate: {:.1} bpm at {}",
            new_reading.bpm, new_reading.timestamp
        );
        flow.update(new_reading.clone());

        // Persist after each update
        flow.persist().unwrap_or_else(|e| eprintln!("Failed to persist: {}", e));

        // Broadcast to peers
        bridge.broadcast(&new_reading);

        sleep(Duration::from_secs(2)).await;
    }

    // Simulate merging a remote value
    let remote_device = Uuid::new_v4();
    let remote_reading = Heartrate::new(76.4, remote_device);
    println!(
        "[Merge] Merging remote heartrate: {:.1} bpm",
        remote_reading.bpm
    );
    flow.merge(remote_reading);

    // Persist final state
    flow.persist().unwrap_or_else(|e| eprintln!("Failed to persist: {}", e));
}

