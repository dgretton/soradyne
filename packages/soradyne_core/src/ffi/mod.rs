use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use serde_json;

use crate::album::album::*;

// Giantt Flow FFI module
pub mod giantt_flow;
mod serializer;

// Inventory Flow FFI module
pub mod inventory_flow;

// Pairing Bridge FFI module
pub mod pairing_bridge;
use crate::album::operations::*;
use crate::album::crdt::{Crdt, CrdtCollection};
use crate::storage::block_manager::BlockManager;
use crate::video::{generate_video_at_size, generate_image_at_size, create_audio_placeholder_at_size, create_video_placeholder_at_size, is_video_file, is_audio_file};

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
    data_dir: PathBuf,
    albums_index_block_id: Option<[u8; 32]>,
}

impl AlbumSystem {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("Creating AlbumSystem...");
        
        // Use a writable location for metadata - app's container directory
        let metadata_path = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
                .join("Library/Containers/com.example.soradyneApp/Data/.soradyne_metadata.json")
        } else {
            PathBuf::from("/tmp/soradyne_metadata.json")
        };
        println!("Metadata path: {:?}", metadata_path);
        
        // Ensure the parent directory exists
        if let Some(parent) = metadata_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                println!("Failed to create metadata directory: {}", e);
                e
            })?;
        }
        
        println!("🔍 Discovering SD cards...");
        let rimsd_dirs = crate::storage::device_identity::discover_soradyne_volumes().await
            .map_err(|e| format!("SD card discovery failed: {}", e))?;

        if rimsd_dirs.is_empty() {
            return Err("No Soradyne SD cards found! Please insert SD cards with .rimsd directories".into());
        }

        println!("✅ Found {} SD cards", rimsd_dirs.len());
        let threshold = std::cmp::min(3, rimsd_dirs.len()); // Adaptive threshold, prefer 3 but adapt to available
        let total_shards = rimsd_dirs.len();

        println!("Creating BlockManager with {} shards (threshold: {})...", total_shards, threshold);
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            threshold,
            total_shards,
        ).map_err(|e| {
            println!("Failed to create BlockManager: {}", e);
            e
        })?);
        println!("BlockManager created successfully");
        
        let mut system = Self {
            albums: HashMap::new(),
            block_manager,
            data_dir: PathBuf::from("/tmp"), // Temporary directory since we're using SD cards
            albums_index_block_id: None,
        };
        
        println!("Loading existing albums from block storage...");
        // Load existing albums from block storage
        system.load_albums_from_blocks().await.map_err(|e| {
            println!("Failed to load albums from blocks: {}", e);
            e
        })?;
        println!("Albums loaded successfully");
        
        println!("AlbumSystem initialization complete");
        Ok(system)
    }
    
    async fn load_albums_from_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Try to load from local storage first (for existing albums)
        let local_data_dir = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join("Library/Containers/com.example.soradyneApp/Data/.soradyne_albums")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".soradyne_albums")
        };
        
        let index_file = local_data_dir.join("albums_index.txt");
        println!("Looking for albums index at: {:?}", index_file);
        
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
                    
                    // Load the albums index
                    if let Ok(index_data) = self.block_manager.read_block(&block_id).await {
                        println!("Successfully read index data: {} bytes", index_data.len());
                        
                        if let Ok(index_json) = String::from_utf8(index_data) {
                            println!("Index JSON: {}", index_json);
                            
                            if let Ok(album_index) = serde_json::from_str::<HashMap<String, [u8; 32]>>(&index_json) {
                                println!("Found {} albums in index", album_index.len());
                                
                                // Load each album from its block
                                for (album_id, album_block_id) in album_index {
                                    println!("Loading album {} from block {}", album_id, hex::encode(album_block_id));
                                    
                                    if let Ok(album_data) = self.block_manager.read_block(&album_block_id).await {
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
            // TODO: In the future, scan SD cards for album metadata blocks
            println!("🔍 Future: Will scan SD cards for existing album metadata");
        }
        
        Ok(())
    }
    
    async fn save_albums_to_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create an index mapping album IDs to their block IDs
        let mut album_index = HashMap::new();
        
        // Save each album to its own block
        for (album_id, album) in &self.albums {
            // Create a serializable version without the block manager
            let mut serializable_album = album.clone();
            serializable_album.block_manager = None;
            
            let album_json = serde_json::to_string_pretty(&serializable_album)?;
            let album_data = album_json.as_bytes();
            
            // Store the album in a block
            let album_block_id = self.block_manager.write_direct_block(album_data).await?;
            album_index.insert(album_id.clone(), album_block_id);
        }
        
        // Save the index to a block
        let index_json = serde_json::to_string_pretty(&album_index)?;
        let index_data = index_json.as_bytes();
        let index_block_id = self.block_manager.write_direct_block(index_data).await?;
        
        // Store the index block ID in the local directory for bootstrapping
        let local_data_dir = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join("Library/Containers/com.example.soradyneApp/Data/.soradyne_albums")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".soradyne_albums")
        };
        
        // Ensure the directory exists
        std::fs::create_dir_all(&local_data_dir)?;
        
        let index_file = local_data_dir.join("albums_index.txt");
        std::fs::write(&index_file, hex::encode(index_block_id))?;
        
        self.albums_index_block_id = Some(index_block_id);
        
        println!("Saved {} albums to block storage", self.albums.len());
        
        Ok(())
    }
}

// FFI function to initialize the album system
#[no_mangle]
pub extern "C" fn soradyne_init() -> i32 {
    println!("Starting Soradyne initialization...");
    
    // Create a runtime to handle the async initialization
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("Failed to create Tokio runtime: {}", e);
            return -1;
        }
    };
    
    match rt.block_on(AlbumSystem::new()) {
        Ok(system) => {
            println!("AlbumSystem created successfully");
            unsafe {
                ALBUM_SYSTEM = Some(Arc::new(Mutex::new(system)));
            }
            println!("Soradyne initialization completed successfully");
            0 // Success
        }
        Err(e) => {
            println!("Soradyne initialization failed: {}", e);
            eprintln!("Soradyne initialization failed: {}", e);
            -1 // Error
        }
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
                
                // Save to persistent storage immediately
                let rt = Runtime::new().unwrap();
                if let Err(e) = rt.block_on(async {
                    system.save_albums_to_blocks().await
                }) {
                    println!("Failed to save albums to blocks: {}", e);
                } else {
                    println!("Album created and saved successfully");
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
                    
                    // Store in block storage
                    let block_manager = Arc::clone(&system.block_manager);
                    
                    println!("Attempting to write {} bytes to block storage", file_data.len());
                    
                    // Create temporary runtime for FFI
                    let rt = Runtime::new().unwrap();
                    let result = rt.block_on(async {
                        block_manager.write_direct_block(&file_data).await
                    });
                    
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
                                    println!("Successfully uploaded media: {}", media_id);
                                    
                                    // Save albums to persistent storage immediately after adding media
                                    let rt = Runtime::new().unwrap();
                                    if let Err(e) = rt.block_on(async {
                                        system.save_albums_to_blocks().await
                                    }) {
                                        println!("Failed to save albums after media upload: {}", e);
                                    } else {
                                        println!("Albums saved successfully after media upload");
                                    }
                                    
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
                                        
                                        // Read the media data from block storage
                                        let rt = Runtime::new().unwrap();
                                        if let Ok(media_data) = rt.block_on(async {
                                            block_manager.read_block(&block_id).await
                                        }) {
                                            println!("Successfully read {} bytes from block storage for media {}", media_data.len(), media_id);
                                            
                                            // Get the filename from the operation payload for better type detection
                                            let filename = op.payload.get("filename")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            
                                            println!("Processing media file: {}", filename);
                                            
                                            // Use filename extension for primary detection, fallback to content detection
                                            // Check images FIRST to avoid false positives from audio detection
                                            let resized_data = if filename.to_lowercase().ends_with(".mp4") ||
                                                                 filename.to_lowercase().ends_with(".mov") ||
                                                                 filename.to_lowercase().ends_with(".avi") ||
                                                                 filename.to_lowercase().ends_with(".mkv") ||
                                                                 is_video_file(&media_data) {
                                                println!("Detected video file, generating video thumbnail at size {}", max_size);
                                                generate_video_at_size(&media_data, max_size)
                                            } else if filename.to_lowercase().ends_with(".jpg") ||
                                                     filename.to_lowercase().ends_with(".jpeg") ||
                                                     filename.to_lowercase().ends_with(".png") ||
                                                     filename.to_lowercase().ends_with(".gif") ||
                                                     filename.to_lowercase().ends_with(".bmp") ||
                                                     filename.to_lowercase().ends_with(".webp") ||
                                                     filename.to_lowercase().ends_with(".tiff") ||
                                                     filename.to_lowercase().ends_with(".tif") {
                                                println!("Detected image file (by extension), generating resized image at size {}", max_size);
                                                generate_image_at_size(&media_data, max_size)
                                            } else if filename.to_lowercase().ends_with(".mp3") ||
                                                     filename.to_lowercase().ends_with(".wav") ||
                                                     filename.to_lowercase().ends_with(".flac") ||
                                                     filename.to_lowercase().ends_with(".aac") ||
                                                     filename.to_lowercase().ends_with(".ogg") ||
                                                     is_audio_file(&media_data) {
                                                println!("Detected audio file, generating audio placeholder at size {}", max_size);
                                                create_audio_placeholder_at_size(max_size)
                                            } else {
                                                println!("Detected image file (fallback), generating resized image at size {}", max_size);
                                                generate_image_at_size(&media_data, max_size)
                                            };
                                            
                                            match resized_data {
                                                Ok(data) => {
                                                    let boxed_data = data.into_boxed_slice();
                                                    let len = boxed_data.len();
                                                    let ptr = Box::into_raw(boxed_data) as *mut u8;
                                                
                                                    *data_ptr = ptr;
                                                    *size_ptr = len;
                                                
                                                    return 0; // Success
                                                }
                                                Err(e) => {
                                                    println!("Failed to generate resized media: {}", e);
                                                    
                                                    // Fall back to original data for images only
                                                    if !is_video_file(&media_data) && !is_audio_file(&media_data) {
                                                        let boxed_data = media_data.into_boxed_slice();
                                                        let len = boxed_data.len();
                                                        let ptr = Box::into_raw(boxed_data) as *mut u8;
                                                    
                                                        *data_ptr = ptr;
                                                        *size_ptr = len;
                                                    
                                                        return 0; // Success with original data
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

// FFI function to get storage status
#[no_mangle]
pub extern "C" fn soradyne_get_storage_status() -> *mut c_char {
    // Create a runtime to handle the async discovery
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("Failed to create runtime for storage status: {}", e);
            let error_status = serde_json::json!({
                "available_devices": 0,
                "required_threshold": 3,
                "can_read_data": false,
                "missing_devices": 3,
                "device_paths": [],
                "error": "Failed to create runtime"
            });
            return CString::new(error_status.to_string()).unwrap().into_raw();
        }
    };
    
    // Discover SD cards
    let discovery_result = rt.block_on(async {
        crate::storage::device_identity::discover_soradyne_volumes().await
    });
    
    let status_json = match discovery_result {
        Ok(volumes) => {
            let available_devices = volumes.len();
            let required_threshold = 3;
            let can_read_data = available_devices >= required_threshold;
            let missing_devices = if available_devices < required_threshold {
                required_threshold - available_devices
            } else {
                0
            };
            
            let device_paths: Vec<String> = volumes
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            
            serde_json::json!({
                "available_devices": available_devices,
                "required_threshold": required_threshold,
                "can_read_data": can_read_data,
                "missing_devices": missing_devices,
                "device_paths": device_paths,
            })
        }
        Err(e) => {
            println!("SD card discovery failed: {}", e);
            serde_json::json!({
                "available_devices": 0,
                "required_threshold": 3,
                "can_read_data": false,
                "missing_devices": 3,
                "device_paths": [],
                "error": format!("Discovery failed: {}", e)
            })
        }
    };
    
    let status_str = status_json.to_string();
    CString::new(status_str).unwrap().into_raw()
}

// FFI function to refresh storage
#[no_mangle]
pub extern "C" fn soradyne_refresh_storage() -> i32 {
    // Create a runtime to handle the async discovery
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("Failed to create runtime for storage refresh: {}", e);
            return -1;
        }
    };
    
    // Discover SD cards
    let discovery_result = rt.block_on(async {
        crate::storage::device_identity::discover_soradyne_volumes().await
    });
    
    match discovery_result {
        Ok(volumes) => {
            let available_devices = volumes.len();
            let required_threshold = 3;
            let can_read_data = available_devices >= required_threshold;
            
            println!("Storage refreshed: {} devices found (need {} for operation)", 
                     available_devices, required_threshold);
            
            if can_read_data {
                1 // Ready
            } else {
                0 // Not ready
            }
        }
        Err(e) => {
            println!("Storage refresh failed: {}", e);
            -1 // Error
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
