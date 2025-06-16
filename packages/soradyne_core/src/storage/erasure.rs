//! Erasure coding for fault tolerance

use crate::flow::FlowError;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ErasureEncoder {
    threshold: usize,
    total_shards: usize,
}

impl ErasureEncoder {
    pub fn new(threshold: usize, total_shards: usize) -> Self {
        Self {
            threshold,
            total_shards,
        }
    }
    
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Vec<u8>>, FlowError> {
        // Simple replication for now (not true erasure coding)
        // In a real implementation, you'd use Reed-Solomon codes
        let mut shards = Vec::new();
        
        for _ in 0..self.total_shards {
            shards.push(data.to_vec());
        }
        
        Ok(shards)
    }
    
    pub fn decode(&self, shards: HashMap<usize, Vec<u8>>, expected_size: usize) -> Result<Vec<u8>, FlowError> {
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        // With simple replication, just return any shard
        if let Some((_, data)) = shards.iter().next() {
            if data.len() == expected_size {
                Ok(data.clone())
            } else {
                // Truncate to expected size
                Ok(data[..expected_size.min(data.len())].to_vec())
            }
        } else {
            Err(FlowError::PersistenceError("No shards available".to_string()))
        }
    }
}
