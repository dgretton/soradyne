//! types/heartrate.rs
//!
//! Defines the Heartrate struct and the HeartrateChannel type, which is a concrete
//! DataChannel for streaming and synchronizing heartrate data across devices.

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::flow::DataChannel;

/// Heartrate data structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heartrate {
    pub bpm: f32,
    pub source_device_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

impl Heartrate {
    pub fn new(bpm: f32, source_device_id: Uuid) -> Self {
        Self {
            bpm,
            source_device_id,
            timestamp: Utc::now(),
        }
    }
}

/// A type alias for DataChannel carrying Heartrate data.
/// This would typically be used as a stream within a larger Flow.
pub type HeartrateChannel = DataChannel<Heartrate>;

/// DEPRECATED: Use HeartrateChannel instead.
#[deprecated(since = "0.2.0", note = "Use HeartrateChannel instead")]
pub type HeartrateFlow = DataChannel<Heartrate>;

