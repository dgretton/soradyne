//! Heart Rate Self-Data Object for Soradyne
//!
//! This module implements a real-time SDO for heart rate data.

use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::sdo::{RealtimeSDO, SDOType};

/// Heart rate data point
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeartRateData {
    /// Beats per minute
    pub bpm: f32,
    
    /// Heart rate variability (milliseconds)
    pub hrv: Option<f32>,
    
    /// Timestamp when this measurement was taken
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HeartRateData {
    /// Create a new heart rate data point
    pub fn new(bpm: f32, hrv: Option<f32>) -> Self {
        Self {
            bpm,
            hrv,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Heart rate Self-Data Object
///
/// This SDO is used to share real-time heart rate data between devices.
pub type HeartRateSDO = RealtimeSDO<HeartRateData>;

impl HeartRateSDO {
    /// Create a new heart rate SDO
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        // Create an initial data point with zero BPM
        let initial_data = HeartRateData::new(0.0, None);
        
        // Create the SDO
        RealtimeSDO::create(name, owner_id, initial_data)
    }
    
    /// Update the heart rate
    pub fn update_heart_rate(&self, identity_id: Uuid, bpm: f32, hrv: Option<f32>) -> Result<(), crate::sdo::base::SDOError> {
        let data = HeartRateData::new(bpm, hrv);
        self.set_value(identity_id, data)
    }
    
    /// Get the current heart rate
    pub fn get_heart_rate(&self) -> Result<HeartRateData, crate::sdo::base::SDOError> {
        self.get_value()
    }
    
    /// Get heart rate history within a time range
    pub fn get_history(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<(chrono::DateTime<chrono::Utc>, HeartRateData)>, crate::sdo::base::SDOError> {
        self.get_history_data(start, end)
    }
}
