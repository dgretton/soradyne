//! Robot Joints Example
//!
//! This example demonstrates using DataChannel with the Diffable trait
//! for efficient updates to robot joint positions.
//!
//! In a full flow-based architecture, this DataChannel would be registered
//! as a stream within a Flow. For now, it shows the basic mechanics of
//! reactive data with diff support.

use std::collections::HashMap;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use crate::flow::{DataChannel, FlowType, Diffable};

/// Represents a single robot joint position
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointPosition {
    /// Joint name
    pub name: String,
    /// Current angle in radians
    pub angle: f64,
    /// Current velocity in radians per second
    pub velocity: f64,
    /// Last update timestamp
    pub timestamp: u64,
}

/// Represents the state of all joints in a robot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobotJointState {
    /// Robot identifier
    pub robot_id: String,
    /// Map of joint name to joint position
    pub joints: HashMap<String, JointPosition>,
    /// Last update timestamp
    pub last_update: u64,
}

/// Represents a change to a single joint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointDiff {
    /// Joint name
    pub name: String,
    /// New angle (if changed)
    pub angle: Option<f64>,
    /// New velocity (if changed)
    pub velocity: Option<f64>,
    /// Update timestamp
    pub timestamp: u64,
}

/// Represents changes to multiple joints
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobotJointDiff {
    /// List of joint changes
    pub changes: Vec<JointDiff>,
    /// Update timestamp
    pub timestamp: u64,
}

impl Diffable for RobotJointState {
    type Diff = RobotJointDiff;
    
    fn diff(&self, other: &Self) -> Self::Diff {
        let mut changes = Vec::new();
        
        // Check for changes in existing joints
        for (name, joint) in &other.joints {
            if let Some(self_joint) = self.joints.get(name) {
                let mut diff_needed = false;
                let mut joint_diff = JointDiff {
                    name: name.clone(),
                    angle: None,
                    velocity: None,
                    timestamp: joint.timestamp,
                };
                
                // Check if angle changed
                if (joint.angle - self_joint.angle).abs() > 0.001 {
                    joint_diff.angle = Some(joint.angle);
                    diff_needed = true;
                }
                
                // Check if velocity changed
                if (joint.velocity - self_joint.velocity).abs() > 0.001 {
                    joint_diff.velocity = Some(joint.velocity);
                    diff_needed = true;
                }
                
                if diff_needed {
                    changes.push(joint_diff);
                }
            } else {
                // New joint
                changes.push(JointDiff {
                    name: name.clone(),
                    angle: Some(joint.angle),
                    velocity: Some(joint.velocity),
                    timestamp: joint.timestamp,
                });
            }
        }
        
        RobotJointDiff {
            changes,
            timestamp: other.last_update,
        }
    }
    
    fn apply(&self, diff: &Self::Diff) -> Self {
        let mut new_state = self.clone();
        new_state.last_update = diff.timestamp;
        
        for change in &diff.changes {
            let joint = new_state.joints.entry(change.name.clone())
                .or_insert_with(|| JointPosition {
                    name: change.name.clone(),
                    angle: 0.0,
                    velocity: 0.0,
                    timestamp: 0,
                });
            
            if let Some(angle) = change.angle {
                joint.angle = angle;
            }
            
            if let Some(velocity) = change.velocity {
                joint.velocity = velocity;
            }
            
            joint.timestamp = change.timestamp;
        }
        
        new_state
    }
}

/// Create a demo robot joint flow
pub fn create_robot_joint_flow(robot_id: &str, owner_id: Uuid) -> DataChannel<RobotJointState> {
    // Create initial state with some joints
    let mut initial_joints = HashMap::new();
    
    // Add some initial joints
    initial_joints.insert("shoulder".to_string(), JointPosition {
        name: "shoulder".to_string(),
        angle: 0.0,
        velocity: 0.0,
        timestamp: 0,
    });
    
    initial_joints.insert("elbow".to_string(), JointPosition {
        name: "elbow".to_string(),
        angle: 0.0,
        velocity: 0.0,
        timestamp: 0,
    });
    
    initial_joints.insert("wrist".to_string(), JointPosition {
        name: "wrist".to_string(),
        angle: 0.0,
        velocity: 0.0,
        timestamp: 0,
    });
    
    let initial_state = RobotJointState {
        robot_id: robot_id.to_string(),
        joints: initial_joints,
        last_update: 0,
    };
    
    // Create the flow
    DataChannel::new(
        &format!("Robot Joints - {}", robot_id),
        owner_id,
        initial_state,
        FlowType::RobotState,
    )
}

/// Demo function showing how to use the robot joint flow with diffs
pub fn run_robot_joint_demo() {
    let owner_id = Uuid::new_v4();
    let flow = create_robot_joint_flow("robot-1", owner_id);
    
    // Subscribe to updates
    let _subscription_id = flow.subscribe(Box::new(|state| {
        println!("Robot state updated:");
        println!("  Robot ID: {}", state.robot_id);
        println!("  Last update: {}", state.last_update);
        println!("  Joints:");
        for (name, joint) in &state.joints {
            println!("    {}: angle={:.2}, velocity={:.2}", 
                     name, joint.angle, joint.velocity);
        }
        println!();
    }));
    
    // Update using full state
    if let Some(current_state) = flow.get_value() {
        let mut new_state = current_state.clone();
        
        // Update shoulder joint
        if let Some(joint) = new_state.joints.get_mut("shoulder") {
            joint.angle = 0.5;
            joint.velocity = 0.1;
            joint.timestamp = 1;
        }
        
        new_state.last_update = 1;
        flow.update(new_state);
    }
    
    // Update using diff (more efficient)
    let diff = RobotJointDiff {
        changes: vec![
            JointDiff {
                name: "elbow".to_string(),
                angle: Some(0.75),
                velocity: Some(0.2),
                timestamp: 2,
            },
            JointDiff {
                name: "wrist".to_string(),
                angle: Some(0.3),
                velocity: None, // Don't change velocity
                timestamp: 2,
            },
        ],
        timestamp: 2,
    };
    
    flow.update_with_diff(&diff);
    
    // Create a new joint with diff
    let new_joint_diff = RobotJointDiff {
        changes: vec![
            JointDiff {
                name: "gripper".to_string(),
                angle: Some(0.0),
                velocity: Some(0.0),
                timestamp: 3,
            },
        ],
        timestamp: 3,
    };
    
    flow.update_with_diff(&new_joint_diff);
    
    // Update multiple joints with a single diff
    let multi_diff = RobotJointDiff {
        changes: vec![
            JointDiff {
                name: "shoulder".to_string(),
                angle: Some(0.6),
                velocity: None,
                timestamp: 4,
            },
            JointDiff {
                name: "elbow".to_string(),
                angle: Some(0.8),
                velocity: Some(0.0),
                timestamp: 4,
            },
        ],
        timestamp: 4,
    };
    
    flow.update_with_diff(&multi_diff);
    
    println!("Robot joint demo completed!");
}
