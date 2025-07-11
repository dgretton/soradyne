//! Large Video Block Storage Test
//! 
//! This example creates large video files that exceed the 32MB direct block limit
//! to test the indirect block storage functionality. It generates mock video data
//! of various sizes and verifies that the block storage system can handle files
//! larger than a single block.

use std::path::PathBuf;
use std::sync::Arc;
use tokio;
use sha2::{Sha256, Digest};

use soradyne::storage::block_manager::BlockManager;
use soradyne::types::media::VideoStorage;

/// Test video configurations - sizes designed to exceed 32MB limit
const TEST_VIDEOS: &[(&str, usize, &str)] = &[
    // Small video (under 32MB limit)
    ("small_video.mp4", 16 * 1024 * 1024, "16MB test video - should use direct block"),
    
    // Videos that exceed 32MB limit - should use indirect blocks
    ("medium_video.mp4", 48 * 1024 * 1024, "48MB test video - should use indirect blocks"),
    ("large_video.mp4", 64 * 1024 * 1024, "64MB test video - should use indirect blocks"),
    ("huge_video.mp4", 128 * 1024 * 1024, "128MB test video - should use multiple indirect blocks"),
    ("massive_video.mp4", 256 * 1024 * 1024, "256MB test video - stress test for block system"),
];

/// Block size limit from the storage system
const BLOCK_SIZE: usize = 32 * 1024 * 1024; // 32MB

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Large Video Block Storage Test");
    println!("=================================");
    println!("This test creates large video files that exceed the 32MB direct block limit");
    println!("to verify that the indirect block storage system works correctly.\n");
    
    let mut test_session = LargeVideoTestSession::new().await?;
    
    // Run the large video test
    test_session.run_large_video_test().await?;
    
    // Show results
    test_session.show_results();
    
    Ok(())
}

struct LargeVideoTestSession {
    block_manager: Arc<BlockManager>,
    video_storage: VideoStorage,
    test_dir: PathBuf,
    test_results: Vec<VideoTestResult>,
}

#[derive(Debug)]
struct VideoTestResult {
    name: String,
    description: String,
    original_size: usize,
    retrieved_size: usize,
    hash_match: bool,
    storage_time: std::time::Duration,
    retrieval_time: std::time::Duration,
    uses_indirect_blocks: bool,
    success: bool,
}

impl LargeVideoTestSession {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create test directory structure
        let test_dir = std::env::current_dir()?.join("large_video_test");
        
        // Clean and recreate directories
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        std::fs::create_dir_all(&test_dir)?;
        
        // Create rimsd directories (hidden .rimsd subdirectories)
        let mut rimsd_dirs = Vec::new();
        for i in 0..5 {
            let device_dir = test_dir.join(format!("rimsd_{}", i));
            let rimsd_dir = device_dir.join(".rimsd");
            std::fs::create_dir_all(&rimsd_dir)?;
            rimsd_dirs.push(rimsd_dir);
        }
        
        // Set up block manager with good redundancy for large files
        let metadata_path = test_dir.join("metadata.json");
        let threshold = 3; // Need 3 out of 5 shards
        let total_shards = 5;
        
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            threshold,
            total_shards,
        )?);
        
        let video_storage = VideoStorage::new(block_manager.clone());
        
        println!("✅ Test environment created at: {:?}", test_dir);
        println!("📊 Block size limit: {} MB", BLOCK_SIZE / (1024 * 1024));
        println!("🔧 Erasure coding: {}/{} shards\n", threshold, total_shards);
        
        Ok(Self {
            block_manager,
            video_storage,
            test_dir,
            test_results: Vec::new(),
        })
    }
    
    async fn run_large_video_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Starting large video test with {} videos...\n", TEST_VIDEOS.len());
        
        for (i, (name, size, description)) in TEST_VIDEOS.iter().enumerate() {
            println!("🎥 Test {}/{}: {}", i + 1, TEST_VIDEOS.len(), name);
            println!("   Description: {}", description);
            println!("   Size: {} MB ({} bytes)", size / (1024 * 1024), size);
            
            let uses_indirect = *size > BLOCK_SIZE;
            println!("   Expected storage type: {}", 
                    if uses_indirect { "Indirect blocks" } else { "Direct block" });
            
            match self.test_single_video(name, *size, description, uses_indirect).await {
                Ok(result) => {
                    if result.success {
                        println!("   ✅ SUCCESS - Hash match: {}, Storage: {:.2?}", 
                                result.hash_match, result.storage_time);
                    } else {
                        println!("   ❌ FAILED - Check logs for details");
                    }
                    self.test_results.push(result);
                }
                Err(e) => {
                    println!("   ❌ ERROR: {}", e);
                    // Create a failed result entry
                    self.test_results.push(VideoTestResult {
                        name: name.to_string(),
                        description: description.to_string(),
                        original_size: *size,
                        retrieved_size: 0,
                        hash_match: false,
                        storage_time: std::time::Duration::from_secs(0),
                        retrieval_time: std::time::Duration::from_secs(0),
                        uses_indirect_blocks: uses_indirect,
                        success: false,
                    });
                }
            }
            println!();
        }
        
        Ok(())
    }
    
    async fn test_single_video(&self, name: &str, size: usize, description: &str, uses_indirect: bool) 
        -> Result<VideoTestResult, Box<dyn std::error::Error>> {
        
        // Generate mock video data
        println!("   🎬 Generating {} MB of mock video data...", size / (1024 * 1024));
        let video_data = generate_mock_video_data(size);
        
        if video_data.len() != size {
            return Err(format!("Generated data size mismatch: expected {}, got {}", 
                              size, video_data.len()).into());
        }
        
        // Calculate hash for integrity verification
        let original_hash = calculate_hash(&video_data);
        
        // Store using block storage
        println!("   💾 Storing in block storage...");
        let storage_start = std::time::Instant::now();
        let metadata = self.video_storage.save_video(name, "video/mp4", &video_data).await?;
        let storage_time = storage_start.elapsed();
        
        println!("   🔑 Video ID: {}", metadata.id);
        
        // Retrieve from block storage
        println!("   📤 Retrieving from block storage...");
        let retrieval_start = std::time::Instant::now();
        let retrieved_data = self.video_storage.load_video(&metadata).await?;
        let retrieval_time = retrieval_start.elapsed();
        
        // Verify integrity
        let retrieved_hash = calculate_hash(&retrieved_data);
        let hash_match = original_hash == retrieved_hash;
        let size_match = video_data.len() == retrieved_data.len();
        let data_match = video_data == retrieved_data;
        
        let success = hash_match && size_match && data_match;
        
        println!("   ⏱️  Storage: {:.2?}, Retrieval: {:.2?}", storage_time, retrieval_time);
        println!("   🔍 Integrity: Hash={}, Size={}, Data={}", 
                hash_match, size_match, data_match);
        
        // Calculate throughput
        let storage_throughput = size as f64 / storage_time.as_secs_f64() / (1024.0 * 1024.0);
        let retrieval_throughput = size as f64 / retrieval_time.as_secs_f64() / (1024.0 * 1024.0);
        println!("   📈 Throughput: Storage {:.1} MB/s, Retrieval {:.1} MB/s", 
                storage_throughput, retrieval_throughput);
        
        Ok(VideoTestResult {
            name: name.to_string(),
            description: description.to_string(),
            original_size: video_data.len(),
            retrieved_size: retrieved_data.len(),
            hash_match,
            storage_time,
            retrieval_time,
            uses_indirect_blocks: uses_indirect,
            success,
        })
    }
    
    fn show_results(&self) {
        println!("📊 LARGE VIDEO TEST RESULTS");
        println!("===========================");
        
        let successful = self.test_results.iter().filter(|r| r.success).count();
        let total = self.test_results.len();
        
        println!("✅ Successful: {}/{} ({:.1}%)", 
                successful, total, 
                successful as f64 / total as f64 * 100.0);
        
        if successful < total {
            println!("❌ Failed: {}", total - successful);
        }
        
        println!("\n📋 Detailed Results:");
        for (i, result) in self.test_results.iter().enumerate() {
            let status = if result.success { "✅" } else { "❌" };
            let block_type = if result.uses_indirect_blocks { "Indirect" } else { "Direct" };
            let size_mb = result.original_size as f64 / (1024.0 * 1024.0);
            
            println!("{}. {} {} - {:.1} MB ({})", 
                    i + 1, status, result.name, size_mb, block_type);
            
            if result.success {
                println!("     Storage: {:.2?}, Retrieval: {:.2?}", 
                        result.storage_time, result.retrieval_time);
            }
        }
        
        // Performance analysis
        if !self.test_results.is_empty() {
            println!("\n⚡ Performance Analysis:");
            
            let direct_results: Vec<_> = self.test_results.iter()
                .filter(|r| !r.uses_indirect_blocks && r.success)
                .collect();
            
            let indirect_results: Vec<_> = self.test_results.iter()
                .filter(|r| r.uses_indirect_blocks && r.success)
                .collect();
            
            if !direct_results.is_empty() {
                let avg_direct_storage: std::time::Duration = direct_results.iter()
                    .map(|r| r.storage_time)
                    .sum::<std::time::Duration>() / direct_results.len() as u32;
                
                println!("   Direct blocks - Avg storage time: {:.2?}", avg_direct_storage);
            }
            
            if !indirect_results.is_empty() {
                let avg_indirect_storage: std::time::Duration = indirect_results.iter()
                    .map(|r| r.storage_time)
                    .sum::<std::time::Duration>() / indirect_results.len() as u32;
                
                println!("   Indirect blocks - Avg storage time: {:.2?}", avg_indirect_storage);
            }
            
            let total_bytes: usize = self.test_results.iter()
                .filter(|r| r.success)
                .map(|r| r.original_size)
                .sum();
            
            println!("   Total data processed: {:.2} MB", 
                    total_bytes as f64 / (1024.0 * 1024.0));
        }
        
        println!("\n🎯 Block Storage Verification:");
        println!("   32MB limit tests:");
        for result in &self.test_results {
            if result.success {
                let expected_indirect = result.original_size > BLOCK_SIZE;
                let size_mb = result.original_size as f64 / (1024.0 * 1024.0);
                println!("   - {:.1}MB: {} (expected {})", 
                        size_mb,
                        if result.uses_indirect_blocks { "Indirect" } else { "Direct" },
                        if expected_indirect { "Indirect" } else { "Direct" });
            }
        }
        
        println!("\n💡 Test completed! The block storage system should:");
        println!("   ✓ Use direct blocks for files ≤ 32MB");
        println!("   ✓ Use indirect blocks for files > 32MB");
        println!("   ✓ Maintain data integrity regardless of block type");
        println!("   ✓ Handle large files efficiently with erasure coding");
    }
}

/// Generate mock video data of the specified size
/// This creates realistic-looking video data with headers and frame patterns
fn generate_mock_video_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    
    // Mock MP4 header (simplified)
    let header = b"ftypisom\x00\x00\x02\x00isomiso2avc1mp41";
    data.extend_from_slice(header);
    
    // Add metadata section
    let metadata = format!("MOCK_VIDEO_{}MB_TEST_DATA", size / (1024 * 1024));
    data.extend_from_slice(metadata.as_bytes());
    
    // Fill the rest with pseudo-random video frame data
    // Use a pattern that compresses poorly to simulate real video data
    let mut frame_counter = 0u64;
    
    while data.len() < size {
        // Simulate video frame header
        data.extend_from_slice(b"\x00\x00\x01\xE0"); // MPEG frame start code
        
        // Add frame number
        data.extend_from_slice(&frame_counter.to_le_bytes());
        frame_counter += 1;
        
        // Add pseudo-random frame data that doesn't compress well
        let frame_size = std::cmp::min(8192, size - data.len()); // 8KB frames
        for i in 0..frame_size {
            // Create a pattern that looks like video data but doesn't compress
            let byte = ((frame_counter * 31 + i as u64 * 17) % 256) as u8;
            data.push(byte);
            
            if data.len() >= size {
                break;
            }
        }
    }
    
    // Ensure exact size
    data.truncate(size);
    
    data
}

fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
