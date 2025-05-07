//! Robot Joints Example
//!
//! This example demonstrates using the Diffable trait for efficient updates
//! to robot joint positions.

use uuid::Uuid;
use soradyne::flow::{self, SelfDataFlow, FlowType, Diffable};
use soradyne::flow::examples::robot_joints::{
    RobotJointState, RobotJointDiff, JointDiff, JointPosition, create_robot_joint_flow
};

fn main() {
    println!("Starting Robot Joints Demo");
    
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
