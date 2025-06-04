//! Album and media item implementations

use super::crdt::*;
use super::operations::*;
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use crate::storage::block_manager::BlockManager;
use std::sync::Arc;


// === Simple Log CRDT Implementation ===

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogCrdt {
    ops: Vec<EditOp>,
}

impl LogCrdt {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }
    
    pub fn get_state(&self) -> Result<MediaState, CrdtError> {
        Ok(self.reduce())
    }
    
    fn sort_ops(&mut self) {
        self.ops.sort_by(|a, b| {
            a.timestamp().cmp(&b.timestamp())
                .then_with(|| a.author().cmp(&b.author()))
                .then_with(|| a.id().cmp(&b.id()))
        });
        
        // Deduplicate by op_id
        self.ops.dedup_by(|a, b| a.id() == b.id());
    }
}

impl Crdt<EditOp> for LogCrdt {
    type State = MediaState;
    type Error = CrdtError;
    
    fn apply_local(&mut self, op: EditOp) -> Result<(), Self::Error> {
        if !self.has_op(&op.id()) {
            self.ops.push(op);
            self.sort_ops();
        }
        Ok(())
    }
    
    fn merge(&mut self, other: &Self) -> Result<(), Self::Error> {
        for op in &other.ops {
            if !self.has_op(&op.id()) {
                self.ops.push(op.clone());
            }
        }
        self.sort_ops();
        Ok(())
    }
    
    fn ops(&self) -> &[EditOp] {
        &self.ops
    }
    
    fn reduce(&self) -> Self::State {
        MediaReducer::reduce(&self.ops).unwrap_or_default()
    }
    
    fn has_op(&self, op_id: &OpId) -> bool {
        self.ops.iter().any(|op| op.id() == *op_id)
    }
    
    fn ops_since(&self, timestamp: LogicalTime) -> Vec<EditOp> {
        self.ops.iter()
            .filter(|op| op.timestamp() > timestamp)
            .cloned()
            .collect()
    }
}

// === Media State ===

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MediaState {
    pub media: Option<MediaInfo>,
    pub comments: Vec<Comment>,
    pub reactions: HashMap<String, Vec<ReplicaId>>, // emoji -> users
    pub crop: Option<CropData>,
    pub rotation: f32,
    pub markup: Vec<MarkupElement>,
    pub deleted_items: HashSet<OpId>,
    pub shared_with: HashMap<UserId, Permission>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaInfo {
    pub block_id: [u8; 32],
    pub media_type: MediaType,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub added_by: ReplicaId,
    pub added_at: LogicalTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: OpId,
    pub author: ReplicaId,
    pub text: String,
    pub parent: Option<OpId>,
    pub timestamp: LogicalTime,
    pub deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CropData {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkupElement {
    pub id: OpId,
    pub markup_type: MarkupType,
    pub data: serde_json::Value,
    pub author: ReplicaId,
    pub timestamp: LogicalTime,
}


// === Media Reducer ===

pub struct MediaReducer;

impl Reducer<EditOp> for MediaReducer {
    type State = MediaState;
    type Error = CrdtError;
    
    fn reduce(ops: &[EditOp]) -> Result<Self::State, Self::Error> {
        let mut state = MediaState::default();
        
        for op in ops {
            Self::apply_to_state(&mut state, op)?;
        }
        
        Ok(state)
    }
    
    fn apply_to_state(state: &mut Self::State, op: &EditOp) -> Result<(), Self::Error> {
        match op.op_type.as_str() {
            "add_media" | "set_media" => {
                // Handle both add_media (from web interface) and set_media operations
                if let Some(block_id_hex) = op.payload.get("block_id").and_then(|v| v.as_str()) {
                    if let Ok(block_id_bytes) = hex::decode(block_id_hex) {
                        if block_id_bytes.len() == 32 {
                            let mut block_id = [0u8; 32];
                            block_id.copy_from_slice(&block_id_bytes);
                            
                            let filename = op.payload.get("filename")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let size = op.payload.get("size")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as usize;
                            
                            state.media = Some(MediaInfo {
                                block_id,
                                media_type: MediaType::Photo, // Default to photo
                                filename,
                                mime_type: "image/jpeg".to_string(),
                                size,
                                added_by: op.author(),
                                added_at: op.timestamp(),
                            });
                        }
                    }
                }
                
                // Also handle the structured payload format
                if let Ok(payload) = serde_json::from_value::<SetMediaPayload>(op.payload.clone()) {
                    let mime_type = match payload.media_type {
                        MediaType::Photo => "image/jpeg".to_string(),
                        MediaType::Video => "video/mp4".to_string(),
                        MediaType::Audio => "audio/mp3".to_string(),
                    };
                    
                    state.media = Some(MediaInfo {
                        block_id: payload.block_id,
                        media_type: payload.media_type,
                        filename: payload.filename,
                        mime_type,
                        size: payload.size,
                        added_by: op.author(),
                        added_at: op.timestamp(),
                    });
                }
            }
            
            "add_comment" => {
                // Handle simple text payload from web interface
                if let Some(text) = op.payload.get("text").and_then(|v| v.as_str()) {
                    state.comments.push(Comment {
                        id: op.id(),
                        author: op.author(),
                        text: text.to_string(),
                        parent: None,
                        timestamp: op.timestamp(),
                        deleted: false,
                    });
                }
                
                // Also handle structured payload format
                if let Ok(payload) = serde_json::from_value::<CommentPayload>(op.payload.clone()) {
                    state.comments.push(Comment {
                        id: op.id(),
                        author: op.author(),
                        text: payload.text,
                        parent: payload.parent,
                        timestamp: op.timestamp(),
                        deleted: false,
                    });
                }
            }
            
            "delete" => {
                if let Ok(payload) = serde_json::from_value::<DeletePayload>(op.payload.clone()) {
                    state.deleted_items.insert(payload.target);
                    // Mark comment as deleted but keep it for replies
                    for comment in &mut state.comments {
                        if comment.id == payload.target {
                            comment.deleted = true;
                        }
                    }
                }
            }
            
            "add_reaction" => {
                if let Ok(payload) = serde_json::from_value::<ReactionPayload>(op.payload.clone()) {
                    state.reactions
                        .entry(payload.emoji)
                        .or_insert_with(Vec::new)
                        .push(op.author());
                }
            }
            
            "set_crop" => {
                if let Ok(payload) = serde_json::from_value::<CropPayload>(op.payload.clone()) {
                    state.crop = Some(CropData {
                        left: payload.left,
                        top: payload.top,
                        right: payload.right,
                        bottom: payload.bottom,
                    });
                }
            }
            
            "rotate" => {
                // Handle simple degrees payload from web interface
                if let Some(degrees) = op.payload.get("degrees").and_then(|v| v.as_f64()) {
                    state.rotation = degrees as f32; // LWW semantics
                }
                
                // Also handle structured payload format
                if let Ok(payload) = serde_json::from_value::<RotatePayload>(op.payload.clone()) {
                    state.rotation = payload.angle; // LWW semantics
                }
            }
            
            "add_markup" => {
                if let Ok(payload) = serde_json::from_value::<MarkupPayload>(op.payload.clone()) {
                    state.markup.push(MarkupElement {
                        id: op.id(),
                        markup_type: payload.markup_type,
                        data: payload.data,
                        author: op.author(),
                        timestamp: op.timestamp(),
                    });
                }
            }
            
            "share_with" => {
                if let Ok(payload) = serde_json::from_value::<SharePayload>(op.payload.clone()) {
                    state.shared_with.insert(payload.user_id, payload.permission);
                }
            }
            
            _ => {
                // Unknown operation type - ignore gracefully for forward compatibility
            }
        }
        
        Ok(())
    }
}

// === Album Collection ===

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaAlbum {
    pub album_id: String,
    pub items: HashMap<MediaId, LogCrdt>,
    pub metadata: AlbumMetadata,
    #[serde(skip)]
    pub block_manager: Option<Arc<BlockManager>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub title: String,
    pub created_by: ReplicaId,
    pub created_at: LogicalTime,
    pub shared_with: HashMap<UserId, Permission>,
}

impl Default for AlbumMetadata {
    fn default() -> Self {
        Self {
            title: "Untitled Album".to_string(),
            created_by: "unknown".to_string(),
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            shared_with: HashMap::new(),
        }
    }
}

impl MediaAlbum {
    pub fn new(album_id: String, title: String, created_by: ReplicaId) -> Self {
        Self {
            album_id,
            items: HashMap::new(),
            metadata: AlbumMetadata {
                title,
                created_by,
                created_at: chrono::Utc::now().timestamp_millis() as u64,
                shared_with: HashMap::new(),
            },
            block_manager: None,
        }
    }
    
    pub fn with_block_manager(mut self, block_manager: Arc<BlockManager>) -> Self {
        self.block_manager = Some(block_manager);
        self
    }
    
    /// Serialize the album to bytes for storage in block system
    pub fn to_bytes(&self) -> Result<Vec<u8>, CrdtError> {
        Ok(serde_json::to_vec(self)?)
    }
    
    /// Deserialize album from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, CrdtError> {
        Ok(serde_json::from_slice(data)?)
    }
}

impl CrdtCollection<MediaId, EditOp> for MediaAlbum {
    type ItemCrdt = LogCrdt;
    type Error = CrdtError;
    
    fn get_or_create(&mut self, key: &MediaId) -> &mut Self::ItemCrdt {
        self.items.entry(key.clone()).or_insert_with(LogCrdt::new)
    }
    
    fn get(&self, key: &MediaId) -> Option<&Self::ItemCrdt> {
        self.items.get(key)
    }
    
    fn apply_to_item(&mut self, key: &MediaId, op: EditOp) -> Result<(), Self::Error> {
        let crdt = self.get_or_create(key);
        crdt.apply_local(op)
    }
    
    fn merge_collection(&mut self, other: &Self) -> Result<(), Self::Error> {
        // Merge metadata (LWW based on created_at)
        if other.metadata.created_at > self.metadata.created_at {
            self.metadata = other.metadata.clone();
        }
        
        // Merge individual media items
        for (key, other_crdt) in &other.items {
            let our_crdt = self.get_or_create(key);
            our_crdt.merge(other_crdt)?;
        }
        Ok(())
    }
    
    fn keys(&self) -> Vec<MediaId> {
        self.items.keys().cloned().collect()
    }
    
    fn reduce_all(&self) -> HashMap<MediaId, MediaState> {
        self.items.iter()
            .map(|(k, v)| (k.clone(), v.reduce()))
            .collect()
    }
}
