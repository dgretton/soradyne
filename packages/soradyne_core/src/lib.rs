// Soradyne - Collaborative Media Album System

pub mod album;
pub mod ble;
pub mod convergent;
pub mod ffi;
pub mod flow;
pub mod identity;
pub mod network;
pub mod storage;
pub mod topology;
pub mod types;
pub mod video;

use crate::storage::device_identity::discover_soradyne_volumes;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct StorageStatus {
    pub available_devices: usize,
    pub required_threshold: usize,
    pub can_read_data: bool,
    pub missing_devices: usize,
    pub device_paths: Vec<String>,
}

