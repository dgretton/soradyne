//! Robotics Self-Data Object for Soradyne
//!
//! This module implements a real-time SDO for robot state data.

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

use crate::sdo::{RealtimeSDO, SDOType};

/// Joint state data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointState {
    /// Joint position (radians)
    pub position: f32,
    
    /// Joint velocity (radians/s)
    pub velocity: f32,
    
    /// Joint effort (Nm)
    pub effort: f32,
}

/// Robot state data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobotState {
    /// Joint states by joint name
    pub joints: HashMap<String, JointState>,
    
    /// Timestamp when this state was captured
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Robot mode (e.g., "manual", "automatic", "error")
    pub mode: String,
    
    /// Whether the robot is enabled
    pub enabled: bool,
}

impl RobotState {
    /// Create a new robot state
    pub fn new() -> Self {
        Self {
            joints: HashMap::new(),
            timestamp: chrono::Utc::now(),
            mode: "standby".to_string(),
            enabled: false,
        }
    }
    
    /// Add or update a joint
    pub fn update_joint(&mut self, name: &str, position: f32, velocity: f32, effort: f32) {
        self.joints.insert(name.to_string(), JointState {
            position,
            velocity,
            effort,
        });
        
        // Update timestamp
        self.timestamp = chrono::Utc::now();
    }
    
    /// Set the robot mode
    pub fn set_mode(&mut self, mode: &str) {
        self.mode = mode.to_string();
        self.timestamp = chrono::Utc::now();
    }
    
    /// Enable or disable the robot
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.timestamp = chrono::Utc::now();
    }
}

impl Default for RobotState {
    fn default() -> Self {
        Self::new()
    }
}

/// Robot State Self-Data Object
///
/// This SDO is used to share real-time robot state data between devices.
pub type RobotStateSDO = RealtimeSDO<RobotState>;

impl RobotStateSDO {
    /// Create a new robot state SDO
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        // Create an initial empty state
        let initial_data = RobotState::new();
        
        // Create the SDO
        RealtimeSDO::new(name, owner_id, initial_data)
    }
    
    /// Get the current robot state
    pub fn get_robot_state(&self) -> Result<RobotState, crate::sdo::base::SDOError> {
        self.get_value()
    }
    
    /// Update the robot state
    pub fn update_robot_state(&self, identity_id: Uuid, state: RobotState) -> Result<(), crate::sdo::base::SDOError> {
        self.set_value(identity_id, state)
    }
    
    /// Update a single joint
    pub fn update_joint(&self, identity_id: Uuid, joint_name: &str, position: f32, velocity: f32, effort: f32) -> Result<(), crate::sdo::base::SDOError> {
        // Get the current state
        let mut state = self.get_value()?;
        
        // Update the joint
        state.update_joint(joint_name, position, velocity, effort);
        
        // Update the SDO
        self.set_value(identity_id, state)
    }
    
    /// Set the robot mode
    pub fn set_mode(&self, identity_id: Uuid, mode: &str) -> Result<(), crate::sdo::base::SDOError> {
        // Get the current state
        let mut state = self.get_value()?;
        
        // Update the mode
        state.set_mode(mode);
        
        // Update the SDO
        self.set_value(identity_id, state)
    }
    
    /// Enable or disable the robot
    pub fn set_enabled(&self, identity_id: Uuid, enabled: bool) -> Result<(), crate::sdo::base::SDOError> {
        // Get the current state
        let mut state = self.get_value()?;
        
        // Update the enabled status
        state.set_enabled(enabled);
        
        // Update the SDO
        self.set_value(identity_id, state)
    }
}
