//! Web Album Server
//! 
//! A locally-hosted web server that provides a photo album interface
//! using Soradyne's block storage and CRDT album system.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tempfile::TempDir;

use soradyne::album::*;
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
    sync_manager: Arc<RwLock<AlbumSyncManager>>,
    albums: Arc<RwLock<HashMap<String, String>>>, // album_id -> album_name
    _temp_dir: TempDir, // Keep alive for the duration of the server
}

impl WebAlbumServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().to_path_buf();
        
        // Create rimsd directories
        let mut rimsd_dirs = Vec::new();
        for i in 0..4 {
            let device_dir = test_dir.join(format!("rimsd_{}", i));
            let rimsd_dir = device_dir.join(".rimsd");
            std::fs::create_dir_all(&rimsd_dir)?;
            rimsd_dirs.push(rimsd_dir);
        }
        
        let metadata_path = test_dir.join("metadata.json");
        let block_manager = Arc::new(BlockManager::new(
            rimsd_dirs,
            metadata_path,
            3, // threshold
            4, // total_shards
        )?);
        
        let sync_manager = AlbumSyncManager::new(block_manager, "web_server".to_string());
        
        Ok(Self {
            sync_manager: Arc::new(RwLock::new(sync_manager)),
            albums: Arc::new(RwLock::new(HashMap::new())),
            _temp_dir: temp_dir,
        })
    }
    
    pub async fn start_server(self: Arc<Self>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::clone(&self);
        
        // CORS headers for local development
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]);
        
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
            .and(warp::multipart::form().max_length(50 * 1024 * 1024)) // 50MB max
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
        
        let routes = root
            .or(static_files)
            .or(get_albums)
            .or(create_album)
            .or(get_album)
            .or(upload_media)
            .or(get_thumbnail)
            .or(add_comment)
            .or(rotate_media)
            .with(cors);
        
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
    let album_list: Vec<AlbumResponse> = albums.iter().map(|(id, name)| {
        AlbumResponse {
            id: id.clone(),
            name: name.clone(),
            item_count: 0, // TODO: Get actual count
        }
    }).collect();
    
    Ok(warp::reply::json(&album_list))
}

async fn handle_create_album(req: CreateAlbumRequest, server: Arc<WebAlbumServer>) -> Result<impl Reply, warp::Rejection> {
    let mut sync_manager = server.sync_manager.write().await;
    
    match sync_manager.create_album(req.name.clone()) {
        Ok(album_id) => {
            let mut albums = server.albums.write().await;
            albums.insert(album_id.clone(), req.name.clone());
            
            let response = AlbumResponse {
                id: album_id,
                name: req.name,
                item_count: 0,
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            eprintln!("Failed to create album: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": "Failed to create album"})))
        }
    }
}

async fn handle_get_album(album_id: String, server: Arc<WebAlbumServer>) -> Result<impl Reply, warp::Rejection> {
    let sync_manager = server.sync_manager.read().await;
    
    if let Some(album) = sync_manager.get_album(&album_id) {
        let states = album.reduce_all();
        let media_items: Vec<MediaItemResponse> = states.iter().map(|(media_id, state)| {
            let comments: Vec<CommentResponse> = state.comments.iter().map(|comment| {
                CommentResponse {
                    author: comment.author.clone(),
                    text: comment.text.clone(),
                    timestamp: comment.timestamp,
                }
            }).collect();
            
            MediaItemResponse {
                id: media_id.clone(),
                filename: state.media.as_ref().map(|m| m.filename.clone()).unwrap_or_default(),
                media_type: state.media.as_ref().map(|m| m.mime_type.clone()).unwrap_or_default(),
                size: state.media.as_ref().map(|m| m.size).unwrap_or(0),
                comments,
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
    form: warp::multipart::FormData,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    use futures_util::TryStreamExt;
    use bytes::BufMut;
    
    let mut sync_manager = server.sync_manager.write().await;
    
    let parts: Vec<_> = form.try_collect().await.map_err(|_| warp::reject::reject())?;
    
    for part in parts {
        if part.name() == "file" {
            let filename = part.filename().unwrap_or("unknown").to_string();
            let data: Vec<u8> = part.stream()
                .try_fold(Vec::new(), |mut vec, data| {
                    vec.put(data);
                    async move { Ok(vec) }
                })
                .await
                .map_err(|_| warp::reject::reject())?;
            
            // Determine media type from filename
            let media_type = if filename.to_lowercase().ends_with(".jpg") || filename.to_lowercase().ends_with(".jpeg") {
                MediaType::Photo
            } else if filename.to_lowercase().ends_with(".png") {
                MediaType::Photo
            } else if filename.to_lowercase().ends_with(".mp4") || filename.to_lowercase().ends_with(".mov") {
                MediaType::Video
            } else {
                MediaType::Photo // Default
            };
            
            let media_id = Uuid::new_v4().to_string();
            
            match sync_manager.add_media_to_album(&album_id, media_id.clone(), &data, media_type, filename).await {
                Ok(_) => {
                    return Ok(warp::reply::json(&serde_json::json!({
                        "success": true,
                        "media_id": media_id
                    })));
                }
                Err(e) => {
                    eprintln!("Failed to add media: {}", e);
                    return Ok(warp::reply::json(&serde_json::json!({"error": "Failed to upload media"})));
                }
            }
        }
    }
    
    Ok(warp::reply::json(&serde_json::json!({"error": "No file found"})))
}

async fn handle_get_thumbnail(
    album_id: String,
    media_id: String,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    let sync_manager = server.sync_manager.read().await;
    
    if let Some(album) = sync_manager.get_album(&album_id) {
        if let Some(crdt) = album.items.get(&media_id) {
            let state = crdt.reduce();
            if let Some(media) = &state.media {
                // TODO: Generate thumbnail using renderer
                // For now, return a placeholder
                let placeholder = create_placeholder_thumbnail();
                return Ok(warp::reply::with_header(
                    placeholder,
                    "content-type",
                    "image/png"
                ));
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
    let mut sync_manager = server.sync_manager.write().await;
    
    let comment_op = EditOp::add_comment(req.author, req.text, None);
    
    match sync_manager.apply_operation(&album_id, &media_id, comment_op) {
        Ok(_) => Ok(warp::reply::json(&serde_json::json!({"success": true}))),
        Err(e) => {
            eprintln!("Failed to add comment: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": "Failed to add comment"})))
        }
    }
}

async fn handle_rotate_media(
    album_id: String,
    media_id: String,
    req: RotateRequest,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    let mut sync_manager = server.sync_manager.write().await;
    
    let rotate_op = EditOp::rotate(req.author, req.degrees);
    
    match sync_manager.apply_operation(&album_id, &media_id, rotate_op) {
        Ok(_) => Ok(warp::reply::json(&serde_json::json!({"success": true}))),
        Err(e) => {
            eprintln!("Failed to rotate media: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": "Failed to rotate media"})))
        }
    }
}

fn create_placeholder_thumbnail() -> Vec<u8> {
    // Create a simple 150x150 placeholder image
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(150, 150);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let gray = ((x + y) % 50) as u8 * 5;
        *pixel = Rgb([gray, gray, gray]);
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png).unwrap();
    buffer
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Arc::new(WebAlbumServer::new()?);
    server.start_server(3030).await?;
    Ok(())
}
