//! Device topology: Capsules & Pieces
//!
//! This module provides the persistent data model for the capsule/piece topology.
//! Capsules are groups of devices (pieces) that share key material and sync data flows.
//! This is pre-flow infrastructure — capsules use trivial set-union merge, not CRDTs.

pub mod capsule;
pub mod capsule_store;

pub use capsule::{Capsule, CapsuleStatus, FlowConfig, PieceCapabilities, PieceRecord, PieceRole};
pub use capsule_store::CapsuleStore;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TopologyError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),
}
