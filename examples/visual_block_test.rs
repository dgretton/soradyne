//! Visual Block Storage Test
//! 
//! This example downloads real images, stores them using the block storage system,
//! and then retrieves them for visual verification. It creates before/after files
//! that you can open and compare to verify the storage system works correctly.

use std::path::PathBuf;
use std::sync::Arc;
use tokio;
use sha2::{Sha256, Digest};

use soradyne::storage::block_manager::BlockManager;
use soradyne::types::media::{PhotoStorage, VideoStorage};

/// High-quality test images from reliable sources
const TEST_IMAGES: &[(&str, &str, &str)] = &[
    // Small test images
    ("https://httpbin.org/image/jpeg", "httpbin_test.jpg", "A simple JPEG test image"),
    ("https://httpbin.org/image/png", "httpbin_test.png", "A simple PNG test image"),
    
    // Placeholder images with different sizes
    ("https://via.placeholder.com/300x200/ff0000/ffffff?text=RED+TEST", "red_300x200.png", "Red test image 300x200"),
    ("https://via.placeholder.com/500x300/00ff00/000000?text=GREEN+TEST", "green_500x300.png", "Green test image 500x300"),
    ("https://via.placeholder.com/800x600/0000ff/ffffff?text=BLUE+TEST", "blue_800x600.png", "Blue test image 800x600"),
    
    // Random images from Picsum (Lorem Ipsum for photos)
    ("https://picsum.photos/400/300", "picsum_400x300.jpg", "Random photo 400x300"),
    ("https://picsum.photos/600/400", "picsum_600x400.jpg", "Random photo 600x400"),
    ("https://picsum.photos/800/600", "picsum_800x600.jpg", "Random photo 800x600"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖼️  Visual Block Storage Test");
    println!("=============================");
    println!("This test downloads real images, stores them using block storage,");
    println!("and creates before/after files for visual verification.\n");
    
    let test_session = VisualTestSession::new().await?;
    
    // Run the comprehensive test
    test_session.run_visual_test().await?;
    
    // Show results and instructions
    test_session.show_results();
    
    Ok(())
}

struct VisualTestSession {
    block_manager: Arc<BlockManager>,
    photo_storage: PhotoStorage,
    test_dir: PathBuf,
    results_dir: PathBuf,
    client: reqwest::Client,
    test_results: Vec<TestResult>,
}

#[derive(Debug)]
struct TestResult {
    name: String,
    description: String,
    original_size: usize,
    retrieved_size: usize,
    hash_match: bool,
    storage_time: std::time::Duration,
    retrieval_time: std::time::Duration,
    original_path: PathBuf,
    retrieved_path: PathBuf,
    success: bool,
}

impl VisualTestSession {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create test directory structure
        let test_dir = std::env::current_dir()?.join("visual_block_test");
        let results_dir = test_dir.join("results");
        
        // Clean and recreate directories
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        std::fs::create_dir_all(&test_dir)?;
        std::fs::create_dir_all(&results_dir)?;
        
        // Create rimsd directories
        let mut rimsd_dirs = Vec::new();
        for i in 0..4 {
            let rimsd_dir = test_dir.join(format!("rimsd_{}.rimsd", i));
            std::fs::create_dir_all(&rimsd_dir)?;
            rimsd_dirs.push(rimsd_dir);
        }
        
        // Set up block manager with good redundancy
        let metadata_path = test_dir.join("metadata.json");
        let threshold = 3; // Need 3 out of 4 shards
        let total_shards = 4;
        
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            threshold,
            total_shards,
        )?);
        
        let photo_storage = PhotoStorage::new(block_manager.clone());
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; SoradyneTest/1.0)")
            .build()?;
        
        println!("✅ Test environment created at: {:?}", test_dir);
        println!("📁 Results will be saved to: {:?}\n", results_dir);
        
        Ok(Self {
            block_manager,
            photo_storage,
            test_dir,
            results_dir,
            client,
            test_results: Vec::new(),
        })
    }
    
    async fn run_visual_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Starting visual test with {} images...\n", TEST_IMAGES.len());
        
        for (i, (url, name, description)) in TEST_IMAGES.iter().enumerate() {
            println!("📸 Test {}/{}: {}", i + 1, TEST_IMAGES.len(), name);
            println!("   Description: {}", description);
            println!("   URL: {}", url);
            
            match self.test_single_image(url, name, description).await {
                Ok(result) => {
                    if result.success {
                        println!("   ✅ SUCCESS - Hash match: {}, Size: {} bytes", 
                                result.hash_match, result.original_size);
                    } else {
                        println!("   ❌ FAILED - Check logs for details");
                    }
                    self.test_results.push(result);
                }
                Err(e) => {
                    println!("   ❌ ERROR: {}", e);
                    // Create a failed result entry
                    self.test_results.push(TestResult {
                        name: name.to_string(),
                        description: description.to_string(),
                        original_size: 0,
                        retrieved_size: 0,
                        hash_match: false,
                        storage_time: std::time::Duration::from_secs(0),
                        retrieval_time: std::time::Duration::from_secs(0),
                        original_path: PathBuf::new(),
                        retrieved_path: PathBuf::new(),
                        success: false,
                    });
                }
            }
            println!();
        }
        
        Ok(())
    }
    
    async fn test_single_image(&self, url: &str, name: &str, description: &str) -> Result<TestResult, Box<dyn std::error::Error>> {
        // Download the image
        println!("   📥 Downloading...");
        let response = self.client.get(url).send().await?;
        let image_data = response.bytes().await?.to_vec();
        
        if image_data.is_empty() {
            return Err("Downloaded empty file".into());
        }
        
        println!("   📊 Downloaded {} bytes", image_data.len());
        
        // Calculate hash for integrity verification
        let original_hash = calculate_hash(&image_data);
        
        // Determine MIME type
        let mime_type = if name.ends_with(".jpg") || name.ends_with(".jpeg") {
            "image/jpeg"
        } else if name.ends_with(".png") {
            "image/png"
        } else {
            "image/png" // Default
        };
        
        // Save original for comparison
        let original_path = self.results_dir.join(format!("original_{}", name));
        std::fs::write(&original_path, &image_data)?;
        
        // Store using block storage
        println!("   💾 Storing in block storage...");
        let storage_start = std::time::Instant::now();
        let metadata = self.photo_storage.save_photo(name, mime_type, &image_data).await?;
        let storage_time = storage_start.elapsed();
        
        println!("   🔑 Block ID: {}", metadata.id);
        
        // Retrieve from block storage
        println!("   📤 Retrieving from block storage...");
        let retrieval_start = std::time::Instant::now();
        let retrieved_data = self.photo_storage.load_photo(&metadata).await?;
        let retrieval_time = retrieval_start.elapsed();
        
        // Save retrieved for comparison
        let retrieved_path = self.results_dir.join(format!("retrieved_{}", name));
        std::fs::write(&retrieved_path, &retrieved_data)?;
        
        // Verify integrity
        let retrieved_hash = calculate_hash(&retrieved_data);
        let hash_match = original_hash == retrieved_hash;
        let size_match = image_data.len() == retrieved_data.len();
        let data_match = image_data == retrieved_data;
        
        let success = hash_match && size_match && data_match;
        
        println!("   ⏱️  Storage: {:.2?}, Retrieval: {:.2?}", storage_time, retrieval_time);
        println!("   🔍 Integrity: Hash={}, Size={}, Data={}", 
                hash_match, size_match, data_match);
        
        Ok(TestResult {
            name: name.to_string(),
            description: description.to_string(),
            original_size: image_data.len(),
            retrieved_size: retrieved_data.len(),
            hash_match,
            storage_time,
            retrieval_time,
            original_path,
            retrieved_path,
            success,
        })
    }
    
    fn show_results(&self) {
        println!("📊 TEST RESULTS SUMMARY");
        println!("=======================");
        
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
            println!("{}. {} {} - {} bytes", 
                    i + 1, status, result.name, result.original_size);
            
            if result.success {
                println!("     Storage: {:.2?}, Retrieval: {:.2?}", 
                        result.storage_time, result.retrieval_time);
            }
        }
        
        println!("\n🔍 VISUAL VERIFICATION INSTRUCTIONS:");
        println!("=====================================");
        println!("1. Open the results directory: {:?}", self.results_dir);
        println!("2. For each image, compare:");
        println!("   - original_[name] (downloaded image)");
        println!("   - retrieved_[name] (reconstructed from block storage)");
        println!("3. They should be visually identical!");
        
        println!("\n💡 Quick verification commands:");
        println!("   # Open results directory:");
        println!("   open {:?}", self.results_dir);
        println!("   # Or on Linux:");
        println!("   xdg-open {:?}", self.results_dir);
        
        // Create an HTML comparison page
        if let Err(e) = self.create_comparison_html() {
            println!("⚠️  Could not create HTML comparison: {}", e);
        } else {
            let html_path = self.results_dir.join("comparison.html");
            println!("\n🌐 HTML Comparison created:");
            println!("   open {:?}", html_path);
        }
        
        // Performance summary
        if !self.test_results.is_empty() {
            let avg_storage_time: std::time::Duration = self.test_results.iter()
                .map(|r| r.storage_time)
                .sum::<std::time::Duration>() / self.test_results.len() as u32;
            
            let avg_retrieval_time: std::time::Duration = self.test_results.iter()
                .map(|r| r.retrieval_time)
                .sum::<std::time::Duration>() / self.test_results.len() as u32;
            
            let total_bytes: usize = self.test_results.iter()
                .map(|r| r.original_size)
                .sum();
            
            println!("\n⚡ Performance Summary:");
            println!("   Average storage time: {:.2?}", avg_storage_time);
            println!("   Average retrieval time: {:.2?}", avg_retrieval_time);
            println!("   Total data processed: {} bytes ({:.2} MB)", 
                    total_bytes, total_bytes as f64 / 1_000_000.0);
        }
    }
    
    fn create_comparison_html(&self) -> Result<(), Box<dyn std::error::Error>> {
        let html_path = self.results_dir.join("comparison.html");
        let mut html = String::new();
        
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<title>Soradyne Block Storage Visual Test Results</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }\n");
        html.push_str(".test-result { border: 1px solid #ccc; margin: 20px 0; padding: 15px; }\n");
        html.push_str(".success { border-color: #4CAF50; background-color: #f9fff9; }\n");
        html.push_str(".failure { border-color: #f44336; background-color: #fff9f9; }\n");
        html.push_str(".image-comparison { display: flex; gap: 20px; margin: 10px 0; }\n");
        html.push_str(".image-container { text-align: center; }\n");
        html.push_str("img { max-width: 300px; max-height: 300px; border: 1px solid #ddd; }\n");
        html.push_str(".stats { background-color: #f5f5f5; padding: 10px; margin: 10px 0; }\n");
        html.push_str("</style>\n</head>\n<body>\n");
        
        html.push_str("<h1>🖼️ Soradyne Block Storage Visual Test Results</h1>\n");
        html.push_str(&format!("<p>Test completed with {}/{} successful results</p>\n", 
                              self.test_results.iter().filter(|r| r.success).count(),
                              self.test_results.len()));
        
        for result in &self.test_results {
            let class = if result.success { "success" } else { "failure" };
            let status = if result.success { "✅ SUCCESS" } else { "❌ FAILED" };
            
            html.push_str(&format!("<div class=\"test-result {}\">\n", class));
            html.push_str(&format!("<h2>{} {}</h2>\n", status, result.name));
            html.push_str(&format!("<p><strong>Description:</strong> {}</p>\n", result.description));
            
            if result.success {
                html.push_str("<div class=\"image-comparison\">\n");
                html.push_str("<div class=\"image-container\">\n");
                html.push_str(&format!("<h3>Original</h3>\n"));
                html.push_str(&format!("<img src=\"{}\" alt=\"Original {}\">\n", 
                                      result.original_path.file_name().unwrap().to_string_lossy(),
                                      result.name));
                html.push_str(&format!("<p>{} bytes</p>\n", result.original_size));
                html.push_str("</div>\n");
                
                html.push_str("<div class=\"image-container\">\n");
                html.push_str(&format!("<h3>Retrieved</h3>\n"));
                html.push_str(&format!("<img src=\"{}\" alt=\"Retrieved {}\">\n", 
                                      result.retrieved_path.file_name().unwrap().to_string_lossy(),
                                      result.name));
                html.push_str(&format!("<p>{} bytes</p>\n", result.retrieved_size));
                html.push_str("</div>\n");
                html.push_str("</div>\n");
                
                html.push_str("<div class=\"stats\">\n");
                html.push_str(&format!("<p><strong>Hash Match:</strong> {}</p>\n", result.hash_match));
                html.push_str(&format!("<p><strong>Storage Time:</strong> {:.2?}</p>\n", result.storage_time));
                html.push_str(&format!("<p><strong>Retrieval Time:</strong> {:.2?}</p>\n", result.retrieval_time));
                html.push_str("</div>\n");
            }
            
            html.push_str("</div>\n");
        }
        
        html.push_str("</body>\n</html>\n");
        
        std::fs::write(html_path, html)?;
        Ok(())
    }
}

fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
