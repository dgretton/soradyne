//! Core functionality for the Soradyne protocol

pub mod identity;
pub mod transport;
pub mod sync;

// Re-export key types for convenience
pub use identity::{Identity, KeyPair, IdentityManager};
pub use transport::{Connection, ConnectionManager, PeerInfo};
pub use sync::{SyncPrimitive, RealTimeSync, EventualSync};
