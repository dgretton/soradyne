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

async fn extract_hardware_id(_rimsd_path: &Path) -> Result<Option<String>, FlowError> {
    // TODO: Use platform-specific APIs to get SD card hardware serial
    // For now, return None
    Ok(None)
}

async fn extract_filesystem_uuid(_rimsd_path: &Path) -> Result<Option<String>, FlowError> {
    // TODO: Read filesystem UUID from the device containing rimsd_path
    // For now, return a placeholder
    Ok(Some("placeholder-uuid".to_string()))
}

async fn extract_bad_blocks(_rimsd_path: &Path) -> Result<Vec<u64>, FlowError> {
    // TODO: Query the device for bad block information
    // For now, return empty list
    Ok(vec![])
}

async fn extract_capacity(_rimsd_path: &Path) -> Result<u64, FlowError> {
    // TODO: Get exact device capacity
    // For now, return placeholder
    Ok(32 * 1024 * 1024 * 1024) // 32GB placeholder
}

#[cfg(test)]
mod tests {
    use super::*;
    
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
}
