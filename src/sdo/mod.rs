//! Self-Data Objects (SDOs) for Soradyne
//!
//! This module defines the core interfaces and implementations for SDOs,
//! which are the primary data sharing mechanism in Soradyne.

mod base;
mod realtime;
mod eventual;
pub mod types;

pub use base::{SelfDataObject, SDOType, SDOAccess, VersionVector, SDOMetadata};
pub use realtime::RealtimeSDO;
pub use eventual::EventualSDO;

// Export specific SDO implementations from the types module
pub use types::heartrate::HeartRateSDO;
pub use types::chat::ChatSDO;
pub use types::photos::PhotoAlbumSDO;
pub use types::files::FileSDO;
pub use types::robotics::RobotStateSDO;
