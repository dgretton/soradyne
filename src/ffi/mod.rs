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

// Global state for the album system
static mut ALBUM_SYSTEM: Option<Arc<Mutex<AlbumSystem>>> = None;

pub struct AlbumSystem {
    albums: HashMap<String, MediaAlbum>,
    block_manager: Arc<BlockManager>,
    runtime: Arc<Runtime>,
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
        
        Ok(Self {
            albums: HashMap::new(),
            block_manager,
            runtime,
        })
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
                        let state = crdt.reduce();
                        serde_json::json!({
                            "id": media_id,
                            "filename": format!("media_{}", media_id),
                            "media_type": "image/jpeg",
                            "size": 0,
                            "rotation": state.rotation,
                            "has_crop": state.crop.is_some(),
                            "markup_count": state.markup.len(),
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
                    let media_id = uuid::Uuid::new_v4().to_string();
                    let filename = PathBuf::from(&file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    
                    // Store in block storage using the runtime
                    let block_manager = Arc::clone(&system.block_manager);
                    let runtime = Arc::clone(&system.runtime);
                    
                    if let Ok(block_id) = runtime.block_on(async {
                        block_manager.write_direct_block(&file_data).await
                    }) {
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
                                "media_type": "image"
                            }),
                        };
                        
                        // Add to album
                        if let Some(album) = system.albums.get_mut(&album_id) {
                            let crdt = album.get_or_create(&media_id);
                            if crdt.apply_local(op).is_ok() {
                                return 0; // Success
                            }
                        }
                    }
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

// FFI function to get media data (returns raw bytes)
#[no_mangle]
pub extern "C" fn soradyne_get_media_data(album_id_ptr: *const c_char, media_id_ptr: *const c_char, data_ptr: *mut *mut u8, size_ptr: *mut usize) -> i32 {
    unsafe {
        if let Some(system) = &ALBUM_SYSTEM {
            if let Ok(system) = system.lock() {
                let album_id = CStr::from_ptr(album_id_ptr).to_string_lossy().to_string();
                let media_id = CStr::from_ptr(media_id_ptr).to_string_lossy().to_string();
                
                if let Some(album) = system.albums.get(&album_id) {
                    if let Some(crdt) = album.items.get(&media_id) {
                        let state = crdt.reduce();
                        
                        // Get the block_id from the first operation's payload
                        if let Some(op) = crdt.ops().first() {
                            if let Some(block_id_hex) = op.payload.get("block_id").and_then(|v| v.as_str()) {
                                if let Ok(block_id_bytes) = hex::decode(block_id_hex) {
                                    if block_id_bytes.len() == 32 {
                                        let mut block_id = [0u8; 32];
                                        block_id.copy_from_slice(&block_id_bytes);
                                        
                                        let block_manager = Arc::clone(&system.block_manager);
                                        let runtime = Arc::clone(&system.runtime);
                                        
                                        if let Ok(data) = runtime.block_on(async {
                                            block_manager.read_block(&block_id).await
                                        }) {
                                            // Allocate memory for the data
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
