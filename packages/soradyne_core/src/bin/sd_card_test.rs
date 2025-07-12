//! Interactive SD Card Device Identity Test
//! 
//! Run with: cargo run --bin sd_card_test

use std::io::{self, Write};
use std::collections::HashMap;
use soradyne::storage::device_identity::{fingerprint_device, BasicFingerprint, BayesianDeviceIdentifier};

#[tokio::main]
async fn main() {
    println!("\n🔍 Interactive SD Card Device Identity Test");
    println!("==========================================");
    println!("This test will help you verify that SD card fingerprinting works correctly.");
    println!("You'll need to insert SD cards when prompted.\n");
    
    let mut stored_fingerprints: HashMap<String, BasicFingerprint> = HashMap::new();
    let identifier = BayesianDeviceIdentifier::default();
    
    loop {
        println!("Options:");
        println!("1. Initialize new SD card");
        println!("2. Verify existing SD card");
        println!("3. List stored fingerprints");
        println!("4. Exit");
        print!("Choose an option (1-4): ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let choice = input.trim();
        
        match choice {
            "1" => {
                println!("\n📱 Insert an SD card and enter its mount path:");
                println!("Examples:");
                println!("  macOS: /Volumes/SDCARD");
                println!("  Linux: /media/username/SDCARD");
                println!("  Windows: D:\\");
                print!("Path: ");
                io::stdout().flush().unwrap();
                
                let mut path_input = String::new();
                io::stdin().read_line(&mut path_input).unwrap();
                let rimsd_path = std::path::Path::new(path_input.trim()).join(".rimsd");
                
                println!("🔍 Fingerprinting SD card...");
                match fingerprint_device(&rimsd_path).await {
                    Ok(fingerprint) => {
                        println!("✅ Successfully fingerprinted SD card!");
                        println!("   Soradyne ID: {:?}", fingerprint.soradyne_device_id);
                        println!("   Hardware ID: {:?}", fingerprint.hardware_id);
                        println!("   Filesystem UUID: {:?}", fingerprint.filesystem_uuid);
                        println!("   Capacity: {} GB", fingerprint.capacity_bytes / (1024 * 1024 * 1024));
                        println!("   Bad blocks: {} detected", if fingerprint.bad_block_signature == 0 { 0 } else { 1 });
                        
                        if let Some(soradyne_id) = &fingerprint.soradyne_device_id {
                            let device_id = soradyne_id.clone();
                            stored_fingerprints.insert(device_id.clone(), fingerprint);
                            println!("💾 Stored fingerprint for device: {}", device_id);
                        } else {
                            println!("⚠️  Warning: No Soradyne device ID found");
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to fingerprint SD card: {}", e);
                    }
                }
            }
            
            "2" => {
                if stored_fingerprints.is_empty() {
                    println!("❌ No stored fingerprints. Initialize an SD card first.");
                    continue;
                }
                
                println!("\n📱 Insert an SD card to verify and enter its mount path:");
                print!("Path: ");
                io::stdout().flush().unwrap();
                
                let mut path_input = String::new();
                io::stdin().read_line(&mut path_input).unwrap();
                let rimsd_path = std::path::Path::new(path_input.trim()).join(".rimsd");
                
                println!("🔍 Fingerprinting SD card...");
                match fingerprint_device(&rimsd_path).await {
                    Ok(current_fingerprint) => {
                        if let Some(soradyne_id) = &current_fingerprint.soradyne_device_id {
                            if let Some(stored_fingerprint) = stored_fingerprints.get(soradyne_id) {
                                println!("🔍 Comparing with stored fingerprint...");
                                
                                match identifier.identify_device(&current_fingerprint, stored_fingerprint) {
                                    Ok(result) => {
                                        if result.is_same_device {
                                            println!("✅ MATCH: This is the same SD card!");
                                            println!("   Confidence: {:.2}%", result.confidence * 100.0);
                                            println!("   Evidence: {:?}", result.evidence_summary);
                                        } else {
                                            println!("❌ NO MATCH: This appears to be a different SD card!");
                                            println!("   Confidence: {:.2}%", result.confidence * 100.0);
                                            println!("   Evidence: {:?}", result.evidence_summary);
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ Failed to compare fingerprints: {}", e);
                                    }
                                }
                            } else {
                                println!("❌ No stored fingerprint found for Soradyne ID: {}", soradyne_id);
                                println!("   This appears to be a new SD card.");
                            }
                        } else {
                            println!("❌ No Soradyne device ID found on this SD card");
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to fingerprint SD card: {}", e);
                    }
                }
            }
            
            "3" => {
                println!("\n📋 Stored Fingerprints:");
                if stored_fingerprints.is_empty() {
                    println!("   (none)");
                } else {
                    for (id, fingerprint) in &stored_fingerprints {
                        println!("   🔑 {}", id);
                        println!("      Hardware: {:?}", fingerprint.hardware_id);
                        println!("      Filesystem: {:?}", fingerprint.filesystem_uuid);
                        println!("      Capacity: {} GB", fingerprint.capacity_bytes / (1024 * 1024 * 1024));
                    }
                }
            }
            
            "4" => {
                println!("👋 Goodbye!");
                break;
            }
            
            _ => {
                println!("❌ Invalid option. Please choose 1-4.");
            }
        }
        
        println!();
    }
}
