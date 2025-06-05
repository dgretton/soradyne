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
use std::process::Command;
use std::io::Write;

use soradyne::album::album::*;
use soradyne::album::operations::*;
use soradyne::album::crdt::*;
use soradyne::storage::block_manager::BlockManager;
use soradyne::storage::block_file::BlockFile;

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
        
        // Get media medium resolution
        let get_medium = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::param::<String>())
            .and(warp::path("medium"))
            .and(warp::path::end())
            .and(warp::get())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_get_medium);
        
        // Get media high resolution
        let get_high = api
            .and(warp::path("albums"))
            .and(warp::path::param::<String>())
            .and(warp::path("media"))
            .and(warp::path::param::<String>())
            .and(warp::path("high"))
            .and(warp::path::end())
            .and(warp::get())
            .and(with_server(Arc::clone(&server)))
            .and_then(handle_get_high);
        
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
        
        // Root route serves a simple HTML page
        let root = warp::path::end()
            .and(warp::get())
            .map(|| warp::reply::html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Soradyne Web Album</title>
    <meta charset="utf-8">
    <style>
        * {
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            color: #333;
        }
        
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: rgba(255, 255, 255, 0.95);
            border-radius: 20px;
            padding: 30px;
            box-shadow: 0 20px 40px rgba(0, 0, 0, 0.1);
            backdrop-filter: blur(10px);
        }
        
        h1 {
            text-align: center;
            color: #4a5568;
            margin-bottom: 30px;
            font-size: 2.5em;
            font-weight: 300;
        }
        
        h2 {
            color: #2d3748;
            border-bottom: 2px solid #e2e8f0;
            padding-bottom: 10px;
            margin-bottom: 20px;
        }
        
        h3 {
            color: #4a5568;
            margin-bottom: 15px;
        }
        
        .album {
            background: white;
            border: 1px solid #e2e8f0;
            border-radius: 12px;
            margin: 15px 0;
            padding: 25px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.05);
            transition: all 0.3s ease;
        }
        
        .album:hover {
            transform: translateY(-2px);
            box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
        }
        
        .media-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 20px;
            margin-top: 20px;
        }
        
        .media-item {
            background: white;
            border-radius: 12px;
            padding: 15px;
            text-align: center;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.05);
            transition: all 0.3s ease;
        }
        
        .media-item:hover {
            transform: translateY(-2px);
            box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
        }
        
        .thumbnail {
            width: 150px;
            height: 150px;
            object-fit: cover;
            border-radius: 8px;
            margin-bottom: 10px;
            border: 2px solid #e2e8f0;
            cursor: pointer;
            transition: all 0.3s ease;
        }
        
        .modal-image {
            max-width: 90vw;
            max-height: 90vh;
            width: 90vw;
            height: auto;
            object-fit: contain;
            border-radius: 8px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
        }
        
        .thumbnail:hover {
            transform: scale(1.05);
            box-shadow: 0 8px 20px rgba(0, 0, 0, 0.2);
        }
        
        .media-filename {
            font-size: 0.9em;
            color: #4a5568;
            margin-bottom: 10px;
            word-break: break-word;
        }
        
        button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 14px;
            font-weight: 500;
            transition: all 0.3s ease;
            margin: 5px;
        }
        
        button:hover {
            transform: translateY(-1px);
            box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
        }
        
        button:active {
            transform: translateY(0);
        }
        
        .btn-secondary {
            background: linear-gradient(135deg, #718096 0%, #4a5568 100%);
        }
        
        .btn-secondary:hover {
            box-shadow: 0 4px 12px rgba(113, 128, 150, 0.4);
        }
        
        .upload-area {
            border: 3px dashed #cbd5e0;
            border-radius: 12px;
            padding: 40px;
            text-align: center;
            margin: 20px 0;
            background: #f7fafc;
            transition: all 0.3s ease;
            cursor: pointer;
        }
        
        .upload-area.drag-over {
            border-color: #667eea;
            background: #edf2f7;
            transform: scale(1.02);
        }
        
        .upload-area:hover {
            border-color: #a0aec0;
            background: #edf2f7;
        }
        
        .upload-text {
            color: #4a5568;
            font-size: 1.1em;
            margin-bottom: 15px;
        }
        
        .upload-subtext {
            color: #718096;
            font-size: 0.9em;
        }
        
        input[type="file"] {
            display: none;
        }
        
        .controls {
            display: flex;
            gap: 10px;
            align-items: center;
            margin-bottom: 20px;
            flex-wrap: wrap;
        }
        
        .back-button {
            background: linear-gradient(135deg, #718096 0%, #4a5568 100%);
        }
        
        .loading {
            display: inline-block;
            width: 20px;
            height: 20px;
            border: 3px solid #f3f3f3;
            border-top: 3px solid #667eea;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-left: 10px;
        }
        
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
        
        .notification {
            position: fixed;
            top: 20px;
            right: 20px;
            background: #48bb78;
            color: white;
            padding: 15px 20px;
            border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
            transform: translateX(400px);
            transition: transform 0.3s ease;
            z-index: 1000;
        }
        
        .notification.show {
            transform: translateX(0);
        }
        
        .notification.error {
            background: #f56565;
        }
        
        /* Modal/Lightbox styles */
        .modal {
            display: none;
            position: fixed;
            z-index: 2000;
            left: 0;
            top: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0, 0, 0, 0.9);
            backdrop-filter: blur(5px);
        }
        
        .modal.show {
            display: flex;
            align-items: center;
            justify-content: center;
            animation: fadeIn 0.3s ease;
        }
        
        .modal-content {
            position: relative;
            max-width: 90vw;
            max-height: 90vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        
        .modal-image {
            max-width: 100%;
            max-height: 100%;
            object-fit: contain;
            border-radius: 8px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
            transition: opacity 0.5s ease;
        }
        
        .modal-image.loading {
            opacity: 0.7;
            filter: blur(2px);
        }
        
        .modal-close {
            position: absolute;
            top: 20px;
            right: 30px;
            color: white;
            font-size: 40px;
            font-weight: bold;
            cursor: pointer;
            z-index: 2001;
            background: rgba(0, 0, 0, 0.5);
            border-radius: 50%;
            width: 60px;
            height: 60px;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.3s ease;
        }
        
        .modal-close:hover {
            background: rgba(0, 0, 0, 0.8);
            transform: scale(1.1);
        }
        
        .modal-loading {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: white;
            font-size: 18px;
            z-index: 2001;
        }
        
        .resolution-indicator {
            position: absolute;
            bottom: 20px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(0, 0, 0, 0.7);
            color: white;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 14px;
            z-index: 2001;
        }
        
        @keyframes fadeIn {
            from { opacity: 0; }
            to { opacity: 1; }
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🎨 Soradyne Web Album</h1>
        <div id="app">
            <h2>Albums</h2>
            <div id="albums"></div>
            <button onclick="createAlbum()">✨ Create New Album</button>
        </div>
    </div>
    
    <div id="notification" class="notification"></div>
    
    <!-- Modal for full-size image viewing -->
    <div id="imageModal" class="modal">
        <span class="modal-close" onclick="closeModal()">&times;</span>
        <div class="modal-content">
            <img id="modalImage" class="modal-image" src="" alt="Full size image" />
            <div id="modalLoading" class="modal-loading" style="display: none;">Loading higher resolution...</div>
            <div id="resolutionIndicator" class="resolution-indicator">Thumbnail</div>
        </div>
    </div>
    
    <script>
        let currentAlbumId = null;
        
        function showNotification(message, isError = false) {
            const notification = document.getElementById('notification');
            notification.textContent = message;
            notification.className = `notification ${isError ? 'error' : ''} show`;
            setTimeout(() => {
                notification.className = 'notification';
            }, 3000);
        }
        
        async function loadAlbums() {
            try {
                const response = await fetch('/api/albums');
                const albums = await response.json();
                const container = document.getElementById('albums');
                container.innerHTML = albums.map(album => `
                    <div class="album">
                        <h3>📁 ${album.name}</h3>
                        <p>Items: ${album.item_count}</p>
                        <button onclick="viewAlbum('${album.id}')">View Album</button>
                    </div>
                `).join('');
                currentAlbumId = null;
            } catch (error) {
                showNotification('Failed to load albums', true);
            }
        }
        
        async function createAlbum() {
            const name = prompt('Album name:');
            if (name) {
                try {
                    await fetch('/api/albums', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ name })
                    });
                    showNotification('Album created successfully!');
                    loadAlbums();
                } catch (error) {
                    showNotification('Failed to create album', true);
                }
            }
        }
        
        async function viewAlbum(albumId) {
            currentAlbumId = albumId;
            try {
                const response = await fetch(`/api/albums/${albumId}`);
                const items = await response.json();
                const container = document.getElementById('albums');
                container.innerHTML = `
                    <div class="controls">
                        <button class="back-button" onclick="loadAlbums()">← Back to Albums</button>
                        <h3>Album Contents</h3>
                    </div>
                    
                    <div class="upload-area" id="uploadArea" onclick="document.getElementById('fileInput').click()">
                        <div class="upload-text">📎 Drop files here or click to upload</div>
                        <div class="upload-subtext">Supports images, videos, and audio files</div>
                        <input type="file" id="fileInput" accept="image/*,video/*,audio/*" multiple />
                    </div>
                    
                    <div class="media-grid">
                        ${items.map(item => `
                            <div class="media-item">
                                <img src="/api/albums/${albumId}/media/${item.id}/thumbnail" 
                                     class="thumbnail" 
                                     onclick="openModal('${albumId}', '${item.id}')" />
                            </div>
                        `).join('')}
                    </div>
                `;
                
                setupDragAndDrop();
                setupFileInput();
            } catch (error) {
                showNotification('Failed to load album', true);
            }
        }
        
        function setupDragAndDrop() {
            const uploadArea = document.getElementById('uploadArea');
            if (!uploadArea) return;
            
            ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
                uploadArea.addEventListener(eventName, preventDefaults, false);
                document.body.addEventListener(eventName, preventDefaults, false);
            });
            
            ['dragenter', 'dragover'].forEach(eventName => {
                uploadArea.addEventListener(eventName, highlight, false);
            });
            
            ['dragleave', 'drop'].forEach(eventName => {
                uploadArea.addEventListener(eventName, unhighlight, false);
            });
            
            uploadArea.addEventListener('drop', handleDrop, false);
            
            function preventDefaults(e) {
                e.preventDefault();
                e.stopPropagation();
            }
            
            function highlight(e) {
                uploadArea.classList.add('drag-over');
            }
            
            function unhighlight(e) {
                uploadArea.classList.remove('drag-over');
            }
            
            function handleDrop(e) {
                const dt = e.dataTransfer;
                const files = dt.files;
                handleFiles(files);
            }
        }
        
        function setupFileInput() {
            const fileInput = document.getElementById('fileInput');
            if (fileInput) {
                fileInput.addEventListener('change', function(e) {
                    handleFiles(e.target.files);
                });
            }
        }
        
        async function handleFiles(files) {
            if (!currentAlbumId) return;
            
            for (let file of files) {
                await uploadSingleFile(file);
            }
            
            // Refresh the album view
            viewAlbum(currentAlbumId);
        }
        
        async function uploadSingleFile(file) {
            try {
                const formData = new FormData();
                formData.append('file', file);
                
                showNotification(`Uploading ${file.name}...`);
                
                const response = await fetch(`/api/albums/${currentAlbumId}/media`, {
                    method: 'POST',
                    body: formData
                });
                
                if (response.ok) {
                    showNotification(`${file.name} uploaded successfully!`);
                } else {
                    throw new Error('Upload failed');
                }
            } catch (error) {
                showNotification(`Failed to upload ${file.name}`, true);
            }
        }
        
        async function rotateMedia(albumId, mediaId) {
            try {
                await fetch(`/api/albums/${albumId}/media/${mediaId}/rotate`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ degrees: 90, author: 'web_user' })
                });
                showNotification('Media rotated!');
                viewAlbum(albumId);
            } catch (error) {
                showNotification('Failed to rotate media', true);
            }
        }
        
        // Modal functionality
        function openModal(albumId, mediaId) {
            const modal = document.getElementById('imageModal');
            const modalImage = document.getElementById('modalImage');
            const resolutionIndicator = document.getElementById('resolutionIndicator');
            const modalLoading = document.getElementById('modalLoading');
            
            // Show modal with thumbnail first
            const thumbnailUrl = `/api/albums/${albumId}/media/${mediaId}/thumbnail`;
            modalImage.src = thumbnailUrl;
            modal.classList.add('show');
            resolutionIndicator.textContent = 'Thumbnail';
            
            // Start loading medium resolution
            setTimeout(() => {
                loadMediumResolution(albumId, mediaId);
            }, 100);
        }
        
        function loadMediumResolution(albumId, mediaId) {
            const modalImage = document.getElementById('modalImage');
            const resolutionIndicator = document.getElementById('resolutionIndicator');
            const modalLoading = document.getElementById('modalLoading');
            
            modalLoading.style.display = 'block';
            modalLoading.textContent = 'Loading medium resolution...';
            resolutionIndicator.textContent = 'Loading medium...';
            
            // Create a new image to preload medium resolution
            const mediumImg = new Image();
            mediumImg.onload = function() {
                modalImage.src = mediumImg.src;
                resolutionIndicator.textContent = 'Medium Resolution';
                modalLoading.style.display = 'none';
                
                // Start loading high resolution after a short delay
                setTimeout(() => {
                    loadHighResolution(albumId, mediaId);
                }, 500);
            };
            mediumImg.onerror = function() {
                modalLoading.style.display = 'none';
                resolutionIndicator.textContent = 'Thumbnail (medium failed)';
                // Still try to load high resolution
                setTimeout(() => {
                    loadHighResolution(albumId, mediaId);
                }, 500);
            };
            
            // Use the medium resolution endpoint
            mediumImg.src = `/api/albums/${albumId}/media/${mediaId}/medium`;
        }
        
        function loadHighResolution(albumId, mediaId) {
            const modalImage = document.getElementById('modalImage');
            const resolutionIndicator = document.getElementById('resolutionIndicator');
            const modalLoading = document.getElementById('modalLoading');
            
            modalLoading.style.display = 'block';
            modalLoading.textContent = 'Loading high resolution...';
            resolutionIndicator.textContent = 'Loading high...';
            
            // Create a new image to preload high resolution
            const highImg = new Image();
            highImg.onload = function() {
                // Smoothly transition without blanking out
                modalImage.src = highImg.src;
                resolutionIndicator.textContent = 'High Resolution';
                modalLoading.style.display = 'none';
            };
            highImg.onerror = function() {
                modalLoading.style.display = 'none';
                resolutionIndicator.textContent = 'Medium Resolution (high failed)';
            };
            
            // Use the high resolution endpoint
            highImg.src = `/api/albums/${albumId}/media/${mediaId}/high`;
        }
        
        function closeModal() {
            const modal = document.getElementById('imageModal');
            const modalImage = document.getElementById('modalImage');
            const modalLoading = document.getElementById('modalLoading');
            
            modal.classList.remove('show');
            modalLoading.style.display = 'none';
            
            // Clear the image source after animation
            setTimeout(() => {
                modalImage.src = '';
                modalImage.classList.remove('loading');
            }, 300);
        }
        
        // Close modal when clicking outside the image
        document.getElementById('imageModal').addEventListener('click', function(e) {
            if (e.target === this) {
                closeModal();
            }
        });
        
        // Close modal with Escape key
        document.addEventListener('keydown', function(e) {
            if (e.key === 'Escape') {
                closeModal();
            }
        });
        
        // Load albums on page load
        loadAlbums();
    </script>
</body>
</html>
            "#));
        
        // Add logging filter to see all requests
        let log = warp::log("api");
        
        let routes = root
            .or(static_files)
            .or(get_albums)
            .or(create_album)
            .or(get_album)
            .or(upload_media)
            .or(get_thumbnail)
            .or(get_medium)
            .or(get_high)
            .or(add_comment)
            .or(rotate_media)
            .recover(handle_rejection)
            .with(cors)
            .with(log);
        
        println!("🌐 Starting web album server on http://localhost:{}", port);
        println!("📁 Album interface available at http://localhost:{}", port);
        
        // Try to bind to the port, if it fails, try the next few ports
        let mut current_port = port;
        let max_attempts = 10;
        
        for attempt in 0..max_attempts {
            match std::net::TcpListener::bind((std::net::Ipv4Addr::new(127, 0, 0, 1), current_port)) {
                Ok(listener) => {
                    drop(listener); // Release the port for warp to use
                    if current_port != port {
                        println!("🔄 Port {} was busy, using port {} instead", port, current_port);
                        println!("🌐 Starting web album server on http://localhost:{}", current_port);
                        println!("📁 Album interface available at http://localhost:{}", current_port);
                    }
                    
                    warp::serve(routes)
                        .run((std::net::Ipv4Addr::new(127, 0, 0, 1), current_port))
                        .await;
                    return Ok(());
                }
                Err(_) => {
                    current_port += 1;
                    if attempt == max_attempts - 1 {
                        eprintln!("❌ Could not bind to any port from {} to {}", port, current_port);
                        return Err("Failed to bind to any available port".into());
                    }
                }
            }
        }
        
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
            
            // Store the media data in block storage using BlockFile for large files
            let block_file = BlockFile::new(Arc::clone(&server.block_manager));
            match block_file.append(&data).await {
                Ok(()) => {
                    let block_id = block_file.root_block().await
                        .ok_or_else(|| {
                            eprintln!("Failed to get root block ID after write");
                            warp::reject::reject()
                        })?;
                    // Detect media type
                    let media_type = if is_video_file(&data) {
                        "video"
                    } else if is_audio_file(&data) {
                        "audio"
                    } else {
                        "image"
                    };
                    
                    // Create an operation to add media
                    let op = EditOp {
                        op_id: Uuid::new_v4(),
                        timestamp: chrono::Utc::now().timestamp() as u64,
                        author: "web_user".to_string(),
                        op_type: "add_media".to_string(),
                        payload: serde_json::json!({
                            "filename": filename,
                            "block_id": hex::encode(block_id),
                            "size": data.len(),
                            "media_type": media_type
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
    get_media_at_resolution(album_id, media_id, server, 150).await
}

async fn handle_get_medium(
    album_id: String,
    media_id: String,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    get_media_at_resolution(album_id, media_id, server, 600).await
}

async fn handle_get_high(
    album_id: String,
    media_id: String,
    server: Arc<WebAlbumServer>
) -> Result<impl Reply, warp::Rejection> {
    get_media_at_resolution(album_id, media_id, server, 1200).await
}

async fn get_media_at_resolution(
    album_id: String,
    media_id: String,
    server: Arc<WebAlbumServer>,
    max_size: u32
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
                                
                                // Try to read the original media data
                                if let Ok(media_data) = server.block_manager.read_block(&block_id).await {
                                    // Generate resized image from the actual media
                                    if let Ok(resized_image) = generate_resized_media(&media_data, max_size) {
                                        return Ok(warp::reply::with_header(
                                            resized_image,
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
    generate_resized_media(media_data, 150)
}

fn generate_resized_media(media_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to detect the media type by checking the first few bytes
    let is_video = is_video_file(media_data);
    let is_audio = is_audio_file(media_data);
    let is_image = is_image_file(media_data);
    
    println!("Generating resized media: is_video={}, is_audio={}, is_image={}, data_len={}, max_size={}", is_video, is_audio, is_image, media_data.len(), max_size);
    
    if media_data.len() >= 12 {
        println!("First 12 bytes: {:?}", &media_data[0..12]);
        println!("First 12 bytes as hex: {}", hex::encode(&media_data[0..12]));
    }
    
    if is_video {
        println!("Generating video at size {}", max_size);
        generate_video_at_size(media_data, max_size)
    } else if is_audio {
        println!("Generating audio waveform at size {}", max_size);
        generate_audio_at_size(media_data, max_size)
    } else {
        println!("Generating image at size {}", max_size);
        generate_image_at_size(media_data, max_size)
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
    
    // Check for QuickTime/MOV signature
    if data.len() >= 8 && &data[4..8] == b"moov" {
        return true;
    }
    
    // Check for additional MP4 variants
    if data.len() >= 12 {
        let ftyp_slice = &data[4..8];
        if ftyp_slice == b"ftyp" {
            let brand = &data[8..12];
            // Common MP4 brands
            if brand == b"isom" || brand == b"mp41" || brand == b"mp42" || 
               brand == b"avc1" || brand == b"dash" || brand == b"iso2" {
                return true;
            }
        }
    }
    
    false
}

fn is_image_file(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    
    // JPEG
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        return true;
    }
    
    // PNG
    if data.len() >= 8 && &data[0..8] == b"\x89PNG\r\n\x1a\n" {
        return true;
    }
    
    // GIF
    if data.len() >= 6 && (&data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a") {
        return true;
    }
    
    // BMP
    if data.len() >= 2 && &data[0..2] == b"BM" {
        return true;
    }
    
    // TIFF
    if data.len() >= 4 && (&data[0..4] == b"II*\0" || &data[0..4] == b"MM\0*") {
        return true;
    }
    
    // WebP
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return true;
    }
    
    false
}

fn is_audio_file(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    
    // Check for MP3 signature
    if data.len() >= 3 {
        // MP3 with ID3v2 tag
        if &data[0..3] == b"ID3" {
            return true;
        }
        // MP3 frame sync
        if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
            return true;
        }
    }
    
    // Check for FLAC signature
    if data.len() >= 4 && &data[0..4] == b"fLaC" {
        return true;
    }
    
    // Check for OGG signature (Vorbis/Opus)
    if data.len() >= 4 && &data[0..4] == b"OggS" {
        return true;
    }
    
    // Check for WAV signature
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
        return true;
    }
    
    // Check for AIFF signature
    if data.len() >= 12 && &data[0..4] == b"FORM" && &data[8..12] == b"AIFF" {
        return true;
    }
    
    // Check for M4A (AAC in MP4 container)
    if data.len() >= 12 {
        let ftyp_slice = &data[4..8];
        if ftyp_slice == b"ftyp" {
            let brand = &data[8..12];
            // Common M4A brands
            if brand == b"M4A " || brand == b"mp42" || brand == b"isom" {
                return true;
            }
        }
    }
    
    // Check for common audio file extensions in filename if we had it
    // For now, let's also check some other common audio patterns
    
    // Check for AAC ADTS header
    if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
        return true;
    }
    
    // Check for AC3 signature
    if data.len() >= 2 && data[0] == 0x0B && data[1] == 0x77 {
        return true;
    }
    
    // For debugging: if it's a very small file that's not an image, assume it's audio
    if data.len() < 1000 && !is_image_file(data) {
        println!("Small file detected as audio (fallback): {} bytes", data.len());
        return true;
    }
    
    false
}

fn generate_image_thumbnail(image_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    generate_image_at_size(image_data, 150)
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

fn generate_video_thumbnail(video_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    generate_video_at_size(video_data, 150)
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
    // Try using FFmpeg command line tool to extract a frame
    // This is a simpler approach than using the FFmpeg Rust bindings
    
    use std::process::Command;
    
    // Create temporary files
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join(format!("video_input_{}.mp4", uuid::Uuid::new_v4()));
    let output_path = temp_dir.join(format!("frame_output_{}.jpg", uuid::Uuid::new_v4()));
    
    // Write video data to temporary file
    std::fs::write(&input_path, video_data)?;
    
    // Extract frame at 1 second using FFmpeg
    let output = Command::new("ffmpeg")
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

fn extract_audio_waveform(audio_data: &[u8]) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // Use FFmpeg to extract raw audio samples
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join(format!("audio_input_{}.tmp", uuid::Uuid::new_v4()));
    let output_path = temp_dir.join(format!("audio_output_{}.raw", uuid::Uuid::new_v4()));
    
    // Write audio data to temporary file
    std::fs::write(&input_path, audio_data)?;
    
    // Extract raw audio samples using FFmpeg
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", input_path.to_str().unwrap(),
            "-f", "f32le",          // 32-bit float little-endian
            "-ac", "1",             // Convert to mono
            "-ar", "8000",          // Sample rate 8kHz (sufficient for waveform visualization)
            "-y",                   // Overwrite output
            output_path.to_str().unwrap()
        ])
        .output();
    
    // Clean up input file
    let _ = std::fs::remove_file(&input_path);
    
    match output {
        Ok(result) if result.status.success() => {
            // Read the raw audio samples
            let raw_data = std::fs::read(&output_path)?;
            let _ = std::fs::remove_file(&output_path);
            
            // Convert bytes to f32 samples
            let mut samples = Vec::new();
            for chunk in raw_data.chunks_exact(4) {
                let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(sample);
            }
            
            // Downsample for visualization (keep every Nth sample based on length)
            let target_samples = 2000; // Target number of samples for visualization
            if samples.len() > target_samples {
                let step = samples.len() / target_samples;
                let downsampled: Vec<f32> = samples.iter().step_by(step).cloned().collect();
                Ok(downsampled)
            } else {
                Ok(samples)
            }
        }
        _ => {
            let _ = std::fs::remove_file(&output_path);
            Err("FFmpeg audio extraction failed".into())
        }
    }
}

fn generate_audio_thumbnail(_audio_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    generate_audio_at_size(_audio_data, 150)
}

fn generate_audio_at_size(audio_data: &[u8], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Try to extract waveform data from the audio
    if let Ok(waveform_data) = extract_audio_waveform(audio_data) {
        // Generate waveform visualization based on the requested size
        if max_size <= 200 {
            // Square thumbnail for small sizes
            return generate_square_waveform(&waveform_data, max_size);
        } else {
            // Wide rectangular waveform for larger sizes
            return generate_wide_waveform(&waveform_data, max_size);
        }
    }
    
    // Fall back to placeholder if waveform extraction fails
    create_audio_placeholder_at_size(max_size)
}

fn create_audio_placeholder_thumbnail() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    create_audio_placeholder_at_size(150)
}

fn generate_square_waveform(samples: &[f32], size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(size, size);
    let center_y = size / 2;
    
    // Dark background
    for pixel in img.pixels_mut() {
        *pixel = Rgb([15, 20, 30]);
    }
    
    // Calculate how many samples to show in the square
    let samples_to_show = (size as usize).min(samples.len());
    let sample_step = if samples.len() > samples_to_show {
        samples.len() / samples_to_show
    } else {
        1
    };
    
    // Draw circular waveform for square thumbnail
    let radius = (size / 2) as f32 * 0.8;
    let center_x = size / 2;
    
    for i in 0..samples_to_show {
        let sample_idx = i * sample_step;
        if sample_idx >= samples.len() { break; }
        
        let amplitude = samples[sample_idx].abs().min(1.0);
        let angle = (i as f32 / samples_to_show as f32) * 2.0 * std::f32::consts::PI;
        
        // Inner and outer radius based on amplitude
        let inner_radius = radius * 0.3;
        let outer_radius = inner_radius + (radius * 0.4 * amplitude);
        
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();
        
        // Draw line from inner to outer radius
        let steps = ((outer_radius - inner_radius) as u32).max(1);
        for step in 0..steps {
            let r = inner_radius + (step as f32 / steps as f32) * (outer_radius - inner_radius);
            let x = (center_x as f32 + r * cos_angle) as u32;
            let y = (center_y as f32 + r * sin_angle) as u32;
            
            if x < size && y < size {
                let intensity = (255.0 * (1.0 - step as f32 / steps as f32)) as u8;
                img.put_pixel(x, y, Rgb([intensity / 4, intensity / 2, intensity]));
            }
        }
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    Ok(buffer)
}

fn generate_wide_waveform(samples: &[f32], max_size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbImage, Rgb};
    
    // Create a wide aspect ratio (3:1 or 4:1 depending on size)
    let width = max_size;
    let height = max_size / 3;
    
    let mut img = RgbImage::new(width, height);
    let center_y = height / 2;
    
    // Dark background
    for pixel in img.pixels_mut() {
        *pixel = Rgb([15, 20, 30]);
    }
    
    // Draw traditional horizontal waveform
    let samples_per_pixel = (samples.len() as f32 / width as f32).max(1.0);
    
    for x in 0..width {
        let sample_start = (x as f32 * samples_per_pixel) as usize;
        let sample_end = ((x + 1) as f32 * samples_per_pixel) as usize;
        
        // Find min and max amplitude in this pixel's range
        let mut min_amp = 0.0f32;
        let mut max_amp = 0.0f32;
        let mut rms = 0.0f32;
        let mut count = 0;
        
        for i in sample_start..sample_end.min(samples.len()) {
            let sample = samples[i];
            min_amp = min_amp.min(sample);
            max_amp = max_amp.max(sample);
            rms += sample * sample;
            count += 1;
        }
        
        if count > 0 {
            rms = (rms / count as f32).sqrt();
            
            // Scale amplitudes to pixel coordinates
            let max_height = (height / 2) as f32 * 0.9;
            let min_y = (center_y as f32 - max_amp * max_height) as u32;
            let max_y = (center_y as f32 - min_amp * max_height) as u32;
            let rms_y1 = (center_y as f32 - rms * max_height) as u32;
            let rms_y2 = (center_y as f32 + rms * max_height) as u32;
            
            // Draw the waveform line
            for y in min_y..=max_y.min(height - 1) {
                if y < height {
                    // Brighter color for the main waveform
                    img.put_pixel(x, y, Rgb([100, 150, 255]));
                }
            }
            
            // Draw RMS envelope with different color
            if rms_y1 < height {
                img.put_pixel(x, rms_y1, Rgb([255, 200, 100]));
            }
            if rms_y2 < height {
                img.put_pixel(x, rms_y2, Rgb([255, 200, 100]));
            }
        }
    }
    
    // Add subtle grid lines
    let grid_spacing = width / 10;
    for i in 1..10 {
        let grid_x = i * grid_spacing;
        if grid_x < width {
            for y in 0..height {
                if y % 4 == 0 {
                    img.put_pixel(grid_x, y, Rgb([30, 35, 45]));
                }
            }
        }
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    Ok(buffer)
}

fn create_audio_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(size, size);
    let center_x = size / 2;
    let center_y = size / 2;
    
    // Create a dark background with audio waveform-like visualization
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // Create a dark audio-like background
        *pixel = Rgb([20, 25, 35]); // Dark blue-gray background
        
        // Draw simplified waveform bars scaled to size
        let bar_width = (size / 40).max(2);
        let bar_spacing = (size / 25).max(3);
        let num_bars = size / bar_spacing;
        
        for i in 0..num_bars {
            let bar_x = i * bar_spacing + size / 15;
            let bar_height = (size / 8) + ((i * 7) % (size / 4)); // Varying heights for waveform effect
            let bar_top = center_y - bar_height / 2;
            let bar_bottom = center_y + bar_height / 2;
            
            if x >= bar_x && x < bar_x + bar_width && y >= bar_top && y <= bar_bottom {
                // Create gradient effect for bars
                let intensity = 255 - ((y as i32 - bar_top as i32).abs() * 100 / bar_height as i32).min(100) as u8;
                *pixel = Rgb([intensity / 3, intensity / 2, intensity]); // Blue-ish gradient
            }
        }
        
        // Add a subtle border
        let border_width = (size / 75).max(1);
        if x < border_width || x >= size - border_width || y < border_width || y >= size - border_width {
            *pixel = Rgb([60, 70, 90]); // Lighter border
        }
    }
    
    let mut buffer = Vec::new();
    let dynamic_img = image::DynamicImage::ImageRgb8(img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    Ok(buffer)
}

fn create_video_placeholder_thumbnail() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    create_video_placeholder_at_size(150)
}

fn create_video_placeholder_at_size(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{RgbImage, Rgb};
    
    let mut img = RgbImage::new(size, size);
    let center_x = size / 2;
    let center_y = size / 2;
    
    // Create a dark background with a prominent play button
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // Create a dark video-like background
        *pixel = Rgb([30, 30, 30]);
        
        // Draw a large white play triangle scaled to size
        let triangle_size = size / 5;
        
        // Draw a proper right-pointing triangle (play button)
        let triangle_left = center_x - triangle_size / 3;
        let triangle_right = center_x + triangle_size / 3;
        
        // Check if we're in the triangle area
        if x >= triangle_left && x <= triangle_right {
            let relative_x = x as i32 - triangle_left as i32;
            let triangle_width = (triangle_right - triangle_left) as i32;
            
            // Calculate the triangle bounds at this x position (right-pointing)
            let half_height_at_x = (relative_x * triangle_size as i32) / (triangle_width * 2);
            let top_bound = center_y as i32 - half_height_at_x;
            let bottom_bound = center_y as i32 + half_height_at_x;
            
            if y as i32 >= top_bound && y as i32 <= bottom_bound {
                *pixel = Rgb([255, 255, 255]); // White play button
            }
        }
        
        // Add a subtle border to make it look more like a video thumbnail
        let border_width = (size / 50).max(1);
        if x < border_width || x >= size - border_width || y < border_width || y >= size - border_width {
            *pixel = Rgb([100, 100, 100]); // Gray border
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
