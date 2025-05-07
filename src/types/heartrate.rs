//! types/heartrate.rs
//!
//! Defines the Heartrate struct and the HeartrateFlow type, which is a concrete
//! SelfDataFlow for streaming and synchronizing heartrate data across devices.

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::flow::SelfDataFlow;

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

/// A type alias for SelfDataFlow carrying Heartrate data
pub type HeartrateFlow = SelfDataFlow<Heartrate>;

