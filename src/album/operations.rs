//! Operation types for album editing

use super::crdt::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Type aliases
pub type MediaId = String;
pub type UserId = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    View,
    Comment,
    Edit,
    Admin,
}

// === Edit Operation ===

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EditOp {
    pub op_id: OpId,
    pub timestamp: LogicalTime,
    pub author: ReplicaId,
    pub op_type: String,
    pub payload: serde_json::Value,
}

impl CrdtOp for EditOp {
    fn id(&self) -> OpId { 
        self.op_id 
    }
    
    fn timestamp(&self) -> LogicalTime { 
        self.timestamp 
    }
    
    fn author(&self) -> ReplicaId { 
        self.author.clone() 
    }
    
    fn op_type(&self) -> &str { 
        &self.op_type 
    }
}

impl EditOp {
    pub fn new(author: ReplicaId, op_type: String, payload: serde_json::Value) -> Self {
        Self {
            op_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            author,
            op_type,
            payload,
        }
    }
}

// === Operation Payloads ===

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SetMediaPayload {
    pub block_id: [u8; 32],
    pub media_type: MediaType,
    pub filename: String,
    pub size: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MediaType {
    Photo,
    Video,
    Audio,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CommentPayload {
    pub text: String,
    pub parent: Option<OpId>, // For threaded comments
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ReactionPayload {
    pub target: OpId, // What we're reacting to
    pub emoji: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CropPayload {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RotatePayload {
    pub angle: f32, // degrees
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MarkupPayload {
    pub markup_type: MarkupType,
    pub data: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MarkupType {
    Arrow,
    Circle,
    Rectangle,
    Text,
    Freehand,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SharePayload {
    pub user_id: UserId,
    pub permission: Permission,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DeletePayload {
    pub target: OpId, // What we're deleting
}

// === Operation Builders ===

impl EditOp {
    pub fn set_media(author: ReplicaId, block_id: [u8; 32], media_type: MediaType, filename: String, size: usize) -> Self {
        let payload = SetMediaPayload { block_id, media_type, filename, size };
        Self::new(author, "set_media".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn add_comment(author: ReplicaId, text: String, parent: Option<OpId>) -> Self {
        let payload = CommentPayload { text, parent };
        Self::new(author, "add_comment".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn add_reaction(author: ReplicaId, target: OpId, emoji: String) -> Self {
        let payload = ReactionPayload { target, emoji };
        Self::new(author, "add_reaction".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn set_crop(author: ReplicaId, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        let payload = CropPayload { left, top, right, bottom };
        Self::new(author, "set_crop".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn rotate(author: ReplicaId, angle: f32) -> Self {
        let payload = RotatePayload { angle };
        Self::new(author, "rotate".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn add_markup(author: ReplicaId, markup_type: MarkupType, data: serde_json::Value) -> Self {
        let payload = MarkupPayload { markup_type, data };
        Self::new(author, "add_markup".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn share_with(author: ReplicaId, user_id: UserId, permission: Permission) -> Self {
        let payload = SharePayload { user_id, permission };
        Self::new(author, "share_with".to_string(), serde_json::to_value(payload).unwrap())
    }
    
    pub fn delete(author: ReplicaId, target: OpId) -> Self {
        let payload = DeletePayload { target };
        Self::new(author, "delete".to_string(), serde_json::to_value(payload).unwrap())
    }
}
