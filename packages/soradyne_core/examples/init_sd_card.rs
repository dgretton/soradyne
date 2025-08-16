use std::path::PathBuf;
use std::io::{self, Write};
use tokio;
use soradyne::storage::device_identity::{fingerprint_device, discover_soradyne_volumes};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Soradyne SD Card Initialization Tool");
    println!("========================================");
    println!("This tool will initialize SD cards for Soradyne storage.\n");
    
    loop {
        println!("Options:");
        println!("1. Initialize a specific SD card path");
        println!("2. Auto-discover and initialize SD cards");
        println!("3. List existing Soradyne volumes");
        println!("4. Exit");
        print!("Choose an option (1-4): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();
        
        match choice {
            "1" => {
                initialize_specific_path().await?;
            }
            
            "2" => {
                auto_discover_and_initialize().await?;
            }
            
            "3" => {
                list_existing_volumes().await?;
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
    
    Ok(())
}

async fn initialize_specific_path() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📱 Initialize Specific SD Card");
    println!("==============================");
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
        return Ok(());
    }
    
    let rimsd_path = sd_path.join(".rimsd");
    
    // Check if already initialized
    if rimsd_path.exists() {
        println!("⚠️  .rimsd directory already exists at: {}", rimsd_path.display());
        print!("Do you want to reinitialize? (y/N): ");
        io::stdout().flush()?;
        
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        if !confirm.trim().to_lowercase().starts_with('y') {
            println!("❌ Initialization cancelled.");
            return Ok(());
        }
    }
    
    println!("\n🔍 Initializing Soradyne storage at: {}", rimsd_path.display());
    
    // Create .rimsd directory
    tokio::fs::create_dir_all(&rimsd_path).await?;
    
    // Generate device fingerprint
    match fingerprint_device(&rimsd_path).await {
        Ok(fingerprint) => {
            println!("✅ Successfully initialized Soradyne storage!");
            print_fingerprint_info(&fingerprint, &rimsd_path).await?;
        }
        Err(e) => {
            println!("❌ Failed to generate device fingerprint: {}", e);
            println!("   The .rimsd directory was created but fingerprinting failed.");
        }
    }
    
    Ok(())
}

async fn auto_discover_and_initialize() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Auto-Discover SD Cards");
    println!("=========================");
    println!("Scanning for mounted volumes that could be SD cards...");
    
    // Get potential mount points
    let mount_points = get_potential_sd_cards().await?;
    
    if mount_points.is_empty() {
        println!("❌ No potential SD card mount points found.");
        println!("   Please ensure your SD cards are mounted and try option 1 for manual initialization.");
        return Ok(());
    }
    
    println!("Found {} potential SD card mount points:", mount_points.len());
    for (i, path) in mount_points.iter().enumerate() {
        let rimsd_path = path.join(".rimsd");
        let status = if rimsd_path.exists() { "✅ Already initialized" } else { "❌ Not initialized" };
        println!("   {}. {} - {}", i + 1, path.display(), status);
    }
    
    print!("\nEnter the number of the SD card to initialize (or 0 to cancel): ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice: usize = input.trim().parse().unwrap_or(0);
    
    if choice == 0 || choice > mount_points.len() {
        println!("❌ Cancelled or invalid choice.");
        return Ok(());
    }
    
    let selected_path = &mount_points[choice - 1];
    let rimsd_path = selected_path.join(".rimsd");
    
    println!("\n🔍 Initializing: {}", selected_path.display());
    
    // Create .rimsd directory
    tokio::fs::create_dir_all(&rimsd_path).await?;
    
    // Generate device fingerprint
    match fingerprint_device(&rimsd_path).await {
        Ok(fingerprint) => {
            println!("✅ Successfully initialized Soradyne storage!");
            print_fingerprint_info(&fingerprint, &rimsd_path).await?;
        }
        Err(e) => {
            println!("❌ Failed to generate device fingerprint: {}", e);
        }
    }
    
    Ok(())
}

async fn list_existing_volumes() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Existing Soradyne Volumes");
    println!("============================");
    
    match discover_soradyne_volumes().await {
        Ok(volumes) => {
            if volumes.is_empty() {
                println!("❌ No Soradyne volumes found.");
                println!("   Use options 1 or 2 to initialize SD cards.");
            } else {
                println!("Found {} Soradyne volumes:", volumes.len());
                for (i, rimsd_path) in volumes.iter().enumerate() {
                    println!("\n{}. {}", i + 1, rimsd_path.display());
                    
                    // Try to read the device fingerprint
                    match fingerprint_device(rimsd_path).await {
                        Ok(fingerprint) => {
                            println!("   Device ID: {:?}", fingerprint.soradyne_device_id);
                            println!("   Capacity: {:.2} GB", fingerprint.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
                            if let Some(hw_id) = &fingerprint.hardware_id {
                                println!("   Hardware: {}", hw_id);
                            }
                        }
                        Err(e) => {
                            println!("   ❌ Error reading fingerprint: {}", e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to discover volumes: {}", e);
        }
    }
    
    Ok(())
}

async fn print_fingerprint_info(fingerprint: &soradyne::storage::device_identity::BasicFingerprint, rimsd_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Device Fingerprint:");
    println!("   Soradyne Device ID: {:?}", fingerprint.soradyne_device_id);
    println!("   Hardware ID: {:?}", fingerprint.hardware_id);
    println!("   Filesystem UUID: {:?}", fingerprint.filesystem_uuid);
    println!("   Capacity: {:.2} GB", fingerprint.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
    println!("   Bad block signature: 0x{:016x}", fingerprint.bad_block_signature);
    
    println!("\n💾 Files created:");
    let device_id_file = rimsd_path.join("soradyne_device_id.txt");
    if device_id_file.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&device_id_file).await {
            println!("   {}/soradyne_device_id.txt", rimsd_path.display());
            println!("   Device ID: {}", content.trim());
        }
    }
    
    println!("\n🎉 SD card is now ready for Soradyne storage!");
    
    Ok(())
}

async fn get_potential_sd_cards() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut mount_points = Vec::new();
    
    #[cfg(target_os = "macos")]
    {
        // Check /Volumes for mounted drives
        if let Ok(entries) = std::fs::read_dir("/Volumes") {
            for entry in entries.flatten() {
                let path = entry.path();
                // Skip system volumes
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                if !name.contains("macintosh") && !name.contains("system") && !name.contains("data") {
                    mount_points.push(path);
                }
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Check common mount points
        for base_dir in &["/media", "/mnt", "/run/media"] {
            if let Ok(entries) = std::fs::read_dir(base_dir) {
                for entry in entries.flatten() {
                    if base_dir == "/run/media" {
                        // /run/media has user subdirectories
                        if let Ok(user_entries) = std::fs::read_dir(entry.path()) {
                            for user_entry in user_entries.flatten() {
                                mount_points.push(user_entry.path());
                            }
                        }
                    } else {
                        mount_points.push(entry.path());
                    }
                }
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Check removable drives
        for letter in 'D'..='Z' {
            let drive_path = PathBuf::from(format!("{}:\\", letter));
            if drive_path.exists() {
                // Try to determine if it's removable (simplified check)
                mount_points.push(drive_path);
            }
        }
    }
    
    Ok(mount_points)
}
