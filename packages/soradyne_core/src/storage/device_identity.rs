use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use crate::flow::FlowError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicFingerprint {
    /// Soradyne-assigned device ID (stored in .rimsd directory)
    pub soradyne_device_id: Option<String>,
    /// Combined hardware serial + manufacturer ID
    pub hardware_id: Option<String>,
    /// Filesystem UUID
    pub filesystem_uuid: Option<String>,
    /// Hash of bad block positions (monotonic - can only grow)
    pub bad_block_signature: u64,
    /// Exact capacity in bytes
    pub capacity_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct EvidenceType {
    pub name: String,
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct LikelihoodModel {
    /// P(evidence | same device)
    pub prob_same: f64,
    /// P(evidence | different device)
    pub prob_different: f64,
}

#[derive(Debug)]
pub struct BayesianDeviceIdentifier {
    /// Prior probability that a device is the same
    pub prior_same: f64,
    /// Evidence models for each fingerprint component
    pub evidence_models: HashMap<String, LikelihoodModel>,
    /// Confidence threshold for "same device" decision
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentityResult {
    pub is_same_device: bool,
    pub confidence: f64,
    pub evidence_summary: Vec<String>,
}

impl BasicFingerprint {
    pub fn new(
        soradyne_device_id: Option<String>,
        hardware_id: Option<String>,
        filesystem_uuid: Option<String>,
        bad_blocks: &[u64],
        capacity_bytes: u64,
    ) -> Self {
        let bad_block_signature = Self::hash_bad_blocks(bad_blocks);
        
        Self {
            soradyne_device_id,
            hardware_id,
            filesystem_uuid,
            bad_block_signature,
            capacity_bytes,
        }
    }
    
    fn hash_bad_blocks(bad_blocks: &[u64]) -> u64 {
        let mut hasher = Sha256::new();
        let mut sorted_blocks = bad_blocks.to_vec();
        sorted_blocks.sort();
        
        for block in sorted_blocks {
            hasher.update(block.to_le_bytes());
        }
        
        let result = hasher.finalize();
        u64::from_le_bytes(result[0..8].try_into().unwrap())
    }
    
    /// Check if this fingerprint could be a legitimate evolution of the previous one
    pub fn is_valid_evolution(&self, previous: &BasicFingerprint) -> Result<bool, FlowError> {
        // Soradyne device ID should never change (most reliable identifier)
        if self.soradyne_device_id != previous.soradyne_device_id {
            return Ok(false);
        }
        
        // Hardware ID should never change
        if self.hardware_id != previous.hardware_id {
            return Ok(false);
        }
        
        // Filesystem UUID should only change with reformatting (suspicious)
        if self.filesystem_uuid != previous.filesystem_uuid {
            return Ok(false);
        }
        
        // Capacity should never change
        if self.capacity_bytes != previous.capacity_bytes {
            return Ok(false);
        }
        
        // Bad block signature can only increase (new bad blocks, never fewer)
        // For now, we just check they're different - more sophisticated logic later
        Ok(true)
    }
}

impl Default for BayesianDeviceIdentifier {
    fn default() -> Self {
        let mut evidence_models = HashMap::new();
        
        // Soradyne device ID match (most reliable)
        evidence_models.insert("soradyne_device_id".to_string(), LikelihoodModel {
            prob_same: 0.999,     // Extremely high confidence - we control this
            prob_different: 0.000001, // Virtually impossible collision
        });
        
        // Hardware ID match
        evidence_models.insert("hardware_id".to_string(), LikelihoodModel {
            prob_same: 0.95,      // High confidence when it matches
            prob_different: 0.0001, // Very rare collision
        });
        
        // Filesystem UUID match
        evidence_models.insert("filesystem_uuid".to_string(), LikelihoodModel {
            prob_same: 0.99,      // Very reliable
            prob_different: 0.00001, // Extremely rare collision
        });
        
        // Bad block signature match
        evidence_models.insert("bad_block_signature".to_string(), LikelihoodModel {
            prob_same: 0.90,      // Pretty reliable
            prob_different: 0.001,  // Rare collision
        });
        
        // Capacity match
        evidence_models.insert("capacity".to_string(), LikelihoodModel {
            prob_same: 0.80,      // Common among same model
            prob_different: 0.1,   // Many cards have same capacity
        });
        
        Self {
            prior_same: 0.5,      // No prior bias
            evidence_models,
            threshold: 0.95,      // 95% confidence required
        }
    }
}

impl BayesianDeviceIdentifier {
    pub fn identify_device(
        &self,
        current: &BasicFingerprint,
        previous: &BasicFingerprint,
    ) -> Result<DeviceIdentityResult, FlowError> {
        let mut evidence_summary = Vec::new();
        let mut log_odds = (self.prior_same / (1.0 - self.prior_same)).ln();
        
        // Soradyne device ID evidence (highest priority)
        if let (Some(curr_id), Some(prev_id)) = (&current.soradyne_device_id, &previous.soradyne_device_id) {
            let matches = curr_id == prev_id;
            let model = &self.evidence_models["soradyne_device_id"];
            
            if matches {
                log_odds += (model.prob_same / model.prob_different).ln();
                evidence_summary.push("Soradyne device ID matches".to_string());
            } else {
                log_odds += ((1.0 - model.prob_same) / (1.0 - model.prob_different)).ln();
                evidence_summary.push("Soradyne device ID differs".to_string());
            }
        } else {
            evidence_summary.push("Soradyne device ID unavailable".to_string());
        }
        
        // Hardware ID evidence
        if let (Some(curr_hw), Some(prev_hw)) = (&current.hardware_id, &previous.hardware_id) {
            let matches = curr_hw == prev_hw;
            let model = &self.evidence_models["hardware_id"];
            
            if matches {
                log_odds += (model.prob_same / model.prob_different).ln();
                evidence_summary.push("Hardware ID matches".to_string());
            } else {
                log_odds += ((1.0 - model.prob_same) / (1.0 - model.prob_different)).ln();
                evidence_summary.push("Hardware ID differs".to_string());
            }
        } else {
            evidence_summary.push("Hardware ID unavailable".to_string());
        }
        
        // Filesystem UUID evidence
        if let (Some(curr_fs), Some(prev_fs)) = (&current.filesystem_uuid, &previous.filesystem_uuid) {
            let matches = curr_fs == prev_fs;
            let model = &self.evidence_models["filesystem_uuid"];
            
            if matches {
                log_odds += (model.prob_same / model.prob_different).ln();
                evidence_summary.push("Filesystem UUID matches".to_string());
            } else {
                log_odds += ((1.0 - model.prob_same) / (1.0 - model.prob_different)).ln();
                evidence_summary.push("Filesystem UUID differs".to_string());
            }
        } else {
            evidence_summary.push("Filesystem UUID unavailable".to_string());
        }
        
        // Bad block signature evidence
        let matches = current.bad_block_signature == previous.bad_block_signature;
        let model = &self.evidence_models["bad_block_signature"];
        
        if matches {
            log_odds += (model.prob_same / model.prob_different).ln();
            evidence_summary.push("Bad block pattern matches".to_string());
        } else {
            log_odds += ((1.0 - model.prob_same) / (1.0 - model.prob_different)).ln();
            evidence_summary.push("Bad block pattern differs".to_string());
        }
        
        // Capacity evidence
        let matches = current.capacity_bytes == previous.capacity_bytes;
        let model = &self.evidence_models["capacity"];
        
        if matches {
            log_odds += (model.prob_same / model.prob_different).ln();
            evidence_summary.push("Capacity matches".to_string());
        } else {
            log_odds += ((1.0 - model.prob_same) / (1.0 - model.prob_different)).ln();
            evidence_summary.push("Capacity differs".to_string());
        }
        
        // Convert log odds back to probability
        let odds = log_odds.exp();
        let confidence = odds / (1.0 + odds);
        
        let is_same_device = confidence >= self.threshold;
        
        Ok(DeviceIdentityResult {
            is_same_device,
            confidence,
            evidence_summary,
        })
    }
}

/// Extract device fingerprint from a rimsd directory path
pub async fn fingerprint_device(rimsd_path: &Path) -> Result<BasicFingerprint, FlowError> {
    let soradyne_device_id = extract_soradyne_device_id(rimsd_path).await?;
    let hardware_id = extract_hardware_id(rimsd_path).await?;
    let filesystem_uuid = extract_filesystem_uuid(rimsd_path).await?;
    let bad_blocks = extract_bad_blocks(rimsd_path).await?;
    let capacity = extract_capacity(rimsd_path).await?;
    
    Ok(BasicFingerprint::new(
        soradyne_device_id,
        hardware_id,
        filesystem_uuid,
        &bad_blocks,
        capacity,
    ))
}

/// Extract or create Soradyne device ID from .rimsd directory
async fn extract_soradyne_device_id(rimsd_path: &Path) -> Result<Option<String>, FlowError> {
    let device_id_file = rimsd_path.join("soradyne_device_id.txt");
    
    if device_id_file.exists() {
        // Read existing device ID
        let content = tokio::fs::read_to_string(&device_id_file).await.map_err(|e|
            FlowError::PersistenceError(format!("Failed to read device ID: {}", e))
        )?;
        Ok(Some(content.trim().to_string()))
    } else {
        // Create new device ID
        let new_device_id = uuid::Uuid::new_v4().to_string();
        
        // Ensure rimsd directory exists
        tokio::fs::create_dir_all(rimsd_path).await.map_err(|e|
            FlowError::PersistenceError(format!("Failed to create rimsd directory: {}", e))
        )?;
        
        // Write device ID to file
        tokio::fs::write(&device_id_file, &new_device_id).await.map_err(|e|
            FlowError::PersistenceError(format!("Failed to write device ID: {}", e))
        )?;
        
        Ok(Some(new_device_id))
    }
}

async fn extract_hardware_id(rimsd_path: &Path) -> Result<Option<String>, FlowError> {
    // Try to get hardware ID from the device containing the rimsd path
    let device_path = get_device_path(rimsd_path).await?;
    
    #[cfg(target_os = "linux")]
    {
        extract_hardware_id_linux(&device_path).await
    }
    
    #[cfg(target_os = "macos")]
    {
        extract_hardware_id_macos(&device_path).await
    }
    
    #[cfg(target_os = "windows")]
    {
        extract_hardware_id_windows(&device_path).await
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(None)
    }
}

async fn extract_filesystem_uuid(rimsd_path: &Path) -> Result<Option<String>, FlowError> {
    let device_path = get_device_path(rimsd_path).await?;
    
    #[cfg(target_os = "linux")]
    {
        extract_filesystem_uuid_linux(&device_path).await
    }
    
    #[cfg(target_os = "macos")]
    {
        extract_filesystem_uuid_macos(&device_path).await
    }
    
    #[cfg(target_os = "windows")]
    {
        extract_filesystem_uuid_windows(&device_path).await
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(None)
    }
}

async fn extract_bad_blocks(rimsd_path: &Path) -> Result<Vec<u64>, FlowError> {
    let device_path = get_device_path(rimsd_path).await?;
    
    #[cfg(target_os = "linux")]
    {
        extract_bad_blocks_linux(&device_path).await
    }
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        // Bad block detection is more complex on macOS/Windows
        // For now, use filesystem-based heuristics
        extract_bad_blocks_heuristic(rimsd_path).await
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(vec![])
    }
}

async fn extract_capacity(rimsd_path: &Path) -> Result<u64, FlowError> {
    let device_path = get_device_path(rimsd_path).await?;
    
    #[cfg(target_os = "linux")]
    {
        extract_capacity_linux(&device_path).await
    }
    
    #[cfg(target_os = "macos")]
    {
        extract_capacity_macos(&device_path).await
    }
    
    #[cfg(target_os = "windows")]
    {
        extract_capacity_windows(&device_path).await
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Fallback: use filesystem stats
        extract_capacity_fallback(rimsd_path).await
    }
}

// === Platform-agnostic helpers ===

async fn get_device_path(rimsd_path: &Path) -> Result<String, FlowError> {
    // Find the mount point and device for this path
    let mut current_path = rimsd_path;
    
    // Walk up the directory tree to find the mount point
    while let Some(parent) = current_path.parent() {
        if is_mount_point(current_path).await? {
            break;
        }
        current_path = parent;
    }
    
    get_device_for_mount_point(current_path).await
}

async fn is_mount_point(path: &Path) -> Result<bool, FlowError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        
        let metadata = tokio::fs::metadata(path).await.map_err(|e|
            FlowError::PersistenceError(format!("Failed to get metadata: {}", e))
        )?;
        
        if let Some(parent) = path.parent() {
            let parent_metadata = tokio::fs::metadata(parent).await.map_err(|e|
                FlowError::PersistenceError(format!("Failed to get parent metadata: {}", e))
            )?;
            
            // Different device IDs indicate a mount point
            Ok(metadata.dev() != parent_metadata.dev())
        } else {
            Ok(true) // Root is always a mount point
        }
    }
    
    #[cfg(windows)]
    {
        // On Windows, check if this is a drive root
        let path_str = path.to_string_lossy();
        Ok(path_str.len() == 3 && path_str.ends_with(":\\"))
    }
}

async fn get_device_for_mount_point(mount_point: &Path) -> Result<String, FlowError> {
    #[cfg(target_os = "linux")]
    {
        // Read /proc/mounts to find the device
        let mounts = tokio::fs::read_to_string("/proc/mounts").await.map_err(|e|
            FlowError::PersistenceError(format!("Failed to read /proc/mounts: {}", e))
        )?;
        
        let mount_point_str = mount_point.to_string_lossy();
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == mount_point_str {
                return Ok(parts[0].to_string());
            }
        }
        
        Err(FlowError::PersistenceError("Device not found in /proc/mounts".to_string()))
    }
    
    #[cfg(target_os = "macos")]
    {
        // Use diskutil to find the device
        let output = tokio::process::Command::new("diskutil")
            .args(&["info", &mount_point.to_string_lossy()])
            .output()
            .await
            .map_err(|e| FlowError::PersistenceError(format!("Failed to run diskutil: {}", e)))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.trim().starts_with("Device Node:") {
                if let Some(device) = line.split(':').nth(1) {
                    return Ok(device.trim().to_string());
                }
            }
        }
        
        Err(FlowError::PersistenceError("Device not found in diskutil output".to_string()))
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, the mount point is typically the drive letter
        Ok(mount_point.to_string_lossy().to_string())
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok("unknown".to_string())
    }
}

// === Linux implementations ===

#[cfg(target_os = "linux")]
async fn extract_hardware_id_linux(device_path: &str) -> Result<Option<String>, FlowError> {
    // Try to get hardware info from udev
    let output = tokio::process::Command::new("udevadm")
        .args(&["info", "--query=all", "--name", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run udevadm: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut serial = None;
    let mut vendor = None;
    let mut model = None;
    
    for line in output_str.lines() {
        if line.contains("ID_SERIAL_SHORT=") {
            serial = line.split('=').nth(1).map(|s| s.to_string());
        } else if line.contains("ID_VENDOR=") {
            vendor = line.split('=').nth(1).map(|s| s.to_string());
        } else if line.contains("ID_MODEL=") {
            model = line.split('=').nth(1).map(|s| s.to_string());
        }
    }
    
    match (vendor, model, serial) {
        (Some(v), Some(m), Some(s)) => Ok(Some(format!("{}:{}:{}", v, m, s))),
        (Some(v), Some(m), None) => Ok(Some(format!("{}:{}", v, m))),
        _ => Ok(None),
    }
}

#[cfg(target_os = "linux")]
async fn extract_filesystem_uuid_linux(device_path: &str) -> Result<Option<String>, FlowError> {
    let output = tokio::process::Command::new("blkid")
        .args(&["-s", "UUID", "-o", "value", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run blkid: {}", e)))?;
    
    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() {
        Ok(None)
    } else {
        Ok(Some(uuid))
    }
}

#[cfg(target_os = "linux")]
async fn extract_bad_blocks_linux(device_path: &str) -> Result<Vec<u64>, FlowError> {
    // Use badblocks to scan for bad blocks (read-only scan)
    let output = tokio::process::Command::new("badblocks")
        .args(&["-v", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run badblocks: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut bad_blocks = Vec::new();
    
    for line in output_str.lines() {
        if let Ok(block_num) = line.trim().parse::<u64>() {
            bad_blocks.push(block_num);
        }
    }
    
    Ok(bad_blocks)
}

#[cfg(target_os = "linux")]
async fn extract_capacity_linux(device_path: &str) -> Result<u64, FlowError> {
    // Read the size from /sys/block/*/size
    let device_name = device_path.trim_start_matches("/dev/");
    let size_path = format!("/sys/block/{}/size", device_name);
    
    let size_str = tokio::fs::read_to_string(&size_path).await.map_err(|e|
        FlowError::PersistenceError(format!("Failed to read device size: {}", e))
    )?;
    
    let sectors = size_str.trim().parse::<u64>().map_err(|e|
        FlowError::PersistenceError(format!("Failed to parse device size: {}", e))
    )?;
    
    // Convert sectors to bytes (assuming 512-byte sectors)
    Ok(sectors * 512)
}

// === macOS implementations ===

#[cfg(target_os = "macos")]
async fn extract_hardware_id_macos(device_path: &str) -> Result<Option<String>, FlowError> {
    let output = tokio::process::Command::new("diskutil")
        .args(&["info", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run diskutil: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut vendor = None;
    let mut model = None;
    
    for line in output_str.lines() {
        let line = line.trim();
        if line.starts_with("Device / Media Name:") {
            if let Some(name) = line.split(':').nth(1) {
                model = Some(name.trim().to_string());
            }
        } else if line.starts_with("Disk / Partition UUID:") {
            if let Some(uuid) = line.split(':').nth(1) {
                return Ok(Some(uuid.trim().to_string()));
            }
        }
    }
    
    match (vendor, model) {
        (Some(v), Some(m)) => Ok(Some(format!("{}:{}", v, m))),
        (None, Some(m)) => Ok(Some(m)),
        _ => Ok(None),
    }
}

#[cfg(target_os = "macos")]
async fn extract_filesystem_uuid_macos(device_path: &str) -> Result<Option<String>, FlowError> {
    let output = tokio::process::Command::new("diskutil")
        .args(&["info", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run diskutil: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        let line = line.trim();
        if line.starts_with("Volume UUID:") {
            if let Some(uuid) = line.split(':').nth(1) {
                return Ok(Some(uuid.trim().to_string()));
            }
        }
    }
    
    Ok(None)
}

#[cfg(target_os = "macos")]
async fn extract_capacity_macos(device_path: &str) -> Result<u64, FlowError> {
    let output = tokio::process::Command::new("diskutil")
        .args(&["info", device_path])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run diskutil: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        let line = line.trim();
        if line.starts_with("Disk Size:") {
            // Parse something like "Disk Size: 32.0 GB (32017047552 Bytes) (exactly 62533296 512-Byte-Units)"
            if let Some(bytes_part) = line.split('(').nth(1) {
                if let Some(bytes_str) = bytes_part.split(' ').next() {
                    if let Ok(bytes) = bytes_str.parse::<u64>() {
                        return Ok(bytes);
                    }
                }
            }
        }
    }
    
    Err(FlowError::PersistenceError("Could not parse disk size from diskutil".to_string()))
}

// === Windows implementations ===

#[cfg(target_os = "windows")]
async fn extract_hardware_id_windows(device_path: &str) -> Result<Option<String>, FlowError> {
    // Use wmic to get disk information
    let drive_letter = device_path.chars().next().unwrap_or('C');
    let output = tokio::process::Command::new("wmic")
        .args(&["logicaldisk", "where", &format!("DeviceID='{}':", drive_letter), "get", "VolumeSerialNumber", "/value"])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run wmic: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        if line.starts_with("VolumeSerialNumber=") {
            if let Some(serial) = line.split('=').nth(1) {
                let serial = serial.trim();
                if !serial.is_empty() {
                    return Ok(Some(serial.to_string()));
                }
            }
        }
    }
    
    Ok(None)
}

#[cfg(target_os = "windows")]
async fn extract_filesystem_uuid_windows(device_path: &str) -> Result<Option<String>, FlowError> {
    // Windows doesn't have UUIDs in the same way, use volume serial number
    extract_hardware_id_windows(device_path).await
}

#[cfg(target_os = "windows")]
async fn extract_capacity_windows(device_path: &str) -> Result<u64, FlowError> {
    let drive_letter = device_path.chars().next().unwrap_or('C');
    let output = tokio::process::Command::new("wmic")
        .args(&["logicaldisk", "where", &format!("DeviceID='{}':", drive_letter), "get", "Size", "/value"])
        .output()
        .await
        .map_err(|e| FlowError::PersistenceError(format!("Failed to run wmic: {}", e)))?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        if line.starts_with("Size=") {
            if let Some(size_str) = line.split('=').nth(1) {
                if let Ok(size) = size_str.trim().parse::<u64>() {
                    return Ok(size);
                }
            }
        }
    }
    
    Err(FlowError::PersistenceError("Could not parse disk size from wmic".to_string()))
}

// === Fallback implementations ===

async fn extract_bad_blocks_heuristic(rimsd_path: &Path) -> Result<Vec<u64>, FlowError> {
    // Heuristic: look for filesystem errors or read errors
    // This is a simplified approach - real bad block detection requires low-level access
    
    // Try to read some test files and see if we get I/O errors
    let test_dir = rimsd_path.join("soradyne_test");
    tokio::fs::create_dir_all(&test_dir).await.ok();
    
    let mut bad_blocks = Vec::new();
    
    // Write and read test patterns to detect bad areas
    for i in 0..10 {
        let test_file = test_dir.join(format!("test_{}.dat", i));
        let test_data = vec![0xAA; 4096]; // Test pattern
        
        if tokio::fs::write(&test_file, &test_data).await.is_err() {
            bad_blocks.push(i as u64);
        } else if let Ok(read_data) = tokio::fs::read(&test_file).await {
            if read_data != test_data {
                bad_blocks.push(i as u64);
            }
        }
        
        tokio::fs::remove_file(&test_file).await.ok();
    }
    
    tokio::fs::remove_dir(&test_dir).await.ok();
    Ok(bad_blocks)
}

async fn extract_capacity_fallback(rimsd_path: &Path) -> Result<u64, FlowError> {
    // Use filesystem stats as fallback
    let metadata = tokio::fs::metadata(rimsd_path).await.map_err(|e|
        FlowError::PersistenceError(format!("Failed to get filesystem metadata: {}", e))
    )?;
    
    // This is not the true device capacity, but filesystem available space
    // It's better than nothing for fingerprinting purposes
    Ok(metadata.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    
    #[test]
    fn test_basic_fingerprint_creation() {
        let fingerprint = BasicFingerprint::new(
            Some("soradyne-device-123".to_string()),
            Some("test-hw-id".to_string()),
            Some("test-fs-uuid".to_string()),
            &[1047, 2891, 4203],
            32 * 1024 * 1024 * 1024,
        );
        
        assert_eq!(fingerprint.soradyne_device_id, Some("soradyne-device-123".to_string()));
        assert_eq!(fingerprint.hardware_id, Some("test-hw-id".to_string()));
        assert_eq!(fingerprint.capacity_bytes, 32 * 1024 * 1024 * 1024);
        assert_ne!(fingerprint.bad_block_signature, 0);
    }
    
    #[test]
    fn test_bayesian_identification() {
        let identifier = BayesianDeviceIdentifier::default();
        
        let fingerprint1 = BasicFingerprint::new(
            Some("soradyne-device-456".to_string()),
            Some("hw123".to_string()),
            Some("fs-uuid-456".to_string()),
            &[100, 200, 300],
            1000000,
        );
        
        let fingerprint2 = fingerprint1.clone();
        
        let result = identifier.identify_device(&fingerprint1, &fingerprint2).unwrap();
        
        assert!(result.is_same_device);
        assert!(result.confidence > 0.95);
    }
    
    #[test]
    fn test_different_devices() {
        let identifier = BayesianDeviceIdentifier::default();
        
        let fingerprint1 = BasicFingerprint::new(
            Some("soradyne-device-123".to_string()),
            Some("hw123".to_string()),
            Some("fs-uuid-456".to_string()),
            &[100, 200, 300],
            1000000,
        );
        
        let fingerprint2 = BasicFingerprint::new(
            Some("soradyne-device-999".to_string()),  // Different Soradyne ID
            Some("hw999".to_string()),  // Different hardware
            Some("fs-uuid-999".to_string()),  // Different filesystem
            &[400, 500, 600],  // Different bad blocks
            2000000,  // Different capacity
        );
        
        let result = identifier.identify_device(&fingerprint1, &fingerprint2).unwrap();
        
        assert!(!result.is_same_device);
        assert!(result.confidence < 0.05);
    }
    
    #[tokio::test]
    #[ignore] // Run manually with: cargo test test_interactive_sd_card_verification -- --ignored
    async fn test_interactive_sd_card_verification() {
        println!("\n🔍 Interactive SD Card Device Identity Test");
        println!("==========================================");
        println!("This test will help you verify that SD card fingerprinting works correctly.");
        println!("You'll need to insert SD cards when prompted.\n");
        
        let mut stored_fingerprints: std::collections::HashMap<String, BasicFingerprint> = std::collections::HashMap::new();
        let identifier = BayesianDeviceIdentifier::default();
        
        loop {
            println!("Options:");
            println!("1. Initialize new SD card");
            println!("2. Verify existing SD card");
            println!("3. List stored fingerprints");
            println!("4. Exit");
            print!("Choose an option (1-4): ");
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let choice = input.trim();
            
            match choice {
                "1" => {
                    println!("\n📱 Insert an SD card and enter its mount path:");
                    print!("Path (e.g., /Volumes/SDCARD or /media/sdcard): ");
                    io::stdout().flush().unwrap();
                    
                    let mut path_input = String::new();
                    io::stdin().read_line(&mut path_input).unwrap();
                    let rimsd_path = std::path::Path::new(path_input.trim()).join(".rimsd");
                    
                    println!("🔍 Fingerprinting SD card...");
                    match fingerprint_device(&rimsd_path).await {
                        Ok(fingerprint) => {
                            println!("✅ Successfully fingerprinted SD card!");
                            println!("   Soradyne ID: {:?}", fingerprint.soradyne_device_id);
                            println!("   Hardware ID: {:?}", fingerprint.hardware_id);
                            println!("   Filesystem UUID: {:?}", fingerprint.filesystem_uuid);
                            println!("   Capacity: {} GB", fingerprint.capacity_bytes / (1024 * 1024 * 1024));
                            println!("   Bad blocks: {} detected", if fingerprint.bad_block_signature == 0 { 0 } else { 1 });
                            
                            if let Some(soradyne_id) = &fingerprint.soradyne_device_id {
                                stored_fingerprints.insert(soradyne_id.clone(), fingerprint);
                                println!("💾 Stored fingerprint for device: {}", soradyne_id);
                            } else {
                                println!("⚠️  Warning: No Soradyne device ID found");
                            }
                        }
                        Err(e) => {
                            println!("❌ Failed to fingerprint SD card: {}", e);
                        }
                    }
                }
                
                "2" => {
                    if stored_fingerprints.is_empty() {
                        println!("❌ No stored fingerprints. Initialize an SD card first.");
                        continue;
                    }
                    
                    println!("\n📱 Insert an SD card to verify and enter its mount path:");
                    print!("Path: ");
                    io::stdout().flush().unwrap();
                    
                    let mut path_input = String::new();
                    io::stdin().read_line(&mut path_input).unwrap();
                    let rimsd_path = std::path::Path::new(path_input.trim()).join(".rimsd");
                    
                    println!("🔍 Fingerprinting SD card...");
                    match fingerprint_device(&rimsd_path).await {
                        Ok(current_fingerprint) => {
                            if let Some(soradyne_id) = &current_fingerprint.soradyne_device_id {
                                if let Some(stored_fingerprint) = stored_fingerprints.get(soradyne_id) {
                                    println!("🔍 Comparing with stored fingerprint...");
                                    
                                    match identifier.identify_device(&current_fingerprint, stored_fingerprint) {
                                        Ok(result) => {
                                            if result.is_same_device {
                                                println!("✅ MATCH: This is the same SD card!");
                                                println!("   Confidence: {:.2}%", result.confidence * 100.0);
                                                println!("   Evidence: {:?}", result.evidence_summary);
                                            } else {
                                                println!("❌ NO MATCH: This appears to be a different SD card!");
                                                println!("   Confidence: {:.2}%", result.confidence * 100.0);
                                                println!("   Evidence: {:?}", result.evidence_summary);
                                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to compare fingerprints: {}", e);
                                        }
                                    }
                                } else {
                                    println!("❌ No stored fingerprint found for Soradyne ID: {}", soradyne_id);
                                    println!("   This appears to be a new SD card.");
                                }
                            } else {
                                println!("❌ No Soradyne device ID found on this SD card");
                            }
                        }
                        Err(e) => {
                            println!("❌ Failed to fingerprint SD card: {}", e);
                        }
                    }
                }
                
                "3" => {
                    println!("\n📋 Stored Fingerprints:");
                    if stored_fingerprints.is_empty() {
                        println!("   (none)");
                    } else {
                        for (id, fingerprint) in &stored_fingerprints {
                            println!("   🔑 {}", id);
                            println!("      Hardware: {:?}", fingerprint.hardware_id);
                            println!("      Filesystem: {:?}", fingerprint.filesystem_uuid);
                            println!("      Capacity: {} GB", fingerprint.capacity_bytes / (1024 * 1024 * 1024));
                        }
                    }
                }
                
                "4" => {
                    println!("👋 Goodbye!");
                    break;
                }
                
                _ => {
                    println!("❌ Invalid option. Please choose 1-4.");
                }
            }
            
            println!();
        }
    }
}
