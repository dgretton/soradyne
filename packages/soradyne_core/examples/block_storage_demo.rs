use std::path::PathBuf;
use std::io::{self, Write};
use tokio;
use soradyne::storage::{BlockManager, discover_soradyne_volumes};
use soradyne::storage::device_identity::fingerprint_device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧱 Soradyne Block Storage Demo");
    println!("==============================");
    
    // Discover Soradyne devices
    println!("🔍 Discovering Soradyne storage devices...");
    let rimsd_dirs = discover_soradyne_volumes().await?;
    
    if rimsd_dirs.is_empty() {
        println!("❌ No Soradyne devices found!");
        println!("   Please initialize some SD cards with .rimsd directories first.");
        println!("   You can create test directories with:");
        println!("   mkdir -p /tmp/test_sd1/.rimsd");
        println!("   echo 'test-device-1' > /tmp/test_sd1/.rimsd/soradyne_device_id.txt");
        return Ok(());
    }
    
    println!("✅ Found {} Soradyne devices:", rimsd_dirs.len());
    for (i, dir) in rimsd_dirs.iter().enumerate() {
        println!("   {}. {}", i + 1, dir.display());
    }
    
    // Set up erasure coding parameters
    let threshold = std::cmp::min(2, rimsd_dirs.len());
    let total_shards = rimsd_dirs.len();
    
    println!("\n📊 Storage Configuration:");
    println!("   Devices: {}", total_shards);
    println!("   Threshold: {} (minimum devices needed for recovery)", threshold);
    println!("   Redundancy: {}", total_shards - threshold);
    
    // Create BlockManager
    let metadata_path = PathBuf::from("/tmp/soradyne_demo_metadata.json");
    let block_manager = BlockManager::new(rimsd_dirs, metadata_path, threshold, total_shards)?;
    
    // Interactive demo loop
    loop {
        println!("\n🎮 Demo Commands:");
        println!("   init      - Initialize .rimsd directory on SD card");
        println!("   w <text>  - Write text as a block");
        println!("   r <id>    - Read block by ID (first 8 chars)");
        println!("   l         - List all blocks");
        println!("   d <id>    - Show block distribution");
        println!("   t <id>    - Test erasure recovery");
        println!("   s         - Show storage info");
        println!("   q         - Quit");
        print!("\n> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        
        if parts.is_empty() {
            continue;
        }
        
        match parts[0] {
            "init" => {
                println!("\n🔧 Initialize Soradyne SD Card");
                println!("===============================");
                println!("This will create a .rimsd directory and generate device fingerprints.");
                println!("Enter the mount path of your SD card:");
                println!("Examples:");
                println!("  macOS:   /Volumes/SDCARD");
                println!("  Linux:   /media/username/SDCARD or /mnt/sdcard");
                println!("  Windows: D:\\ or E:\\");
                print!("\nSD card path: ");
                io::stdout().flush()?;
                
                let mut path_input = String::new();
                io::stdin().read_line(&mut path_input)?;
                let sd_path = std::path::Path::new(path_input.trim());
                
                if !sd_path.exists() {
                    println!("❌ Path does not exist: {}", sd_path.display());
                    continue;
                }
                
                let rimsd_path = sd_path.join(".rimsd");
                
                println!("\n🔍 Initializing Soradyne storage at: {}", rimsd_path.display());
                
                // Create .rimsd directory if it doesn't exist
                if let Err(e) = tokio::fs::create_dir_all(&rimsd_path).await {
                    println!("❌ Failed to create .rimsd directory: {}", e);
                    continue;
                }
                
                // Generate device fingerprint (this will create soradyne_device_id.txt automatically)
                match fingerprint_device(&rimsd_path).await {
                    Ok(fingerprint) => {
                        println!("✅ Successfully initialized Soradyne storage!");
                        println!("\n📊 Device Fingerprint:");
                        println!("   Soradyne Device ID: {:?}", fingerprint.soradyne_device_id);
                        println!("   Hardware ID: {:?}", fingerprint.hardware_id);
                        println!("   Filesystem UUID: {:?}", fingerprint.filesystem_uuid);
                        println!("   Capacity: {:.2} GB", fingerprint.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
                        println!("   Bad block signature: 0x{:016x}", fingerprint.bad_block_signature);
                        
                        println!("\n💾 Files created:");
                        println!("   {}/soradyne_device_id.txt", rimsd_path.display());
                        
                        // Verify the device ID file was created
                        let device_id_file = rimsd_path.join("soradyne_device_id.txt");
                        if device_id_file.exists() {
                            if let Ok(content) = tokio::fs::read_to_string(&device_id_file).await {
                                println!("   Device ID: {}", content.trim());
                            }
                        }
                        
                        println!("\n🎉 SD card is now ready for Soradyne storage!");
                        println!("   You can now restart this demo to use the initialized card.");
                    }
                    Err(e) => {
                        println!("❌ Failed to generate device fingerprint: {}", e);
                        println!("   The .rimsd directory was created but fingerprinting failed.");
                        println!("   You may need to run this on the actual SD card device.");
                    }
                }
            }
            
            "w" => {
                if parts.len() < 2 {
                    println!("❌ Usage: w <text>");
                    continue;
                }
                
                let text = parts[1..].join(" ");
                let data = text.as_bytes();
                
                println!("📝 Writing block: \"{}\" ({} bytes)", text, data.len());
                
                match block_manager.write_direct_block(data).await {
                    Ok(block_id) => {
                        let id_str = hex::encode(&block_id[..4]);
                        println!("\n✅ Block written successfully!");
                        println!("   Block ID: {} (use '{}' for commands)", hex::encode(block_id), id_str);
                    }
                    Err(e) => println!("❌ Failed to write block: {}", e),
                }
            }
            
            "r" => {
                if parts.len() < 2 {
                    println!("❌ Usage: r <block_id>");
                    continue;
                }
                
                let id_prefix = parts[1];
                if let Some(block_id) = find_block_by_prefix(&block_manager, id_prefix).await {
                    println!("\n📖 Reading block {}...", hex::encode(&block_id[..4]));
                    
                    match block_manager.read_block(&block_id).await {
                        Ok(data) => {
                            let text = String::from_utf8_lossy(&data);
                            println!("\n✅ Block content: \"{}\"", text);
                            println!("   Size: {} bytes", data.len());
                        }
                        Err(e) => println!("❌ Failed to read block: {}", e),
                    }
                } else {
                    println!("❌ Block not found with prefix: {}", id_prefix);
                }
            }
            
            "l" => {
                println!("📋 Listing all blocks...");
                let blocks = block_manager.list_blocks().await;
                
                if blocks.is_empty() {
                    println!("   📭 No blocks found");
                } else {
                    println!("   Found {} blocks:", blocks.len());
                    for (block_id, metadata) in blocks.iter() {
                        let id_str = hex::encode(&block_id[..4]);
                        println!("     {} - {} bytes - {}", 
                            id_str,
                            metadata.size,
                            metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                    println!("\n   💡 Use block ID (first 8 chars) with 'r', 'd', or 't' commands");
                }
            }
            
            "d" => {
                if parts.len() < 2 {
                    println!("❌ Usage: d <block_id>");
                    continue;
                }
                
                let id_prefix = parts[1];
                if let Some(block_id) = find_block_by_prefix(&block_manager, id_prefix).await {
                    match block_manager.get_block_distribution(&block_id).await {
                        Ok(distribution) => {
                            println!("📊 Block Distribution for {}:", hex::encode(&block_id[..4]));
                            println!("   Original size: {} bytes", distribution.original_size);
                            println!("   Total shards: {}", distribution.total_shards);
                            println!("   Available shards: {}", distribution.available_shards.len());
                            println!("   Missing shards: {}", distribution.missing_shards.len());
                            println!("   Can reconstruct: {}", if distribution.can_reconstruct { "✅ Yes" } else { "❌ No" });
                            
                            println!("\n   Shard locations:");
                            for shard in &distribution.available_shards {
                                println!("     Shard {} → {} ({} bytes)", 
                                    shard.index, shard.device_path, shard.size);
                            }
                            
                            if !distribution.missing_shards.is_empty() {
                                println!("\n   Missing shards: {:?}", distribution.missing_shards);
                            }
                        }
                        Err(e) => println!("❌ Failed to get distribution: {}", e),
                    }
                } else {
                    println!("❌ Block not found with prefix: {}", id_prefix);
                }
            }
            
            "t" => {
                if parts.len() < 2 {
                    println!("❌ Usage: t <block_id>");
                    continue;
                }
                
                let id_prefix = parts[1];
                if let Some(block_id) = find_block_by_prefix(&block_manager, id_prefix).await {
                    // Simulate missing some shards
                    let shards_to_remove = vec![0, 1]; // Remove first two shards
                    
                    println!("🧪 Testing erasure recovery for {}...", hex::encode(&block_id[..4]));
                    println!("   Simulating missing shards: {:?}", shards_to_remove);
                    
                    match block_manager.demonstrate_erasure_recovery(&block_id, shards_to_remove).await {
                        Ok(result) => {
                            println!("📊 Recovery Test Results:");
                            println!("   Original shards: {}", result.original_shards);
                            println!("   Simulated missing: {:?}", result.simulated_missing);
                            println!("   Available shards: {}", result.available_shards);
                            println!("   Threshold required: {}", result.threshold_required);
                            println!("   Recovery successful: {}", if result.recovery_successful { "✅ Yes" } else { "❌ No" });
                            
                            if result.recovery_successful {
                                println!("   Recovered data size: {} bytes", result.recovered_data_size);
                                println!("   🎉 Data can be recovered even with {} missing shards!", result.simulated_missing.len());
                            } else {
                                println!("   ⚠️  Not enough shards available for recovery");
                                println!("   Need at least {} shards, but only {} available", 
                                    result.threshold_required, result.available_shards);
                            }
                        }
                        Err(e) => println!("❌ Failed to test recovery: {}", e),
                    }
                } else {
                    println!("❌ Block not found with prefix: {}", id_prefix);
                }
            }
            
            "s" => {
                let info = block_manager.get_storage_info();
                println!("📊 Storage Information:");
                println!("   Total devices: {}", info.total_devices);
                println!("   Erasure threshold: {}", info.threshold);
                println!("   Total shards per block: {}", info.total_shards);
                println!("   Redundancy level: {}", info.total_shards - info.threshold);
                
                println!("\n   Device paths:");
                for (i, path) in info.rimsd_paths.iter().enumerate() {
                    println!("     {}. {}", i + 1, path.display());
                }
            }
            
            "q" => {
                println!("👋 Goodbye!");
                break;
            }
            
            _ => {
                println!("❌ Unknown command: {}", parts[0]);
            }
        }
    }
    
    Ok(())
}

async fn find_block_by_prefix(block_manager: &BlockManager, prefix: &str) -> Option<[u8; 32]> {
    // Get all blocks and find one that starts with the given prefix
    let blocks = block_manager.list_blocks().await;
    
    for (block_id, _metadata) in blocks {
        let block_hex = hex::encode(block_id);
        if block_hex.starts_with(prefix) {
            return Some(block_id);
        }
    }
    
    None
}
