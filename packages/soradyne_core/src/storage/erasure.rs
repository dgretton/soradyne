//! Shamir Secret Sharing + Reed-Solomon erasure coding for secure fault tolerance

use crate::flow::FlowError;
use crate::storage::block::{CHUNK_SIZE, BlockId};
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::HashMap;
use rand::Rng;
use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit, AeadInPlace};
use sha2::{Sha256, Digest};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

// Re-export for compatibility
pub use ShamirErasureEncoder as ErasureEncoder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyShare {
    pub index: u8,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ShardWithKey {
    pub shard_data: Vec<u8>,
    pub key_share: KeyShare,
}

#[derive(Debug)]
pub struct ShamirErasureEncoder {
    threshold: usize,
    total_shards: usize,
    reed_solomon: ReedSolomon,
}

impl ShamirErasureEncoder {
    pub fn new(threshold: usize, total_shards: usize) -> Result<Self, FlowError> {
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
        
        if total_shards > 255 {
            return Err(FlowError::PersistenceError(
                "Total shards cannot exceed 255 (Shamir limitation)".to_string()
            ));
        }
        
        let parity_shards = total_shards - threshold;
        
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
    
    /// Generate a nonce from block ID for consistent encryption
    pub fn derive_nonce(block_id: &BlockId) -> [u8; 12] {
        let mut hasher = Sha256::new();
        hasher.update(b"SORADYNE_NONCE_V1");
        hasher.update(block_id);
        let hash = hasher.finalize();
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&hash[..12]);
        nonce
    }
    
    /// Derive a chunk-specific key from master key and chunk index
    fn derive_chunk_key(master_key: &[u8; 32], chunk_index: usize, block_id: &BlockId) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"SORADYNE_CHUNK_KEY_V1");
        hasher.update(master_key);
        hasher.update(&chunk_index.to_le_bytes());
        hasher.update(block_id);
        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash[..32]);
        key
    }
    
    /// Encode data with Shamir encryption scheme
    pub fn encode(&self, data: &[u8], block_id: &BlockId) -> Result<Vec<ShardWithKey>, FlowError> {
        if data.is_empty() {
            return Ok(vec![]);
        }
        
        // 1. Generate a new master secret key (32 bytes for AES-256)
        let mut rng = rand::rng();
        let master_key: [u8; 32] = rng.random();
        
        // 2. Encrypt the block data chunk by chunk
        let encrypted_data = self.encrypt_data_chunked(data, &master_key, block_id)?;
        
        // 3. Shamir secret share the master key
        let key_shares = self.split_secret(&master_key)?;
        
        // 4. Reed-Solomon encode the encrypted data
        let rs_shards = self.encode_rs(&encrypted_data)?;
        
        // 5. Combine RS shards with key shares
        let mut result = Vec::new();
        for (i, rs_shard) in rs_shards.into_iter().enumerate() {
            result.push(ShardWithKey {
                shard_data: rs_shard,
                key_share: key_shares[i].clone(),
            });
        }
        
        Ok(result)
    }
    
    /// Decode data with streaming capability for early reads
    pub fn decode_with_streaming(&self, shards: HashMap<usize, ShardWithKey>, block_id: &BlockId, expected_size: usize) -> Result<StreamingDecoder, FlowError> {
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        // Extract key shares and reconstruct the master encryption key
        let key_shares: Vec<KeyShare> = shards.values().map(|s| s.key_share.clone()).collect();
        let master_key = self.reconstruct_secret(&key_shares[..self.threshold])?;
        
        // Prepare for streaming RS reconstruction
        let rs_shards: HashMap<usize, Vec<u8>> = shards.into_iter()
            .map(|(i, shard)| (i, shard.shard_data))
            .collect();
        
        Ok(StreamingDecoder::new(
            rs_shards,
            master_key,
            *block_id,
            expected_size,
            self.threshold,
            self.total_shards,
        ))
    }
    
    /// Legacy decode method for compatibility
    pub fn decode(&self, shards: HashMap<usize, Vec<u8>>, expected_size: usize) -> Result<Vec<u8>, FlowError> {
        // This is for backward compatibility with old RS-only blocks
        // We'll implement this as a fallback for migration
        self.decode_rs_only(shards, expected_size)
    }
    
    /// Traditional Reed-Solomon encoding (for internal use)
    fn encode_rs(&self, data: &[u8]) -> Result<Vec<Vec<u8>>, FlowError> {
        let shard_size = (data.len() + self.threshold - 1) / self.threshold;
        let padded_size = shard_size * self.threshold;
        
        let mut padded_data = data.to_vec();
        padded_data.resize(padded_size, 0);
        
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
        
        // Generate parity shards
        self.reed_solomon.encode(&mut shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon encoding failed: {}", e)
            ))?;
        
        Ok(shards)
    }
    
    /// Legacy RS-only decode for backward compatibility
    fn decode_rs_only(&self, shards: HashMap<usize, Vec<u8>>, expected_size: usize) -> Result<Vec<u8>, FlowError> {
        if shards.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough shards: {} < {}", shards.len(), self.threshold)
            ));
        }
        
        let shard_size = shards.values().next()
            .ok_or_else(|| FlowError::PersistenceError("No shards available".to_string()))?
            .len();
        
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
        
        self.reed_solomon.reconstruct(&mut reconstruction_shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon reconstruction failed: {}", e)
            ))?;
        
        let mut result = Vec::with_capacity(expected_size);
        for i in 0..self.threshold {
            if let Some(ref shard) = reconstruction_shards[i] {
                result.extend_from_slice(shard);
            } else {
                return Err(FlowError::PersistenceError(
                    "Failed to reconstruct data shard".to_string()
                ));
            }
        }
        
        result.truncate(expected_size);
        Ok(result)
    }
    
    fn encrypt_data_chunked(&self, data: &[u8], master_key: &[u8; 32], block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        let mut result = Vec::new();
        let nonce_bytes = Self::derive_nonce(block_id);
        
        for (chunk_index, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
            let chunk_key = Self::derive_chunk_key(master_key, chunk_index, block_id);
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&chunk_key));
            let nonce = Nonce::from_slice(&nonce_bytes);
            
            let mut ciphertext = chunk.to_vec();
            let tag = cipher.encrypt_in_place_detached(nonce, b"", &mut ciphertext)
                .map_err(|e| FlowError::PersistenceError(
                    format!("Encryption failed for chunk {}: {}", chunk_index, e)
                ))?;
            
            // Store tag + ciphertext for this chunk
            result.extend_from_slice(&tag);
            result.extend_from_slice(&ciphertext);
        }
        
        Ok(result)
    }
    
    fn split_secret(&self, secret: &[u8; 32]) -> Result<Vec<KeyShare>, FlowError> {
        // Simple Shamir implementation - in production, use a proper library
        let mut shares = Vec::new();
        let mut rng = rand::rng();
        
        // For now, use a simple XOR-based secret sharing as placeholder
        // TODO: Replace with proper Shamir implementation
        let mut coefficients = vec![0u8; (self.threshold - 1) * 32];
        rng.fill(&mut coefficients[..]);
        
        for i in 1..=self.total_shards {
            let mut share_value = vec![0u8; 32];
            
            // Evaluate polynomial at point i
            for byte_idx in 0..32 {
                let mut value = secret[byte_idx];
                let mut x_power = i as u8;
                
                for coeff_idx in 0..(self.threshold - 1) {
                    let coeff = coefficients[coeff_idx * 32 + byte_idx];
                    value ^= coeff.wrapping_mul(x_power);
                    x_power = x_power.wrapping_mul(i as u8);
                }
                
                share_value[byte_idx] = value;
            }
            
            shares.push(KeyShare {
                index: i as u8,
                value: share_value,
            });
        }
        
        Ok(shares)
    }
    
    fn reconstruct_secret(&self, shares: &[KeyShare]) -> Result<[u8; 32], FlowError> {
        if shares.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough key shares: {} < {}", shares.len(), self.threshold)
            ));
        }
        
        // Simple reconstruction - TODO: Replace with proper Shamir
        let mut secret = [0u8; 32];
        
        // Use Lagrange interpolation at x=0
        for byte_idx in 0..32 {
            let mut result = 0u8;
            
            for i in 0..self.threshold {
                let xi = shares[i].index;
                let yi = shares[i].value[byte_idx];
                
                // Calculate Lagrange basis polynomial at x=0
                let mut numerator = 1u8;
                let mut denominator = 1u8;
                
                for j in 0..self.threshold {
                    if i != j {
                        let xj = shares[j].index;
                        numerator = numerator.wrapping_mul(xj);
                        denominator = denominator.wrapping_mul(xi ^ xj);
                    }
                }
                
                // Multiply by y_i and add to result
                let term = yi.wrapping_mul(numerator).wrapping_mul(self.gf_inverse(denominator));
                result ^= term;
            }
            
            secret[byte_idx] = result;
        }
        
        Ok(secret)
    }
    
    // Simple GF(256) inverse - TODO: Use proper implementation
    fn gf_inverse(&self, a: u8) -> u8 {
        if a == 0 { return 0; }
        
        // Extended Euclidean algorithm in GF(256)
        // This is a simplified version - use a proper GF library in production
        for i in 1..=255u8 {
            if self.gf_multiply(a, i) == 1 {
                return i;
            }
        }
        0
    }
    
    fn gf_multiply(&self, a: u8, b: u8) -> u8 {
        // Simple GF(256) multiplication
        let mut result = 0u8;
        let mut a = a;
        let mut b = b;
        
        while b != 0 {
            if b & 1 != 0 {
                result ^= a;
            }
            a = if a & 0x80 != 0 { (a << 1) ^ 0x1b } else { a << 1 };
            b >>= 1;
        }
        
        result
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

/// Streaming decoder for progressive reconstruction with read-ahead
pub struct StreamingDecoder {
    rs_shards: HashMap<usize, Vec<u8>>,
    master_key: [u8; 32],
    block_id: BlockId,
    expected_size: usize,
    threshold: usize,
    total_shards: usize,
    position: usize,
    chunk_cache: Arc<Mutex<HashMap<usize, Vec<u8>>>>,
    read_ahead_tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl StreamingDecoder {
    fn new(
        rs_shards: HashMap<usize, Vec<u8>>,
        master_key: [u8; 32],
        block_id: BlockId,
        expected_size: usize,
        threshold: usize,
        total_shards: usize,
    ) -> Self {
        Self {
            rs_shards,
            master_key,
            block_id,
            expected_size,
            threshold,
            total_shards,
            position: 0,
            chunk_cache: Arc::new(Mutex::new(HashMap::new())),
            read_ahead_tasks: Vec::new(),
        }
    }
    
    /// Read next chunk and return decrypted data with read-ahead
    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, FlowError> {
        let chunk_index = self.position / CHUNK_SIZE;
        let total_chunks = (self.expected_size + CHUNK_SIZE - 1) / CHUNK_SIZE;
        
        if chunk_index >= total_chunks {
            return Ok(None);
        }
        
        // Check cache first
        {
            let cache = self.chunk_cache.lock().await;
            if let Some(chunk_data) = cache.get(&chunk_index) {
                self.position += chunk_data.len();
                return Ok(Some(chunk_data.clone()));
            }
        }
        
        // Start read-ahead for next few chunks
        self.start_read_ahead(chunk_index, total_chunks).await;
        
        // Reconstruct current chunk
        let chunk_data = self.reconstruct_chunk(chunk_index).await?;
        self.position += chunk_data.len();
        
        Ok(Some(chunk_data))
    }
    
    async fn start_read_ahead(&mut self, current_chunk: usize, total_chunks: usize) {
        const READ_AHEAD_COUNT: usize = 4; // Read ahead 4 chunks
        
        for i in 1..=READ_AHEAD_COUNT {
            let chunk_index = current_chunk + i;
            if chunk_index >= total_chunks {
                break;
            }
            
            // Check if already cached or being processed
            {
                let cache = self.chunk_cache.lock().await;
                if cache.contains_key(&chunk_index) {
                    continue;
                }
            }
            
            // Start async reconstruction
            let rs_shards = self.rs_shards.clone();
            let master_key = self.master_key;
            let block_id = self.block_id;
            let threshold = self.threshold;
            let total_shards = self.total_shards;
            let cache = Arc::clone(&self.chunk_cache);
            
            let task = tokio::spawn(async move {
                if let Ok(chunk_data) = Self::reconstruct_chunk_static(
                    &rs_shards, master_key, block_id, chunk_index, threshold, total_shards
                ).await {
                    let mut cache = cache.lock().await;
                    cache.insert(chunk_index, chunk_data);
                }
            });
            
            self.read_ahead_tasks.push(task);
        }
    }
    
    async fn reconstruct_chunk(&self, chunk_index: usize) -> Result<Vec<u8>, FlowError> {
        Self::reconstruct_chunk_static(
            &self.rs_shards,
            self.master_key,
            self.block_id,
            chunk_index,
            self.threshold,
            self.total_shards,
        ).await
    }
    
    async fn reconstruct_chunk_static(
        rs_shards: &HashMap<usize, Vec<u8>>,
        master_key: [u8; 32],
        block_id: BlockId,
        chunk_index: usize,
        threshold: usize,
        total_shards: usize,
    ) -> Result<Vec<u8>, FlowError> {
        // Calculate chunk boundaries in the RS-encoded data
        let shard_size = rs_shards.values().next()
            .ok_or_else(|| FlowError::PersistenceError("No shards available".to_string()))?
            .len();
        
        let total_encrypted_size = shard_size * threshold;
        let chunk_start = chunk_index * (CHUNK_SIZE + 16); // +16 for AES-GCM tag
        let chunk_end = ((chunk_index + 1) * (CHUNK_SIZE + 16)).min(total_encrypted_size);
        
        if chunk_start >= total_encrypted_size {
            return Ok(Vec::new());
        }
        
        // Extract chunk data from shards
        let mut chunk_shards: Vec<Option<Vec<u8>>> = vec![None; total_shards];
        
        for (index, shard_data) in rs_shards {
            if *index >= total_shards {
                continue;
            }
            
            let shard_chunk_start = chunk_start / threshold;
            let shard_chunk_end = ((chunk_end + threshold - 1) / threshold).min(shard_data.len());
            
            if shard_chunk_start < shard_data.len() {
                chunk_shards[*index] = Some(shard_data[shard_chunk_start..shard_chunk_end].to_vec());
            }
        }
        
        // Reconstruct chunk using Reed-Solomon
        let reed_solomon = ReedSolomon::new(threshold, total_shards - threshold)
            .map_err(|e| FlowError::PersistenceError(
                format!("Failed to create Reed-Solomon decoder: {}", e)
            ))?;
        
        reed_solomon.reconstruct(&mut chunk_shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon reconstruction failed: {}", e)
            ))?;
        
        // Concatenate reconstructed data shards
        let mut encrypted_chunk = Vec::new();
        for i in 0..threshold {
            if let Some(ref shard) = chunk_shards[i] {
                encrypted_chunk.extend_from_slice(shard);
            }
        }
        
        // Extract the specific chunk we want
        let chunk_data = if chunk_start + (CHUNK_SIZE + 16) <= encrypted_chunk.len() {
            encrypted_chunk[chunk_start % (shard_size * threshold)..chunk_end % (shard_size * threshold)].to_vec()
        } else {
            encrypted_chunk[chunk_start % (shard_size * threshold)..].to_vec()
        };
        
        // Decrypt the chunk
        Self::decrypt_chunk(&chunk_data, &master_key, chunk_index, &block_id)
    }
    
    fn decrypt_chunk(encrypted_chunk: &[u8], master_key: &[u8; 32], chunk_index: usize, block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        if encrypted_chunk.len() < 16 {
            return Ok(Vec::new()); // Empty chunk
        }
        
        let chunk_key = ShamirErasureEncoder::derive_chunk_key(master_key, chunk_index, block_id);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&chunk_key));
        let nonce_bytes = ShamirErasureEncoder::derive_nonce(block_id);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Extract tag and ciphertext
        let (tag, ciphertext) = encrypted_chunk.split_at(16);
        let mut plaintext = ciphertext.to_vec();
        
        cipher.decrypt_in_place_detached(nonce, b"", &mut plaintext, tag.into())
            .map_err(|e| FlowError::PersistenceError(
                format!("Decryption failed for chunk {}: {}", chunk_index, e)
            ))?;
        
        Ok(plaintext)
    }
    
    /// Convenience method to reconstruct all data at once
    pub async fn reconstruct_all(&mut self) -> Result<Vec<u8>, FlowError> {
        let mut result = Vec::new();
        
        while let Some(chunk) = self.read_chunk().await? {
            result.extend(chunk);
        }
        
        // Wait for any remaining read-ahead tasks
        for task in self.read_ahead_tasks.drain(..) {
            let _ = task.await;
        }
        
        // Truncate to expected size
        result.truncate(self.expected_size);
        Ok(result)
    }
    
    /// Extract pointers early for indirect blocks
    pub async fn peek_pointers(&mut self, expected_pointer_count: usize) -> Result<Vec<[u8; 32]>, FlowError> {
        let bytes_needed = expected_pointer_count * 32;
        let mut pointer_data = Vec::new();
        
        while pointer_data.len() < bytes_needed {
            if let Some(chunk) = self.read_chunk().await? {
                pointer_data.extend(chunk);
            } else {
                break;
            }
        }
        
        let mut pointers = Vec::new();
        for chunk in pointer_data.chunks_exact(32) {
            let mut pointer = [0u8; 32];
            pointer.copy_from_slice(chunk);
            pointers.push(pointer);
        }
        
        Ok(pointers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_encode_decode_basic() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        let original_data = b"Hello, Shamir + Reed-Solomon world! This is a test of the new secure erasure coding system.";
        let block_id = [1u8; 32];
        
        // Encode
        let shards = encoder.encode(original_data, &block_id).unwrap();
        assert_eq!(shards.len(), 5);
        
        // Decode with all shards
        let mut shard_map = HashMap::new();
        for (i, shard) in shards.into_iter().enumerate() {
            shard_map.insert(i, shard);
        }
        
        let mut decoder = encoder.decode_with_streaming(shard_map, &block_id, original_data.len()).unwrap();
        let decoded = decoder.reconstruct_all().await.unwrap();
        assert_eq!(decoded, original_data);
    }
    
    #[tokio::test]
    async fn test_fault_tolerance() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        let original_data = b"This is fault tolerance testing with Shamir secret sharing!";
        let block_id = [2u8; 32];
        
        let shards = encoder.encode(original_data, &block_id).unwrap();
        
        // Test reconstruction with exactly threshold shards (lose 2 shards)
        let mut shard_map = HashMap::new();
        shard_map.insert(0, shards[0].clone());
        shard_map.insert(2, shards[2].clone());
        shard_map.insert(4, shards[4].clone());
        
        let mut decoder = encoder.decode_with_streaming(shard_map, &block_id, original_data.len()).unwrap();
        let decoded = decoder.reconstruct_all().await.unwrap();
        assert_eq!(decoded, original_data);
    }
    
    #[tokio::test]
    async fn test_streaming_chunks() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        let original_data = vec![42u8; CHUNK_SIZE * 3 + 1000]; // Multiple chunks
        let block_id = [3u8; 32];
        
        let shards = encoder.encode(&original_data, &block_id).unwrap();
        
        let mut shard_map = HashMap::new();
        for (i, shard) in shards.into_iter().enumerate() {
            shard_map.insert(i, shard);
        }
        
        let mut decoder = encoder.decode_with_streaming(shard_map, &block_id, original_data.len()).unwrap();
        
        // Read chunk by chunk
        let mut reconstructed = Vec::new();
        while let Some(chunk) = decoder.read_chunk().await.unwrap() {
            reconstructed.extend(chunk);
        }
        
        assert_eq!(reconstructed, original_data);
    }
}
