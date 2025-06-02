use std::path::PathBuf;
use std::sync::Arc;
use tokio;

use soradyne::storage::block_manager::BlockManager;
use soradyne::types::media::{PhotoStorage, VideoStorage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up Soradyne Block Storage Example");
    
    // 1. Create test .rimsd directories
    let rimsd_dirs = setup_test_rimsd_directories()?;
    println!("Created {} test .rimsd directories", rimsd_dirs.len());
    
    // 2. Set up metadata directory
    let metadata_dir = std::env::temp_dir().join("soradyne_metadata");
    std::fs::create_dir_all(&metadata_dir)?;
    let metadata_path = metadata_dir.join("block_metadata.json");
    
    // 3. Create BlockManager with erasure coding configuration
    let threshold = 2; // Need at least 2 shards to reconstruct
    let total_shards = 3; // Create 3 shards total
    
    let block_manager = Arc::new(BlockManager::new(
        rimsd_dirs.clone(),
        metadata_path,
        threshold,
        total_shards,
    )?);
    
    println!("BlockManager created with threshold={}, total_shards={}", threshold, total_shards);
    
    // 4. Test basic block operations
    test_basic_block_operations(&block_manager).await?;
    
    // 5. Test photo storage
    test_photo_storage(&block_manager).await?;
    
    // 6. Test video storage  
    test_video_storage(&block_manager).await?;
    
    println!("All tests completed successfully!");
    println!("Test directories created at:");
    for dir in &rimsd_dirs {
        println!("  {}", dir.display());
    }
    
    Ok(())
}

/// Set up test .rimsd directories in the system temp directory
fn setup_test_rimsd_directories() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let base_temp = std::env::temp_dir();
    let mut rimsd_dirs = Vec::new();
    
    // Create 3 test .rimsd directories
    for i in 0..3 {
        let rimsd_dir = base_temp.join(format!("soradyne_test_{}.rimsd", i));
        
        // Remove existing directory if it exists
        if rimsd_dir.exists() {
            std::fs::remove_dir_all(&rimsd_dir)?;
        }
        
        // Create the directory
        std::fs::create_dir_all(&rimsd_dir)?;
        rimsd_dirs.push(rimsd_dir);
    }
    
    Ok(rimsd_dirs)
}

/// Test basic block read/write operations
async fn test_basic_block_operations(block_manager: &Arc<BlockManager>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Testing Basic Block Operations ---");
    
    // Test writing a small block
    let test_data = b"Hello, Soradyne Block Storage!";
    println!("Writing test data: {:?}", std::str::from_utf8(test_data)?);
    
    let block_id = block_manager.write_direct_block(test_data).await?;
    println!("Block written with ID: {}", hex::encode(block_id));
    
    // Test reading the block back
    let read_data = block_manager.read_block(&block_id).await?;
    println!("Read back data: {:?}", std::str::from_utf8(&read_data)?);
    
    // Verify data integrity
    assert_eq!(test_data, read_data.as_slice());
    println!("✓ Data integrity verified");
    
    // Test writing a larger block
    let large_data = vec![42u8; 1024 * 1024]; // 1MB of data
    println!("Writing 1MB test block...");
    
    let large_block_id = block_manager.write_direct_block(&large_data).await?;
    println!("Large block written with ID: {}", hex::encode(large_block_id));
    
    let read_large_data = block_manager.read_block(&large_block_id).await?;
    assert_eq!(large_data, read_large_data);
    println!("✓ Large block integrity verified");
    
    Ok(())
}

/// Test photo storage functionality
async fn test_photo_storage(block_manager: &Arc<BlockManager>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Testing Photo Storage ---");
    
    let photo_storage = PhotoStorage::new(block_manager.clone());
    
    // Create some mock photo data (in reality this would be JPEG/PNG data)
    let mock_photo_data = create_mock_image_data("JPEG", 1920, 1080);
    println!("Created mock photo data: {} bytes", mock_photo_data.len());
    
    // Store the photo
    let photo_metadata = photo_storage.save_photo("test_photo.jpg", "image/jpeg", &mock_photo_data).await?;
    println!("Photo stored with ID: {}", photo_metadata.id);
    
    // Retrieve the photo
    let retrieved_photo = photo_storage.load_photo(&photo_metadata).await?;
    assert_eq!(mock_photo_data, retrieved_photo);
    println!("✓ Photo storage and retrieval verified");
    
    Ok(())
}

/// Test video storage functionality
async fn test_video_storage(block_manager: &Arc<BlockManager>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Testing Video Storage ---");
    
    let video_storage = VideoStorage::new(block_manager.clone());
    
    // Create some mock video data (in reality this would be MP4/AVI data)
    let mock_video_data = create_mock_video_data(1920, 1080, 30); // 30 seconds at 1080p
    println!("Created mock video data: {} bytes", mock_video_data.len());
    
    // Store the video
    let video_metadata = video_storage.save_video("test_video.mp4", "video/mp4", &mock_video_data).await?;
    println!("Video stored with ID: {}", video_metadata.id);
    
    // Retrieve the video
    let retrieved_video = video_storage.load_video(&video_metadata).await?;
    assert_eq!(mock_video_data, retrieved_video);
    println!("✓ Video storage and retrieval verified");
    
    Ok(())
}

/// Create mock image data for testing
fn create_mock_image_data(format: &str, width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::new();
    
    // Mock JPEG header
    data.extend_from_slice(b"\xFF\xD8\xFF\xE0");
    
    // Add some metadata
    data.extend_from_slice(format!("{}x{} {}", width, height, format).as_bytes());
    
    // Add some mock image data
    for i in 0..(width * height / 100) {
        data.push((i % 256) as u8);
    }
    
    // Mock JPEG footer
    data.extend_from_slice(b"\xFF\xD9");
    
    data
}

/// Create mock video data for testing
fn create_mock_video_data(width: u32, height: u32, duration_seconds: u32) -> Vec<u8> {
    let mut data = Vec::new();
    
    // Mock MP4 header
    data.extend_from_slice(b"ftypisom");
    
    // Add some metadata
    data.extend_from_slice(format!("{}x{}@{}s", width, height, duration_seconds).as_bytes());
    
    // Add some mock video frames (much larger than photo)
    let frame_size = (width * height * 3) / 1000; // Simplified frame size
    for frame in 0..(duration_seconds * 30) { // 30 FPS
        for i in 0..frame_size {
            data.push(((frame + i) % 256) as u8);
        }
    }
    
    data
}
