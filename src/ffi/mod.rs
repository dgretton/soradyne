use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use serde_json;

use crate::album::album::*;
use crate::album::operations::*;
use crate::album::crdt::{Crdt, CrdtCollection};
use crate::storage::block_manager::BlockManager;
use crate::album::renderer::MediaRenderer;

// Generate resized media based on type and resolution
fn generate_resized_media(media_data: &[u8], max_size: u32, media_type: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match media_type {
        "video" => generate_video_at_size(media_data, max_size),
        "audio" => generate_audio_at_size(media_data, max_size),
        _ => generate_image_at_size(media_data, max_size),
    }
}

fn generate_image_at_size(image_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Load the image from the data
    let img = image::load_from_memory(image_data)?;
    
    // Resize while maintaining aspect ratio
    let resized = img.thumbnail(max_size, max_size);
    
    // Use higher quality for larger sizes
    let quality = match max_size {
        0..=200 => 70,
        201..=800 => 85,
        _ => 95,
    };
    
    // Encode as JPEG
    let mut buffer = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Jpeg(quality))?;
    
    Ok(buffer)
}

fn generate_video_at_size(video_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to extract a frame using FFmpeg first
    if let Ok(frame_data) = extract_video_frame(video_data) {
        // Generate resized image from the extracted frame
        return generate_image_at_size(&frame_data, max_size);
    }
    
    // Fall back to placeholder if FFmpeg extraction fails
    create_video_placeholder_at_size(max_size)
}

fn extract_video_frame(video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Create temporary files
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join(format!("video_input_{}.mp4", uuid::Uuid::new_v4()));
    let output_path = temp_dir.join(format!("frame_output_{}.jpg", uuid::Uuid::new_v4()));
    
    // Write video data to temporary file
    std::fs::write(&input_path, video_data)?;
    
    // Extract frame at 1 second using FFmpeg
    let output = std::process::Command::new("ffmpeg")
        .args(&[
            "-i", input_path.to_str().unwrap(),
            "-ss", "00:00:01.000",  // Seek to 1 second
            "-vframes", "1",        // Extract 1 frame
            "-q:v", "2",           // High quality
            "-y",                  // Overwrite output
            output_path.to_str().unwrap()
        ])
        .output();
    
    // Clean up input file
    let _ = std::fs::remove_file(&input_path);
    
    match output {
        Ok(result) if result.status.success() => {
            // Read the extracted frame
            let frame_data = std::fs::read(&output_path)?;
            // Clean up output file
            let _ = std::fs::remove_file(&output_path);
            Ok(frame_data)
        }
        _ => {
            // Clean up output file if it exists
            let _ = std::fs::remove_file(&output_path);
            Err("FFmpeg frame extraction failed".into())
        }
    }
}

fn generate_audio_at_size(audio_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // For now, just create a placeholder - could implement waveform generation later
    create_audio_placeholder_at_size(max_size)
}

fn create_video_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbaImage, Rgba};
    
    // Create a size x size gray image with a play icon
    let mut img = RgbaImage::new(size, size);
    let gray = Rgba([128, 128, 128, 255]);
    let white = Rgba([255, 255, 255, 255]);
    
    // Fill with gray
    for pixel in img.pixels_mut() {
        *pixel = gray;
    }
    
    // Draw a simple play triangle in the center
    let center_x = size / 2;
    let center_y = size / 2;
    let triangle_size = size / 5;
    
    // Simple triangle points
    for y in (center_y.saturating_sub(triangle_size))..(center_y + triangle_size) {
        for x in (center_x.saturating_sub(triangle_size/2))..(center_x + triangle_size) {
            if x < size && y < size {
                // Simple triangle shape
                let dx = x as i32 - center_x as i32;
                let dy = y as i32 - center_y as i32;
                if dx > -(triangle_size as i32)/2 && dx < triangle_size as i32 && dy.abs() < triangle_size as i32 - dx.abs()/2 {
                    img.put_pixel(x, y, white);
                }
            }
        }
    }
    
    // Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    
    Ok(buffer)
}

fn create_audio_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbaImage, Rgba};
    
    // Create a size x size image with audio waveform visualization
    let mut img = RgbaImage::new(size, size);
    let dark_bg = Rgba([20, 25, 35, 255]);
    let waveform_color = Rgba([100, 150, 255, 255]);
    
    // Fill with dark background
    for pixel in img.pixels_mut() {
        *pixel = dark_bg;
    }
    
    // Draw simplified waveform bars
    let bar_width = (size / 40).max(2);
    let bar_spacing = (size / 25).max(3);
    let num_bars = size / bar_spacing;
    let center_y = size / 2;
    
    for i in 0..num_bars {
        let bar_x = i * bar_spacing + size / 15;
        let bar_height = (size / 8) + ((i * 7) % (size / 4)); // Varying heights
        let bar_top = center_y.saturating_sub(bar_height / 2);
        let bar_bottom = center_y + bar_height / 2;
        
        for y in bar_top..bar_bottom.min(size) {
            for x in bar_x..(bar_x + bar_width).min(size) {
                img.put_pixel(x, y, waveform_color);
            }
        }
    }
    
    // Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    
    Ok(buffer)
}

// Create a simple placeholder image for videos when thumbnail generation fails
fn create_video_placeholder() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    create_video_placeholder_at_size(150)
}

// Legacy function - kept for compatibility
fn _create_video_placeholder_legacy() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbaImage, Rgba};
    
    // Create a 150x150 gray image with a play icon
    let mut img = RgbaImage::new(150, 150);
    let gray = Rgba([128, 128, 128, 255]);
    let white = Rgba([255, 255, 255, 255]);
    
    // Fill with gray
    for pixel in img.pixels_mut() {
        *pixel = gray;
    }
    
    // Draw a simple play triangle in the center
    let center_x = 75;
    let center_y = 75;
    let size = 20;
    
    // Simple triangle points
    for y in (center_y - size as i32)..(center_y + size as i32) {
        for x in (center_x - size as i32/2)..(center_x + size as i32) {
            if x >= 0 && x < 150 && y >= 0 && y < 150 {
                // Simple triangle shape
                let dx: i32 = x - center_x;
                let dy: i32 = y - center_y;
                if dx > -(size as i32)/2 && dx < size as i32 && dy.abs() < size as i32 - dx.abs()/2 {
                    img.put_pixel(x as u32, y as u32, white);
                }
            }
        }
    }
    
    // Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    
    Ok(buffer)
}

// Global state for the album system
static mut ALBUM_SYSTEM: Option<Arc<Mutex<AlbumSystem>>> = None;

pub struct AlbumSystem {
    albums: HashMap<String, MediaAlbum>,
    block_manager: Arc<BlockManager>,
    runtime: Arc<Runtime>,
    data_dir: PathBuf,
    albums_index_block_id: Option<[u8; 32]>,
}

impl AlbumSystem {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create data directory
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(home_dir).join(".soradyne_albums");
        std::fs::create_dir_all(&data_dir)?;
        
        // Create rimsd directories
        let mut rimsd_dirs = Vec::new();
        for i in 0..4 {
            let device_dir = data_dir.join(format!("rimsd_{}", i));
            let rimsd_dir = device_dir.join(".rimsd");
            std::fs::create_dir_all(&rimsd_dir)?;
            rimsd_dirs.push(rimsd_dir);
        }
        
        let metadata_path = data_dir.join("metadata.json");
        
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            3, // threshold
            4, // total_shards
        )?);
        
        let runtime = Arc::new(Runtime::new()?);
        
        let mut system = Self {
            albums: HashMap::new(),
            block_manager,
            runtime,
            data_dir,
            albums_index_block_id: None,
        };
        
        // Load existing albums from block storage
        system.load_albums_from_blocks()?;
        
        Ok(system)
    }
    
    fn load_albums_from_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Try to load the albums index from a known location
        let index_file = self.data_dir.join("albums_index.txt");
        
        println!("Looking for albums index at: {:?}", index_file);
        println!("Data directory contents: {:?}", std::fs::read_dir(&self.data_dir).map(|entries| 
            entries.map(|e| e.map(|entry| entry.file_name())).collect::<Result<Vec<_>, _>>()
        ));
        
        if index_file.exists() {
            println!("Albums index file exists, reading...");
            let index_content = std::fs::read_to_string(&index_file)?;
            println!("Index content: {}", index_content.trim());
            
            if let Ok(block_id_bytes) = hex::decode(index_content.trim()) {
                if block_id_bytes.len() == 32 {
                    let mut block_id = [0u8; 32];
                    block_id.copy_from_slice(&block_id_bytes);
                    self.albums_index_block_id = Some(block_id);
                    
                    println!("Loading albums index from block: {}", hex::encode(block_id));
                    
                    // Load the albums index using the runtime
                    if let Ok(index_data) = self.runtime.block_on(async {
                        self.block_manager.read_block(&block_id).await
                    }) {
                        println!("Successfully read index data: {} bytes", index_data.len());
                        
                        if let Ok(index_json) = String::from_utf8(index_data) {
                            println!("Index JSON: {}", index_json);
                            
                            if let Ok(album_index) = serde_json::from_str::<HashMap<String, [u8; 32]>>(&index_json) {
                                println!("Found {} albums in index", album_index.len());
                                
                                // Load each album from its block
                                for (album_id, album_block_id) in album_index {
                                    println!("Loading album {} from block {}", album_id, hex::encode(album_block_id));
                                    
                                    if let Ok(album_data) = self.runtime.block_on(async {
                                        self.block_manager.read_block(&album_block_id).await
                                    }) {
                                        if let Ok(album_json) = String::from_utf8(album_data) {
                                            if let Ok(mut album) = serde_json::from_str::<MediaAlbum>(&album_json) {
                                                // Restore the block manager reference
                                                album.block_manager = Some(Arc::clone(&self.block_manager));
                                                self.albums.insert(album_id.clone(), album);
                                                println!("Successfully loaded album: {}", album_id);
                                            } else {
                                                println!("Failed to parse album JSON for {}", album_id);
                                            }
                                        } else {
                                            println!("Failed to decode album data as UTF-8 for {}", album_id);
                                        }
                                    } else {
                                        println!("Failed to read album block for {}", album_id);
                                    }
                                }
                                
                                println!("Loaded {} albums from block storage", self.albums.len());
                            } else {
                                println!("Failed to parse albums index JSON");
                            }
                        } else {
                            println!("Failed to decode index data as UTF-8");
                        }
                    } else {
                        println!("Failed to read albums index block");
                    }
                } else {
                    println!("Invalid block ID length: {}", block_id_bytes.len());
                }
            } else {
                println!("Failed to decode hex block ID");
            }
        } else {
            println!("Albums index file does not exist");
        }
        
        Ok(())
    }
    
    fn save_albums_to_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create an index mapping album IDs to their block IDs
        let mut album_index = HashMap::new();
        
        // Save each album to its own block
        for (album_id, album) in &self.albums {
            // Create a serializable version without the block manager
            let mut serializable_album = album.clone();
            serializable_album.block_manager = None;
            
            let album_json = serde_json::to_string_pretty(&serializable_album)?;
            let album_data = album_json.as_bytes();
            
            // Store the album in a block using the runtime
            let album_block_id = self.runtime.block_on(async {
                self.block_manager.write_direct_block(album_data).await
            })?;
            album_index.insert(album_id.clone(), album_block_id);
        }
        
        // Save the index to a block
        let index_json = serde_json::to_string_pretty(&album_index)?;
        let index_data = index_json.as_bytes();
        let index_block_id = self.runtime.block_on(async {
            self.block_manager.write_direct_block(index_data).await
        })?;
        
        // Store the index block ID in a simple file for bootstrapping
        let index_file = self.data_dir.join("albums_index.txt");
        std::fs::write(&index_file, hex::encode(index_block_id))?;
        
        self.albums_index_block_id = Some(index_block_id);
        
        println!("Saved {} albums to block storage", self.albums.len());
        
        Ok(())
    }
}

// FFI function to initialize the album system
#[no_mangle]
pub extern "C" fn soradyne_init() -> i32 {
    match AlbumSystem::new() {
        Ok(system) => {
            unsafe {
                ALBUM_SYSTEM = Some(Arc::new(Mutex::new(system)));
            }
            0 // Success
        }
        Err(_) => -1 // Error
    }
}

// FFI function to get all albums as JSON
#[no_mangle]
pub extern "C" fn soradyne_get_albums() -> *mut c_char {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(system) = system.lock() {
                let albums: Vec<serde_json::Value> = system.albums.iter().map(|(id, album)| {
                    serde_json::json!({
                        "id": id,
                        "name": album.metadata.title,
                        "item_count": album.items.len()
                    })
                }).collect();
                
                if let Ok(json) = serde_json::to_string(&albums) {
                    if let Ok(c_string) = CString::new(json) {
                        return c_string.into_raw();
                    }
                }
            }
        }
    }
    
    // Return empty array on error
    let empty = CString::new("[]").unwrap();
    empty.into_raw()
}

// FFI function to create a new album
#[no_mangle]
pub extern "C" fn soradyne_create_album(name_ptr: *const c_char) -> *mut c_char {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(mut system) = system.lock() {
                let name = CStr::from_ptr(name_ptr).to_string_lossy().to_string();
                let album_id = uuid::Uuid::new_v4().to_string();
                
                let album = MediaAlbum {
                    album_id: album_id.clone(),
                    items: HashMap::new(),
                    metadata: AlbumMetadata {
                        title: name.clone(),
                        created_by: "flutter_user".to_string(),
                        created_at: chrono::Utc::now().timestamp() as u64,
                        shared_with: HashMap::new(),
                    },
                    block_manager: Some(Arc::clone(&system.block_manager)),
                };
                
                system.albums.insert(album_id.clone(), album);
                
                // Save albums to persistent storage
                if let Err(e) = system.save_albums_to_blocks() {
                    eprintln!("Failed to save albums to blocks: {}", e);
                }
                
                let response = serde_json::json!({
                    "id": album_id,
                    "name": name,
                    "item_count": 0
                });
                
                if let Ok(json) = serde_json::to_string(&response) {
                    if let Ok(c_string) = CString::new(json) {
                        return c_string.into_raw();
                    }
                }
            }
        }
    }
    
    let error = CString::new(r#"{"error": "Failed to create album"}"#).unwrap();
    error.into_raw()
}

// FFI function to get album items
#[no_mangle]
pub extern "C" fn soradyne_get_album_items(album_id_ptr: *const c_char) -> *mut c_char {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(system) = system.lock() {
                let album_id = CStr::from_ptr(album_id_ptr).to_string_lossy().to_string();
                
                if let Some(album) = system.albums.get(&album_id) {
                    let items: Vec<serde_json::Value> = album.items.iter().map(|(media_id, crdt)| {
                        let _state = crdt.reduce();
                        
                        // Extract metadata from operations
                        let mut filename = format!("media_{}", media_id);
                        let mut media_type = "image/jpeg";
                        let mut size = 0u64;
                        
                        for op in crdt.ops() {
                            if op.op_type == "add_media" {
                                if let Some(f) = op.payload.get("filename").and_then(|v| v.as_str()) {
                                    filename = f.to_string();
                                }
                                if let Some(t) = op.payload.get("media_type").and_then(|v| v.as_str()) {
                                    media_type = match t {
                                        "video" => "video/mp4",
                                        "audio" => "audio/mpeg",
                                        _ => "image/jpeg"
                                    };
                                }
                                if let Some(s) = op.payload.get("size").and_then(|v| v.as_u64()) {
                                    size = s;
                                }
                            }
                        }
                        
                        serde_json::json!({
                            "id": media_id,
                            "filename": filename,
                            "media_type": media_type,
                            "size": size,
                            "rotation": _state.rotation,
                            "has_crop": _state.crop.is_some(),
                            "markup_count": _state.markup.len(),
                            "comments": []
                        })
                    }).collect();
                    
                    if let Ok(json) = serde_json::to_string(&items) {
                        if let Ok(c_string) = CString::new(json) {
                            return c_string.into_raw();
                        }
                    }
                }
            }
        }
    }
    
    let empty = CString::new("[]").unwrap();
    empty.into_raw()
}

// FFI function to upload media (takes file path)
#[no_mangle]
pub extern "C" fn soradyne_upload_media(album_id_ptr: *const c_char, file_path_ptr: *const c_char) -> i32 {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(mut system) = system.lock() {
                let album_id = CStr::from_ptr(album_id_ptr).to_string_lossy().to_string();
                let file_path = CStr::from_ptr(file_path_ptr).to_string_lossy().to_string();
                
                // Read file data
                if let Ok(file_data) = std::fs::read(&file_path) {
                    println!("Read file data: {} bytes", file_data.len());
                    
                    let media_id = uuid::Uuid::new_v4().to_string();
                    let filename = PathBuf::from(&file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    
                    // Store in block storage using the runtime
                    let block_manager = Arc::clone(&system.block_manager);
                    let runtime = Arc::clone(&system.runtime);
                    
                    println!("Attempting to write {} bytes to block storage", file_data.len());
                    
                    // Check if file is too large for direct block storage
                    const MAX_DIRECT_BLOCK_SIZE: usize = 1024 * 1024; // 1MB limit
                    
                    let result = if file_data.len() > MAX_DIRECT_BLOCK_SIZE {
                        println!("File too large for direct block, using indirect storage");
                        // For now, we'll use the block file system for large files
                        runtime.block_on(async {
                            use crate::storage::block_file::BlockFile;
                            let block_file = BlockFile::new(Arc::clone(&block_manager));
                            block_file.append(&file_data).await?;
                            block_file.root_block().await.ok_or_else(|| 
                                crate::flow::error::FlowError::PersistenceError("No root block".to_string())
                            )
                        })
                    } else {
                        runtime.block_on(async {
                            block_manager.write_direct_block(&file_data).await
                        })
                    };
                    
                    match result {
                        Ok(block_id) => {
                            println!("Successfully wrote block: {}", hex::encode(block_id));
                            // Detect media type from file extension
                            let media_type = if filename.to_lowercase().ends_with(".mov") || 
                                               filename.to_lowercase().ends_with(".mp4") ||
                                               filename.to_lowercase().ends_with(".avi") {
                                "video"
                            } else if filename.to_lowercase().ends_with(".mp3") ||
                                     filename.to_lowercase().ends_with(".wav") ||
                                     filename.to_lowercase().ends_with(".flac") {
                                "audio"
                            } else {
                                "image"
                            };
                            
                            // Create operation
                            let op = EditOp {
                                op_id: uuid::Uuid::new_v4(),
                                timestamp: chrono::Utc::now().timestamp() as u64,
                                author: "flutter_user".to_string(),
                                op_type: "add_media".to_string(),
                                payload: serde_json::json!({
                                    "filename": filename,
                                    "block_id": hex::encode(block_id),
                                    "size": file_data.len(),
                                    "media_type": media_type
                                }),
                            };
                            
                            // Add to album
                            if let Some(album) = system.albums.get_mut(&album_id) {
                                let crdt = album.get_or_create(&media_id);
                                if crdt.apply_local(op).is_ok() {
                                    // Save albums to persistent storage
                                    if let Err(e) = system.save_albums_to_blocks() {
                                        eprintln!("Failed to save albums to blocks: {}", e);
                                    }
                                    println!("Successfully uploaded media: {}", media_id);
                                    return 0; // Success
                                } else {
                                    println!("Failed to apply CRDT operation");
                                }
                            } else {
                                println!("Album not found: {}", album_id);
                            }
                        }
                        Err(e) => {
                            println!("Failed to write block: {}", e);
                        }
                    }
                } else {
                    println!("Failed to read file: {}", file_path);
                }
            }
        }
    }
    
    -1 // Error
}

// FFI function to free strings allocated by Rust
#[no_mangle]
pub extern "C" fn soradyne_free_string(ptr: *mut c_char) {
    unsafe {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
}

// FFI function to get media thumbnail (150px)
#[no_mangle]
pub extern "C" fn soradyne_get_media_thumbnail(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize) -> i32 {
    get_media_at_resolution(album_id_ptr, media_id_ptr, data_ptr, size_ptr, 150)
}

// FFI function to get media medium resolution (600px)
#[no_mangle]
pub extern "C" fn soradyne_get_media_medium(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize) -> i32 {
    get_media_at_resolution(album_id_ptr, media_id_ptr, data_ptr, size_ptr, 600)
}

// FFI function to get media high resolution (1200px)
#[no_mangle]
pub extern "C" fn soradyne_get_media_high(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize) -> i32 {
    get_media_at_resolution(album_id_ptr, media_id_ptr, data_ptr, size_ptr, 1200)
}

// FFI function to get media data (returns raw bytes for images, thumbnails for videos) - kept for compatibility
#[no_mangle]
pub extern "C" fn soradyne_get_media_data(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize) -> i32 {
    // Default to medium resolution for backward compatibility
    get_media_at_resolution(album_id_ptr, media_id_ptr, data_ptr, size_ptr, 600)
}

// Internal function to get media at specific resolution
fn get_media_at_resolution(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize, max_size: u32) -> i32 {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(system) = system.lock() {
                let album_id = CStr::from_ptr(album_id_ptr).to_string_lossy().to_string();
                let media_id = CStr::from_ptr(media_id_ptr).to_string_lossy().to_string();
                
                if let Some(album) = system.albums.get(&album_id) {
                    if let Some(crdt) = album.items.get(&media_id) {
                        let _state = crdt.reduce();
                        
                        // Get the block_id and media_type from the first operation's payload
                        if let Some(op) = crdt.ops().first() {
                            if let Some(block_id_hex) = op.payload.get("block_id").and_then(|v| v.as_str()) {
                                if let Ok(block_id_bytes) = hex::decode(block_id_hex) {
                                    if block_id_bytes.len() == 32 {
                                        let mut block_id = [0u8; 32];
                                        block_id.copy_from_slice(&block_id_bytes);
                                        
                                        let block_manager = Arc::clone(&system.block_manager);
                                        let runtime = Arc::clone(&system.runtime);
                                        
                                        // Check if this is a video file
                                        let media_type = op.payload.get("media_type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("image");
                                        
                                        if media_type == "video" {
                                            // For videos, try to generate a thumbnail, but fall back to a placeholder
                                            if let Ok(video_data) = runtime.block_on(async {
                                                block_manager.read_block(&block_id).await
                                            }) {
                                                // Generate resized video thumbnail/frame
                                                if let Ok(resized_data) = generate_resized_media(&video_data, max_size, media_type) {
                                                    let boxed_data = resized_data.into_boxed_slice();
                                                    let len = boxed_data.len();
                                                    let ptr = Box::into_raw(boxed_data) as *mut u8;
                                                
                                                    *data_ptr = ptr;
                                                    *size_ptr = len;
                                                
                                                    return 0; // Success
                                                }
                                            
                                                // If generation failed, create a simple placeholder image
                                                println!("Creating placeholder image for video {} at size {}", media_id, max_size);
                                                if let Ok(placeholder_data) = create_video_placeholder_at_size(max_size) {
                                                    let boxed_data = placeholder_data.into_boxed_slice();
                                                    let len = boxed_data.len();
                                                    let ptr = Box::into_raw(boxed_data) as *mut u8;
                                                
                                                    *data_ptr = ptr;
                                                    *size_ptr = len;
                                                
                                                    return 0; // Success with placeholder
                                                }
                                            }
                                        } else {
                                            // For images and other media, return resized data
                                            if let Ok(data) = runtime.block_on(async {
                                                block_manager.read_block(&block_id).await
                                            }) {
                                                // Generate resized image
                                                if let Ok(resized_data) = generate_resized_media(&data, max_size, media_type) {
                                                    let boxed_data = resized_data.into_boxed_slice();
                                                    let len = boxed_data.len();
                                                    let ptr = Box::into_raw(boxed_data) as *mut u8;
                                                
                                                    *data_ptr = ptr;
                                                    *size_ptr = len;
                                                
                                                    return 0; // Success
                                                }
                                            
                                                // Fall back to original data if resizing fails
                                                let boxed_data = data.into_boxed_slice();
                                                let len = boxed_data.len();
                                                let ptr = Box::into_raw(boxed_data) as *mut u8;
                                            
                                                *data_ptr = ptr;
                                                *size_ptr = len;
                                            
                                                return 0; // Success
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    -1 // Error
}

// FFI function to free media data allocated by Rust
#[no_mangle]
pub extern "C" fn soradyne_free_media_data(data_ptr: *mut u8, size: usize) {
    unsafe {
        if !data_ptr.is_null() {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(data_ptr, size));
        }
    }
}

// FFI function to cleanup
#[no_mangle]
pub extern "C" fn soradyne_cleanup() {
    unsafe {
        ALBUM_SYSTEM = None;
    }
}
