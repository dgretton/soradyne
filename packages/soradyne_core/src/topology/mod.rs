//! Device topology: Capsules, Pieces, and Ensembles
//!
//! This module provides:
//! - Persistent capsule/piece data model (capsule, capsule_store)
//! - Runtime ensemble topology graph (ensemble)
//! - Topology synchronization protocol messages (sync)

pub mod capsule;
pub mod capsule_store;
pub mod ensemble;
pub mod manager;
pub mod messenger;
pub mod sync;

pub use capsule::{Capsule, CapsuleStatus, FlowConfig, PieceCapabilities, PieceRecord, PieceRole};
pub use capsule_store::CapsuleStore;
pub use ensemble::{
    ConnectionQuality, EnsembleTopology, PiecePresence, PieceReachability, TopologyEdge,
    TransportType,
};
pub use manager::{EnsembleConfig, EnsembleManager};
pub use messenger::{RoutingError, TopologyMessenger};
pub use sync::{PeerInfo, TopologySyncMessage};

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
