//! Erasure coding for fault tolerance

use crate::flow::FlowError;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ErasureEncoder {
    threshold: usize,
    total_shards: usize,
    reed_solomon: ReedSolomon,
}

impl ErasureEncoder {
    pub fn new(threshold: usize, total_shards: usize) -> Result<Self, FlowError> {
        // Validate parameters
        if threshold == 0 || total_shards == 0 {
            return Err(FlowError::PersistenceError(
                "Threshold and total_shards must be greater than 0".to_string()
            ));
        }
        
        if threshold > total_shards {
            return Err(FlowError::PersistenceError(
                "Threshold cannot be greater than total_shards".to_string()
            ));
        }
        
        let parity_shards = total_shards - threshold;
        
        // Create Reed-Solomon encoder
        let reed_solomon = ReedSolomon::new(threshold, parity_shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Failed to create Reed-Solomon encoder: {}", e)
            ))?;
        
        Ok(Self {
            threshold,
            total_shards,
            reed_solomon,
        })
    }
    
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Vec<u8>>, FlowError> {
        if data.is_empty() {
            return Ok(vec![Vec::new(); self.total_shards]);
        }
        
        // Calculate shard size - data is split into threshold parts
        let shard_size = (data.len() + self.threshold - 1) / self.threshold; // Ceiling division
        let padded_size = shard_size * self.threshold;
        
        // Pad data to make it divisible by threshold
        let mut padded_data = data.to_vec();
        padded_data.resize(padded_size, 0);
        
        // Split data into data shards
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(self.total_shards);
        
        // Create data shards
        for i in 0..self.threshold {
            let start = i * shard_size;
            let end = start + shard_size;
            shards.push(padded_data[start..end].to_vec());
        }
        
        // Create empty parity shards
        let parity_count = self.total_shards - self.threshold;
        for _ in 0..parity_count {
            shards.push(vec![0u8; shard_size]);
        }
        
        // Generate parity shards using Reed-Solomon
        self.reed_solomon.encode(&mut shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon encoding failed: {}", e)
            ))?;
        
        Ok(shards)
    }
    
    pub fn decode(&self, shards: HashMap<usize, Vec<u8>>, expected_size: usize) -> Result<Vec<u8>, FlowError> {
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        // Determine shard size from any available shard
        let shard_size = shards.values().next()
            .ok_or_else(|| FlowError::PersistenceError("No shards available".to_string()))?
            .len();
        
        // Reconstruct the full shard array with None for missing shards
        let mut reconstruction_shards: Vec<Option<Vec<u8>>> = vec![None; self.total_shards];
        
        for (index, shard_data) in shards {
            if index >= self.total_shards {
                return Err(FlowError::PersistenceError(
                    format!("Invalid shard index: {} >= {}", index, self.total_shards)
                ));
            }
            
            if shard_data.len() != shard_size {
                return Err(FlowError::PersistenceError(
                    format!("Shard size mismatch: expected {}, got {}", shard_size, shard_data.len())
                ));
            }
            
            reconstruction_shards[index] = Some(shard_data);
        }
        
        // Use Reed-Solomon to reconstruct missing shards
        let mut shards_for_reconstruction: Vec<Vec<u8>> = Vec::with_capacity(self.total_shards);
        for maybe_shard in reconstruction_shards {
            match maybe_shard {
                Some(shard) => shards_for_reconstruction.push(shard),
                None => shards_for_reconstruction.push(vec![0u8; shard_size]), // Placeholder for missing shard
            }
        }
        
        // Perform reconstruction
        self.reed_solomon.reconstruct(&mut shards_for_reconstruction)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon reconstruction failed: {}", e)
            ))?;
        
        // Concatenate the data shards (first threshold shards)
        let mut result = Vec::with_capacity(expected_size);
        for i in 0..self.threshold {
            result.extend_from_slice(&shards_for_reconstruction[i]);
        }
        
        // Truncate to actual data size (remove padding)
        result.truncate(expected_size);
        
        Ok(result)
    }
    
    /// Calculate storage overhead factor
    pub fn storage_overhead(&self) -> f64 {
        self.total_shards as f64 / self.threshold as f64
    }
    
    /// Calculate how many shards can be lost while still being able to reconstruct
    pub fn fault_tolerance(&self) -> usize {
        self.total_shards - self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_basic() {
        let encoder = ErasureEncoder::new(3, 5).unwrap();
        let original_data = b"Hello, erasure coding world! This is a test.";
        
        // Encode
        let shards = encoder.encode(original_data).unwrap();
        assert_eq!(shards.len(), 5);
        
        // Verify all shards are the same size
        let shard_size = shards[0].len();
        for shard in &shards {
            assert_eq!(shard.len(), shard_size);
        }
        
        // Decode with all shards
        let mut shard_map = HashMap::new();
        for (i, shard) in shards.iter().enumerate() {
            shard_map.insert(i, shard.clone());
        }
        
        let decoded = encoder.decode(shard_map, original_data.len()).unwrap();
        assert_eq!(decoded, original_data);
    }
    
    #[test]
    fn test_fault_tolerance() {
        let encoder = ErasureEncoder::new(3, 5).unwrap();
        let original_data = b"This is fault tolerance testing!";
        
        let shards = encoder.encode(original_data).unwrap();
        
        // Test reconstruction with exactly threshold shards (lose 2 shards)
        let mut shard_map = HashMap::new();
        shard_map.insert(0, shards[0].clone());
        shard_map.insert(2, shards[2].clone());
        shard_map.insert(4, shards[4].clone());
        
        let decoded = encoder.decode(shard_map, original_data.len()).unwrap();
        assert_eq!(decoded, original_data);
        
        // Test with more than threshold shards
        let mut shard_map = HashMap::new();
        shard_map.insert(1, shards[1].clone());
        shard_map.insert(2, shards[2].clone());
        shard_map.insert(3, shards[3].clone());
        shard_map.insert(4, shards[4].clone());
        
        let decoded = encoder.decode(shard_map, original_data.len()).unwrap();
        assert_eq!(decoded, original_data);
    }
    
    #[test]
    fn test_insufficient_shards() {
        let encoder = ErasureEncoder::new(3, 5).unwrap();
        let original_data = b"Testing insufficient shards";
        
        let shards = encoder.encode(original_data).unwrap();
        
        // Try with only 2 shards (less than threshold of 3)
        let mut shard_map = HashMap::new();
        shard_map.insert(0, shards[0].clone());
        shard_map.insert(1, shards[1].clone());
        
        let result = encoder.decode(shard_map, original_data.len());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_storage_efficiency() {
        let encoder = ErasureEncoder::new(3, 5).unwrap();
        assert_eq!(encoder.storage_overhead(), 5.0 / 3.0); // ~1.67x instead of 5x
        assert_eq!(encoder.fault_tolerance(), 2); // Can lose 2 shards
        
        let encoder_strict = ErasureEncoder::new(4, 6).unwrap();
        assert_eq!(encoder_strict.storage_overhead(), 1.5); // 6/4 = 1.5x
        assert_eq!(encoder_strict.fault_tolerance(), 2); // Can lose 2 shards
    }
    
    #[test]
    fn test_empty_data() {
        let encoder = ErasureEncoder::new(3, 5).unwrap();
        let shards = encoder.encode(&[]).unwrap();
        assert_eq!(shards.len(), 5);
        
        let mut shard_map = HashMap::new();
        for (i, shard) in shards.iter().enumerate() {
            shard_map.insert(i, shard.clone());
        }
        
        let decoded = encoder.decode(shard_map, 0).unwrap();
        assert!(decoded.is_empty());
    }
}
