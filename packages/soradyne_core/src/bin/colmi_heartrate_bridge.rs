// src/bin/colmi_heartrate_bridge.rs
//! Enhanced Soradyne heartrate bridge that receives heart rate data from Colmi ring
//! and distributes it via the Soradyne protocol

use soradyne::types::heartrate::{Heartrate, HeartrateFlow};
use soradyne::network::connection::NetworkBridge;
use soradyne::network::NoOpDiscovery;
use soradyne::flow::FlowType;
use soradyne::storage::LocalFileStorage;
use uuid::Uuid;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use serde_json;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

const SOCKET_PATH: &str = "/tmp/soradyne_heartrate.sock";

#[derive(Debug, Deserialize, Serialize)]
struct ColmiHeartRateMessage {
    bpm: f32,
    timestamp: String,
    source: String,
    raw_data: Option<serde_json::Value>,
}

struct ColmiSoradyneBridge {
    flow: Arc<HeartrateFlow>,
    device_id: Uuid,
    last_processed_time: Option<DateTime<Utc>>,
}

impl ColmiSoradyneBridge {
    fn new(flow: Arc<HeartrateFlow>, device_id: Uuid) -> Self {
        Self {
            flow,
            device_id,
            last_processed_time: None,
        }
    }
    
    async fn start_unix_socket_listener(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove socket file if it exists
        let _ = std::fs::remove_file(SOCKET_PATH);
        
        // Create Unix domain socket listener
        let listener = UnixListener::bind(SOCKET_PATH)?;
        println!("Listening for Colmi ring data on {}", SOCKET_PATH);
        
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    println!("Colmi bridge connected");
                    
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Err(e) = self.process_heartrate_message(&line).await {
                            eprintln!("Error processing heart rate message: {}", e);
                        }
                    }
                    
                    println!("Colmi bridge disconnected");
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
    
    async fn process_heartrate_message(&mut self, json_line: &str) -> Result<(), Box<dyn std::error::Error>> {
        let message: ColmiHeartRateMessage = serde_json::from_str(json_line.trim())?;
        
        // Parse timestamp
        let timestamp = DateTime::parse_from_rfc3339(&message.timestamp)?
            .with_timezone(&Utc);
        
        // Check if this is a new reading (avoid duplicates)
        if let Some(last_time) = self.last_processed_time {
            if timestamp <= last_time {
                return Ok(()); // Skip duplicate or old reading
            }
        }
        
        // Validate heart rate
        if message.bpm < 30.0 || message.bpm > 220.0 {
            println!("Received invalid heart rate: {} BPM, skipping", message.bpm);
            return Ok(());
        }
        
        // Create Soradyne heartrate object
        let mut heartrate = Heartrate::new(message.bpm, self.device_id);
        heartrate.timestamp = timestamp;
        
        // Update the flow
        self.flow.update(heartrate.clone());
        
        // Update last processed time
        self.last_processed_time = Some(timestamp);
        
        println!(
            "[Colmi] Processed heart rate: {:.1} BPM at {} (source: {})",
            message.bpm, 
            timestamp.format("%H:%M:%S%.3f"),
            message.source
        );
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Enhanced Soradyne Heartrate Bridge with Colmi Support");
    
    let device_id = Uuid::new_v4();
    
    // Create a local file storage backend
    let storage = LocalFileStorage::new("./heartrate_data")
        .expect("Failed to create storage directory");
    
    let flow = Arc::new(HeartrateFlow::new(
        "colmi_heartrate_bridge",
        device_id,
        Heartrate::new(70.0, device_id),
        FlowType::RealTimeScalar,
    ).with_storage(storage));

    // Create a network bridge with discovery
    let bridge = Arc::new(NetworkBridge::new()
        .with_discovery(NoOpDiscovery));
    
    // Start peer discovery
    if let Err(e) = bridge.start_discovery() {
        eprintln!("Failed to start discovery: {}", e);
    }

    // Start listening for incoming Soradyne peer connections
    let flow_clone = flow.clone();
    let bridge_clone = bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = bridge_clone
            .listen("0.0.0.0:5001", flow_clone)
            .await 
        {
            eprintln!("Failed to start Soradyne listening: {}", e);
        }
    });

    // Subscribe to heart rate updates and broadcast to Soradyne peers
    let bridge_clone = bridge.clone();
    flow.subscribe(Box::new(move |heartrate| {
        let heartrate_clone = heartrate.clone();
        let bridge_clone = bridge_clone.clone();
        
        // Forward to other Soradyne peers
        bridge_clone.broadcast(&heartrate_clone);
        
        println!(
            "[Soradyne] Broadcasting heart rate: {:.1} BPM from {} at {}",
            heartrate_clone.bpm, 
            heartrate_clone.source_device_id, 
            heartrate_clone.timestamp.format("%H:%M:%S%.3f")
        );
    }));

    // Auto-persist heart rate data every 30 seconds
    let flow_for_persistence = flow.clone();
    tokio::spawn(async move {
        let mut last_persist_time = std::time::Instant::now();
        
        loop {
            sleep(Duration::from_secs(5)).await; // Check every 5 seconds
            
            if last_persist_time.elapsed() >= Duration::from_secs(30) {
                if let Err(e) = flow_for_persistence.persist() {
                    eprintln!("Failed to persist heart rate data: {}", e);
                } else {
                    println!("[Persistence] Heart rate data saved to storage");
                }
                last_persist_time = std::time::Instant::now();
            }
        }
    });

    // Create and start the Colmi bridge
    let mut colmi_bridge = ColmiSoradyneBridge::new(flow.clone(), device_id);
    
    // Start Unix socket listener for Colmi data
    let socket_listener_task = async move {
        if let Err(e) = colmi_bridge.start_unix_socket_listener().await {
            eprintln!("Unix socket listener error: {}", e);
        }
    };

    // Optionally connect to a remote Soradyne peer (uncomment to use)
    let bridge_for_connection = bridge.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(5)).await; // Give time for other side to start listening
        if let Err(e) = bridge_for_connection.connect("10.111.172.135:5001").await {
            eprintln!("Failed to connect to remote Soradyne peer: {}", e);
        } else {
            println!("Connected to remote Soradyne peer");
        }
    });

    // Demo data generation (fallback if no real data)
    let demo_flow = flow.clone();
    tokio::spawn(async move {
        let mut demo_bpm = 72.0;
        let mut demo_count = 0;
        
        // Wait before starting demo data
        sleep(Duration::from_secs(30)).await;
        
        loop {
            // Only generate demo data if we haven't received real data recently
            if let Some(current) = demo_flow.get_value() {
                let age = chrono::Utc::now().signed_duration_since(current.timestamp);
                
                // If no new data for 30 seconds, generate demo data
                if age.num_seconds() > 30 {
                    // Generate realistic heart rate variation
                    demo_bpm += (demo_count as f32 * 0.05).sin() * 3.0 + 
                               (demo_count as f32 * 0.1).cos() * 1.5;
                    demo_bpm = demo_bpm.max(65.0).min(85.0); // Keep in resting range
                    
                    let demo_reading = Heartrate::new(demo_bpm, device_id);
                    println!("[Demo] No recent data, generating demo heart rate: {:.1} BPM", demo_bpm);
                    demo_flow.update(demo_reading);
                    
                    demo_count += 1;
                }
            }
            
            sleep(Duration::from_secs(3)).await; // Generate demo data every 3 seconds
        }
    });

    println!("Enhanced Soradyne Bridge running:");
    println!("- Listening for Colmi ring data on Unix socket: {}", SOCKET_PATH);
    println!("- Listening for Soradyne peers on port 5001");
    println!("- Auto-persisting heart rate data every 30 seconds");
    println!("\nTo connect your Colmi ring, run:");
    println!("python3 colmi_soradyne_bridge.py --discover");
    println!("\nPress Ctrl+C to stop...");

    // Run the socket listener (this will run forever)
    socket_listener_task.await;

    Ok(())
}
