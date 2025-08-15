//! Block storage data structures

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub const BLOCK_SIZE: usize = 32 * 1024 * 1024; // 32MB
pub const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks for streaming
pub const BLOCK_ID_SIZE: usize = 32;

pub type BlockId = [u8; BLOCK_ID_SIZE];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockMetadata {
    pub id: [u8; 32],
    pub directness: u8,
    pub size: usize,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub shard_locations: Vec<ShardLocation>,
    #[serde(default)]
    pub encryption_version: u8, // 0 = legacy RS-only, 1 = Shamir+RS
    #[serde(default)]
    pub nonce: [u8; 12], // AES-GCM nonce derived from block ID
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardLocation {
    pub shard_index: usize,
    pub device_id: Uuid,
    pub rimsd_path: String,
    pub relative_path: String,
    #[serde(default)]
    pub key_share_path: Option<String>, // Path to Shamir key share (for v1+ blocks)
}

#[derive(Debug)]
pub enum Block {
    Direct(DirectBlock),
    Indirect(IndirectBlock),
}

#[derive(Debug)]
pub struct DirectBlock {
    pub metadata: BlockMetadata,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct IndirectBlock {
    pub metadata: BlockMetadata,
    pub addresses: Vec<[u8; 32]>,
}

impl Block {
    pub fn directness(&self) -> u8 {
        match self {
            Block::Direct(b) => b.metadata.directness,
            Block::Indirect(b) => b.metadata.directness,
        }
    }
    
    pub fn id(&self) -> &[u8; 32] {
        match self {
            Block::Direct(b) => &b.metadata.id,
            Block::Indirect(b) => &b.metadata.id,
        }
    }
    
    pub fn size(&self) -> usize {
        match self {
            Block::Direct(b) => b.metadata.size,
            Block::Indirect(b) => b.metadata.size,
        }
    }
}
