//! Web Album Server
//! 
//! A locally-hosted web server that provides a photo album interface
//! using Soradyne's block storage and CRDT album system.

use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::RwLock;
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use soradyne::album::album::*;
use soradyne::album::operations::*;
use soradyne::album::crdt::*;
use soradyne::storage::block_manager::BlockManager;

#[derive(Debug, Serialize, Deserialize)]
struct CreateAlbumRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AlbumResponse {
    id: String,
    name: String,
    item_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaItemResponse {
    id: String,
    filename: String,
    media_type: String,
    size: usize,
    comments: Vec<CommentResponse>,
    rotation: f32,
    has_crop: bool,
    markup_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommentResponse {
    author: String,
    text: String,
    timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddCommentRequest {
    text: String,
    author: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RotateRequest {
    degrees: f32,
    author: String,
}

pub struct WebAlbumServer {
    albums: Arc<RwLock<HashMap<String, MediaAlbum>>>, // album_id -> album
    block_manager: Arc<BlockManager>,
    data_dir: PathBuf,
    albums_index_block_id: Option<[u8; 32]>, // Block ID containing the album index
}

impl WebAlbumServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Use a persistent data directory in the user's home directory
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
        
        let server = Self {
            albums: Arc::new(RwLock::new(HashMap::new())),
            block_manager,
            data_dir,
            albums_index_block_id: None,
        };
        
        Ok(server)
    }
    
    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Load existing albums from block storage
        self.load_albums_from_blocks().await?;
        Ok(())
    }
    
    async fn load_albums_from_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Try to load the albums index from a known location
        let index_file = self.data_dir.join("albums_index.txt");
        
        if index_file.exists() {
            let index_content = std::fs::read_to_string(&index_file)?;
            if let Ok(block_id_bytes) = hex::decode(index_content.trim()) {
                if block_id_bytes.len() == 32 {
                    let mut block_id = [0u8; 32];
                    block_id.copy_from_slice(&block_id_bytes);
                    self.albums_index_block_id = Some(block_id);
                    
                    // Load the albums index
                    if let Ok(index_data) = self.block_manager.read_block(&block_id).await {
                        if let Ok(index_json) = String::from_utf8(index_data) {
                            if let Ok(album_index) = serde_json::from_str::<HashMap<String, [u8; 32]>>(&index_json) {
                                // Load each album from its block
                                let mut albums = HashMap::new();
                                for (album_id, album_block_id) in album_index {
                                    if let Ok(album_data) = self.block_manager.read_block(&album_block_id).await {
                                        if let Ok(album_json) = String::from_utf8(album_data) {
                                            if let Ok(mut album) = serde_json::from_str::<MediaAlbum>(&album_json) {
                                                // Restore the block manager reference
                                                album.block_manager = Some(Arc::clone(&self.block_manager));
                                                albums.insert(album_id, album);
                                            }
                                        }
                                    }
                                }
                                
                                *self.albums.write().await = albums;
                                println!("Loaded {} albums from block storage", self.albums.read().await.len());
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn save_albums_to_blocks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let albums = self.albums.read().await;
        
        // Create an index mapping album IDs to their block IDs
        let mut album_index = HashMap::new();
        
        // Save each album to its own block
        for (album_id, album) in albums.iter() {
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
        
        // Store the index block ID in a simple file for bootstrapping
        let index_file = self.data_dir.join("albums_index.txt");
        std::fs::write(&index_file, hex::encode(index_block_id))?;
        
        self.albums_index_block_id = Some(index_block_id);
        
        Ok(())
    }
    
    pub async fn start_server(self: Arc<Self>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::clone(&self);
        
        // Enable better logging
        env_logger::init();
        
        // CORS headers for local development - make it more permissive
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "authorization", "accept"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);
        
        // Static files route
        let static_files = warp::path("static")
            .and(warp::fs::dir("web_static"));
        
        // API routes
        let api = warp::path("api");
        
        // Get all albums
        let get_albums = api
            .and(warp::path("albums"))
            .and(warp::path::end())
            .and(warp::get())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_get_albums);
        
        // Create album
        let create_album = api
            .and(warp::path("albums"))
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_create_album);
        
        // Get album details
        let get_album = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path::end())
            .and(warp::get())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_get_album);
        
        // Upload media to album
        let upload_media = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::bytes())
            .and(warp::header::<String>("content-type"))
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_upload_media);
        
        // Get media thumbnail
        let get_thumbnail = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::param::<String>())
            .and(warp::path("thumbnail"))
            .and(warp::path::end())
            .and(warp::get())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_get_thumbnail);
        
        // Add comment to media
        let add_comment = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::param::<String>())
            .and(warp::path("comments"))
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_add_comment);
        
        // Rotate media
        let rotate_media = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::param::<String>())
            .and(warp::path("rotate"))
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_rotate_media);
        
        // Root route serves the main HTML page
        let root = warp::path::end()
            .and(warp::get())
            .map(|| warp::reply::html(include_str!("../web_static/index.html")));
        
        // Add logging filter to see all requests
        let log = warp::log("api");
        
        let routes = root
            .or(static_files)
            .or(get_albums)
            .or(create_album)
            .or(get_album)
            .or(upload_media)
            .or(get_thumbnail)
            .or(add_comment)
            .or(rotate_media)
            .recover(handle_rejection)
            .with(cors)
            .with(log);
        
        println!("🌐 Starting web album server on http://localhost:{}", port);
        println!("📁 Album interface available at http://localhost:{}", port);
        
        warp::serve(routes)
            .run(([127, 0, 0, 1], port))
            .await;
        
        Ok(())
    }
}

fn with_server(server: Arc<WebAlbumServer>) -> impl Filter<Extract = (Arc<WebAlbumServer>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || Arc::clone(&server))
}

async fn handle_get_albums(server: Arc<WebAlbumServer>) -> Result<impl Reply, warp::Rejection> {
    let albums = server.albums.read().await;
    let album_list: Vec<AlbumResponse> = albums.iter().map(|(id, album)| {
        AlbumResponse {
            id: id.clone(),
            name: album.metadata.title.clone(),
            item_count: album.items.len(),
        }
    }).collect();
    
    Ok(warp::reply::json(&album_list))
}

async fn handle_create_album(req: CreateAlbumRequest, server: Arc<WebAlbumServer>) -> Result<impl Reply, warp::Rejection> {
    let album_id = Uuid::new_v4().to_string();
    let album_id_clone = album_id.clone();
    
    let album = MediaAlbum {
        album_id: album_id.clone(),
        items: HashMap::new(),
        metadata: AlbumMetadata {
            title: req.name.clone(),
            created_by: "web_user".to_string(),
            created_at: chrono::Utc::now().timestamp() as u64,
            shared_with: HashMap::new(),
        },
        block_manager: Some(Arc::clone(&server.block_manager)),
    };
    
    let mut albums = server.albums.write().await;
    albums.insert(album_id.clone(), album);
    drop(albums); // Release the lock before saving
    
    // Save albums to block storage
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = save_album_update(server_clone, album_id_clone).await {
            eprintln!("Failed to save albums to blocks: {}", e);
        }
    });
    
    let response = AlbumResponse {
        id: album_id,
        name: req.name,
        item_count: 0,
    };
    Ok(warp::reply::json(&response))
}

async fn handle_get_album(album_id: String, server: Arc<WebAlbumServer>) -> Result<impl Reply, warp::Rejection> {
    let albums = server.albums.read().await;
    
    if let Some(album) = albums.get(&album_id) {
        let media_items: Vec<MediaItemResponse> = album.items.iter().map(|(media_id, crdt)| {
            let state = crdt.reduce();
            
            MediaItemResponse {
                id: media_id.clone(),
                filename: format!("media_{}", media_id), // Placeholder filename
                media_type: "image/jpeg".to_string(), // Placeholder type
                size: 0, // Placeholder size
                comments: Vec::new(), // TODO: Extract comments from state
                rotation: state.rotation,
                has_crop: state.crop.is_some(),
                markup_count: state.markup.len(),
            }
        }).collect();
        
        Ok(warp::reply::json(&media_items))
    } else {
        Ok(warp::reply::json(&serde_json::json!({"error": "Album not found"})))
    }
}

async fn handle_upload_media(
    album_id: String,
    body: bytes::Bytes,
    content_type: String,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    
    println!("Received upload request for album: {}", album_id);
    println!("Content-Type: {}", content_type);
    println!("Body size: {} bytes", body.len());
    
    // Parse the boundary from content-type
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .ok_or_else(|| {
            eprintln!("No boundary found in content-type");
            warp::reject::reject()
        })?;
    
    println!("Boundary: {}", boundary);
    
    // Use multer to parse the multipart data
    let mut multipart = multer::Multipart::new(futures_util::stream::once(async { Ok::<_, std::io::Error>(body) }), boundary);
    
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        eprintln!("Multipart parsing error: {}", e);
        warp::reject::reject()
    })? {
        let name = field.name().unwrap_or("").to_string();
        println!("Processing field: {}", name);
        
        if name == "file" {
            let filename = field.file_name().unwrap_or("unknown").to_string();
            let data = field.bytes().await.map_err(|e| {
                eprintln!("Failed to read field data: {}", e);
                warp::reject::reject()
            })?;
            
            let media_id = Uuid::new_v4().to_string();
            
            println!("Uploading file: {} ({} bytes)", filename, data.len());
            
            // Store the media data in block storage
            match server.block_manager.write_direct_block(&data).await {
                Ok(block_id) => {
                    // Create an operation to add media
                    let op = EditOp {
                        op_id: Uuid::new_v4(),
                        timestamp: chrono::Utc::now().timestamp() as u64,
                        author: "web_user".to_string(),
                        op_type: "add_media".to_string(),
                        payload: serde_json::json!({
                            "filename": filename,
                            "block_id": hex::encode(block_id),
                            "size": data.len()
                        }),
                    };
                    
                    // Add to album
                    let mut albums = server.albums.write().await;
                    if let Some(album) = albums.get_mut(&album_id) {
                        let crdt = album.get_or_create(&media_id);
                        match crdt.apply_local(op) {
                            Ok(_) => {
                                println!("Successfully added media {} to album {}", media_id, album_id);
                                
                                // Save albums to block storage
                                drop(albums); // Release the lock before saving
                                let server_clone = Arc::clone(&server);
                                tokio::spawn(async move {
                                    // We can't easily get a mutable reference here, so we'll implement
                                    // a different approach for saving individual album updates
                                    if let Err(e) = save_album_update(server_clone, album_id.clone()).await {
                                        eprintln!("Failed to save album update: {}", e);
                                    }
                                });
                                
                                return Ok(warp::reply::json(&serde_json::json!({
                                    "success": true,
                                    "media_id": media_id
                                })));
                            }
                            Err(e) => {
                                eprintln!("Failed to apply operation: {}", e);
                                return Ok(warp::reply::json(&serde_json::json!({
                                    "error": "Failed to apply operation"
                                })));
                            }
                        }
                    } else {
                        eprintln!("Album {} not found", album_id);
                        return Ok(warp::reply::json(&serde_json::json!({
                            "error": "Album not found"
                        })));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to store media data: {}", e);
                    return Ok(warp::reply::json(&serde_json::json!({
                        "error": "Failed to store media"
                    })));
                }
            }
        }
    }
    
    Ok(warp::reply::json(&serde_json::json!({
        "error": "No file found in upload"
    })))
}

async fn handle_get_thumbnail(
    album_id: String,
    media_id: String,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    let albums = server.albums.read().await;
    
    if let Some(album) = albums.get(&album_id) {
        if let Some(crdt) = album.items.get(&media_id) {
            // Try to find the block_id from the operations
            for op in crdt.ops() {
                if op.op_type == "add_media" {
                    if let Some(block_id_hex) = op.payload.get("block_id").and_then(|v| v.as_str()) {
                        if let Ok(block_id_bytes) = hex::decode(block_id_hex) {
                            if block_id_bytes.len() == 32 {
                                let mut block_id = [0u8; 32];
                                block_id.copy_from_slice(&block_id_bytes);
                                
                                // Try to read the original image data
                                if let Ok(image_data) = server.block_manager.read_block(&block_id).await {
                                    // Generate thumbnail from the actual image
                                    if let Ok(thumbnail) = generate_thumbnail(&image_data) {
                                        return Ok(warp::reply::with_header(
                                            thumbnail,
                                            "content-type",
                                            "image/jpeg"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(warp::reply::with_header(
        create_placeholder_thumbnail(),
        "content-type",
        "image/png"
    ))
}

async fn handle_add_comment(
    album_id: String,
    media_id: String,
    req: AddCommentRequest,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    let comment_op = EditOp {
        op_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        author: req.author,
        op_type: "add_comment".to_string(),
        payload: serde_json::json!({
            "text": req.text
        }),
    };
    
    let mut albums = server.albums.write().await;
    if let Some(album) = albums.get_mut(&album_id) {
        let crdt = album.get_or_create(&media_id);
        match crdt.apply_local(comment_op) {
            Ok(_) => Ok(warp::reply::json(&serde_json::json!({"success": true}))),
            Err(e) => {
                eprintln!("Failed to add comment: {}", e);
                Ok(warp::reply::json(&serde_json::json!({"error": "Failed to add comment"})))
            }
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({"error": "Album not found"})))
    }
}

async fn handle_rotate_media(
    album_id: String,
    media_id: String,
    req: RotateRequest,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    let rotate_op = EditOp {
        op_id: Uuid::new_v4(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        author: req.author,
        op_type: "rotate".to_string(),
        payload: serde_json::json!({
            "degrees": req.degrees
        }),
    };
    
    let mut albums = server.albums.write().await;
    if let Some(album) = albums.get_mut(&album_id) {
        let crdt = album.get_or_create(&media_id);
        match crdt.apply_local(rotate_op) {
            Ok(_) => Ok(warp::reply::json(&serde_json::json!({"success": true}))),
            Err(e) => {
                eprintln!("Failed to rotate media: {}", e);
                Ok(warp::reply::json(&serde_json::json!({"error": "Failed to rotate media"})))
            }
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({"error": "Album not found"})))
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    eprintln!("Request rejection: {:?}", err);
    
    let code;
    let message;
    
    if err.is_not_found() {
        code = warp::http::StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
    } else if let Some(_) = err.find::<warp::filters::body::BodyDeserializeError>() {
        code = warp::http::StatusCode::BAD_REQUEST;
        message = "BAD_REQUEST";
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        code = warp::http::StatusCode::METHOD_NOT_ALLOWED;
        message = "METHOD_NOT_ALLOWED";
    } else {
        code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        message = "INTERNAL_SERVER_ERROR";
    }
    
    let json = warp::reply::json(&serde_json::json!({
        "error": message,
        "code": code.as_u16()
    }));
    
    Ok(warp::reply::with_status(json, code))
}

fn generate_thumbnail(media_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to detect if this is a video file by checking the first few bytes
    if is_video_file(media_data) {
        generate_video_thumbnail(media_data)
    } else {
        generate_image_thumbnail(media_data)
    }
}

fn is_video_file(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    
    // Check for common video file signatures
    // MP4/MOV files start with specific patterns
    if data.len() >= 8 {
        // Check for MP4 ftyp box
        if &data[4..8] == b"ftyp" {
            return true;
        }
    }
    
    // Check for WebM signature
    if data.len() >= 4 && &data[0..4] == b"\x1A\x45\xDF\xA3" {
        return true;
    }
    
    // Check for AVI signature
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"AVI " {
        return true;
    }
    
    false
}

fn generate_image_thumbnail(image_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Load the image from the data
    let img = image::load_from_memory(image_data)?;
    
    // Resize to thumbnail size (150x150) while maintaining aspect ratio
    let thumbnail = img.thumbnail(150, 150);
    
    // Encode as JPEG
    let mut buffer = Vec::new();
    thumbnail.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Jpeg(80))?;
    
    Ok(buffer)
}

fn generate_video_thumbnail(_video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // For now, create a video-specific placeholder thumbnail
    // In a full implementation, you'd use FFmpeg to extract a frame
    create_video_placeholder_thumbnail()
}

fn create_video_placeholder_thumbnail() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(150, 150);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let center_x = 75;
        let center_y = 75;
        let distance = ((x as i32 - center_x).pow(2) + (y as i32 - center_y).pow(2)) as f32;
        
        // Create a play button icon
        if distance < 60.0 * 60.0 {
            // Background circle
            *pixel = Rgb([50, 50, 50]); // Dark background
            
            // Play triangle
            let triangle_left = 50;
            let triangle_right = 100;
            let triangle_top = 55;
            let triangle_bottom = 95;
            
            if x >= triangle_left && x <= triangle_right && y >= triangle_top && y <= triangle_bottom {
                // Simple triangle approximation
                let relative_y = y as i32 - triangle_top as i32;
                let triangle_height = triangle_bottom - triangle_top;
                let triangle_width = triangle_right - triangle_left;
                let expected_x = triangle_left + (relative_y * triangle_width as i32 / triangle_height as i32) as u32;
                
                if x >= triangle_left && x <= expected_x {
                    *pixel = Rgb([255, 255, 255]); // White play button
                }
            }
        } else {
            *pixel = Rgb([240, 240, 240]); // Light gray background
        }
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    Ok(buffer)
}

async fn save_album_update(server: Arc<WebAlbumServer>, _album_id: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // This is a simplified version that saves all albums
    // In a production system, you'd want to save only the changed album
    let albums = server.albums.read().await;
    
    // Create an index mapping album IDs to their block IDs
    let mut album_index = HashMap::new();
    
    // Save each album to its own block
    for (id, album) in albums.iter() {
        // Create a serializable version without the block manager
        let mut serializable_album = album.clone();
        serializable_album.block_manager = None;
        
        let album_json = serde_json::to_string_pretty(&serializable_album)?;
        let album_data = album_json.as_bytes();
        
        // Store the album in a block
        let album_block_id = server.block_manager.write_direct_block(album_data).await?;
        album_index.insert(id.clone(), album_block_id);
    }
    
    // Save the index to a block
    let index_json = serde_json::to_string_pretty(&album_index)?;
    let index_data = index_json.as_bytes();
    let index_block_id = server.block_manager.write_direct_block(index_data).await?;
    
    // Store the index block ID in a simple file for bootstrapping
    let index_file = server.data_dir.join("albums_index.txt");
    std::fs::write(&index_file, hex::encode(index_block_id))?;
    
    Ok(())
}

fn create_placeholder_thumbnail() -> Vec<u8> {
    // Create a simple 150x150 placeholder image with a camera icon pattern
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(150, 150);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // Create a simple camera icon pattern
        let center_x = 75;
        let center_y = 75;
        let distance = ((x as i32 - center_x).pow(2) + (y as i32 - center_y).pow(2)) as f32;
        
        if distance < 30.0 * 30.0 {
            *pixel = Rgb([100, 100, 100]); // Dark gray circle
        } else if distance < 50.0 * 50.0 {
            *pixel = Rgb([200, 200, 200]); // Light gray ring
        } else {
            *pixel = Rgb([240, 240, 240]); // Very light gray background
        }
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png).unwrap();
    buffer
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = WebAlbumServer::new()?;
    server.initialize().await?;
    let server = Arc::new(server);
    server.start_server(3030).await?;
    Ok(())
}
