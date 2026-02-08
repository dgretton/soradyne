//! The five primitive operations for convergent documents

use super::horizon::{DeviceId, Horizon, SeqNum};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an operation
pub type OpId = Uuid;

/// Unique identifier for an item within a document
pub type ItemId = String;

/// A value that can be stored in fields or sets
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Map(std::collections::BTreeMap<String, Value>),
}

impl Value {
    pub fn string(s: impl Into<String>) -> Self {
        Value::String(s.into())
    }

    pub fn int(n: i64) -> Self {
        Value::Int(n)
    }

    pub fn bool(b: bool) -> Self {
        Value::Bool(b)
    }
}

/// The five primitive operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    /// Create a new item
    AddItem {
        item_id: ItemId,
        item_type: String,
    },

    /// Remove an item (informed-remove: only affects what horizon had seen)
    RemoveItem {
        item_id: ItemId,
    },

    /// Set a scalar field on an item (latest-wins)
    SetField {
        item_id: ItemId,
        field: String,
        value: Value,
    },

    /// Add an element to a collection field
    AddToSet {
        item_id: ItemId,
        set_name: String,
        element: Value,
    },

    /// Remove an element from a collection field (informed-remove)
    RemoveFromSet {
        item_id: ItemId,
        set_name: String,
        element: Value,
        /// The specific AddToSet operation IDs this remove observed
        observed_add_ids: Vec<OpId>,
    },
}

impl Operation {
    /// Get the item ID this operation affects
    pub fn item_id(&self) -> &ItemId {
        match self {
            Operation::AddItem { item_id, .. } => item_id,
            Operation::RemoveItem { item_id } => item_id,
            Operation::SetField { item_id, .. } => item_id,
            Operation::AddToSet { item_id, .. } => item_id,
            Operation::RemoveFromSet { item_id, .. } => item_id,
        }
    }

    /// Create an AddItem operation
    pub fn add_item(item_id: impl Into<ItemId>, item_type: impl Into<String>) -> Self {
        Operation::AddItem {
            item_id: item_id.into(),
            item_type: item_type.into(),
        }
    }

    /// Create a RemoveItem operation
    pub fn remove_item(item_id: impl Into<ItemId>) -> Self {
        Operation::RemoveItem {
            item_id: item_id.into(),
        }
    }

    /// Create a SetField operation
    pub fn set_field(item_id: impl Into<ItemId>, field: impl Into<String>, value: Value) -> Self {
        Operation::SetField {
            item_id: item_id.into(),
            field: field.into(),
            value,
        }
    }

    /// Create an AddToSet operation
    pub fn add_to_set(
        item_id: impl Into<ItemId>,
        set_name: impl Into<String>,
        element: Value,
    ) -> Self {
        Operation::AddToSet {
            item_id: item_id.into(),
            set_name: set_name.into(),
            element,
        }
    }

    /// Create a RemoveFromSet operation
    pub fn remove_from_set(
        item_id: impl Into<ItemId>,
        set_name: impl Into<String>,
        element: Value,
        observed_add_ids: Vec<OpId>,
    ) -> Self {
        Operation::RemoveFromSet {
            item_id: item_id.into(),
            set_name: set_name.into(),
            element,
            observed_add_ids,
        }
    }
}

/// An operation wrapped with metadata for transmission and storage
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpEnvelope {
    /// Unique ID for this operation
    pub id: OpId,

    /// Which device authored this operation
    pub author: DeviceId,

    /// Sequence number within the author's stream
    pub seq: SeqNum,

    /// Wall-clock timestamp (for latest-wins tiebreaking)
    pub timestamp: u64,

    /// What the author had seen when they created this operation
    pub horizon: Horizon,

    /// The actual operation
    pub op: Operation,
}

impl OpEnvelope {
    /// Create a new operation envelope
    pub fn new(author: DeviceId, seq: SeqNum, horizon: Horizon, op: Operation) -> Self {
        Self {
            id: Uuid::new_v4(),
            author,
            seq,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            horizon,
            op,
        }
    }

    /// Check if this operation was informed about another operation
    pub fn had_seen(&self, other: &OpEnvelope) -> bool {
        self.horizon.has_seen(&other.author, other.seq)
    }

    /// Compare for latest-wins ordering (timestamp, then author as tiebreaker)
    pub fn is_later_than(&self, other: &OpEnvelope) -> bool {
        if self.timestamp != other.timestamp {
            self.timestamp > other.timestamp
        } else {
            self.author > other.author
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_item_id() {
        let op = Operation::add_item("task_1", "GianttItem");
        assert_eq!(op.item_id(), "task_1");

        let op = Operation::set_field("task_1", "title", Value::string("Hello"));
        assert_eq!(op.item_id(), "task_1");
    }

    #[test]
    fn test_envelope_had_seen() {
        let mut h1 = Horizon::new();
        h1.observe(&"A".into(), 5);

        let env1 = OpEnvelope::new(
            "B".into(),
            1,
            h1,
            Operation::add_item("x", "Test"),
        );

        let env2 = OpEnvelope::new(
            "A".into(),
            3,
            Horizon::new(),
            Operation::add_item("y", "Test"),
        );

        // env1's author had seen A up to seq 5, so had seen env2 (A:3)
        assert!(env1.had_seen(&env2));
    }
}
