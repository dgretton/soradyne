//! Core CRDT traits and types for the album system

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::flow::FlowError;

// === Core Types ===

pub type OpId = Uuid;
pub type ReplicaId = String;
pub type LogicalTime = u64;
pub type MediaId = String;
pub type UserId = String; // Placeholder for now

// === Operation Trait ===

/// A single CRDT operation that can be applied and merged
pub trait CrdtOp: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// Globally unique ID for this operation
    fn id(&self) -> OpId;
    
    /// Logical timestamp for ordering
    fn timestamp(&self) -> LogicalTime;
    
    /// Which replica created this operation
    fn author(&self) -> ReplicaId;
    
    /// Operation type for interpretation
    fn op_type(&self) -> &str;
}

// === Core CRDT Trait ===

/// A conflict-free replicated data type
pub trait Crdt<Op: CrdtOp>: Clone + Send + Sync {
    type State;
    type Error;

    /// Apply a local operation
    fn apply_local(&mut self, op: Op) -> Result<(), Self::Error>;
    
    /// Merge operations from another replica
    fn merge(&mut self, other: &Self) -> Result<(), Self::Error>;
    
    /// Get all operations (for syncing)
    fn ops(&self) -> &[Op];
    
    /// Reduce operations to current state
    fn reduce(&self) -> Self::State;
    
    /// Check if we have a specific operation
    fn has_op(&self, op_id: &OpId) -> bool;
    
    /// Get operations since a given timestamp (for incremental sync)
    fn ops_since(&self, timestamp: LogicalTime) -> Vec<Op>;
}

// === Reducer Trait ===

/// Interprets operations into displayable state
pub trait Reducer<Op: CrdtOp> {
    type State;
    type Error;
    
    /// Build state from a sequence of operations
    fn reduce(ops: &[Op]) -> Result<Self::State, Self::Error>;
    
    /// Incrementally apply a single operation to existing state
    fn apply_to_state(state: &mut Self::State, op: &Op) -> Result<(), Self::Error>;
}

// === Collection Management ===

/// Manages a collection of CRDTs (like an album of media items)
pub trait CrdtCollection<Key, Op: CrdtOp>: Send + Sync 
where 
    Key: Clone + Eq + std::hash::Hash + Send + Sync,
{
    type ItemCrdt: Crdt<Op>;
    type Error;
    
    /// Get or create a CRDT for a specific item
    fn get_or_create(&mut self, key: &Key) -> &mut Self::ItemCrdt;
    
    /// Get an existing CRDT (read-only)
    fn get(&self, key: &Key) -> Option<&Self::ItemCrdt>;
    
    /// Apply an operation to a specific item
    fn apply_to_item(&mut self, key: &Key, op: Op) -> Result<(), Self::Error>;
    
    /// Merge another collection
    fn merge_collection(&mut self, other: &Self) -> Result<(), Self::Error>;
    
    /// List all keys
    fn keys(&self) -> Vec<Key>;
    
    /// Get the state of all items
    fn reduce_all(&self) -> HashMap<Key, <Self::ItemCrdt as Crdt<Op>>::State>;
}

// === Error Types ===

#[derive(Debug, thiserror::Error)]
pub enum CrdtError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Flow error: {0}")]
    Flow(#[from] FlowError),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Operation not found: {0}")]
    OperationNotFound(OpId),
}

// === Permission Types ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    View,
    Comment,
    Edit,
    Admin,
}

impl Permission {
    pub fn can_view(&self) -> bool {
        matches!(self, Permission::View | Permission::Comment | Permission::Edit | Permission::Admin)
    }
    
    pub fn can_comment(&self) -> bool {
        matches!(self, Permission::Comment | Permission::Edit | Permission::Admin)
    }
    
    pub fn can_edit(&self) -> bool {
        matches!(self, Permission::Edit | Permission::Admin)
    }
    
    pub fn can_admin(&self) -> bool {
        matches!(self, Permission::Admin)
    }
}
