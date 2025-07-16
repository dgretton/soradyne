pub mod local_file;
pub mod block;
pub mod erasure;
pub mod block_manager;
pub mod block_file;
pub mod device_identity;

pub use local_file::LocalFileStorage;
pub use local_file::NoOpAuthenticator;
pub use block_manager::{BlockManager, StorageInfo, BlockDistribution, ShardInfo, DemonstrationResult};
pub use block_file::BlockFile;
pub use device_identity::{BasicFingerprint, BayesianDeviceIdentifier, fingerprint_device, discover_soradyne_volumes};
//! Storage subsystem for Soradyne
//! 
//! This module provides dissolution storage capabilities with multiple backend
//! implementations including manual erasure coding and bcachefs.

pub mod block;
pub mod block_file;
pub mod block_manager;
pub mod device_identity;
pub mod erasure;
pub mod local_file;

// New abstraction layer
pub mod dissolution;
pub mod backends;

// Re-export key types for convenience
pub use dissolution::{
    DissolutionStorage, DissolutionConfig, DissolutionFile, BlockId,
    BlockInfo, StorageStats, DissolutionDemo
};
pub use backends::{DissolutionStorageFactory, ManualErasureBackend};

#[cfg(target_os = "linux")]
pub use backends::BcacheFSBackend;
