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
    
    /// Derive a chunk-specific nonce from block ID and chunk index
    fn derive_chunk_nonce(block_id: &BlockId, chunk_index: usize) -> [u8; 12] {
        let mut hasher = Sha256::new();
        hasher.update(b"SORADYNE_CHUNK_NONCE_V1");
        hasher.update(block_id);
        hasher.update(&chunk_index.to_le_bytes());
        let hash = hasher.finalize();
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&hash[..12]);
        nonce
    }
    
    /// Encode data with Shamir encryption scheme
    pub fn encode(&self, data: &[u8], block_id: &BlockId) -> Result<Vec<ShardWithKey>, FlowError> {
        if data.is_empty() {
            return Ok(vec![]);
        }
        
        // 1. Generate a new master secret key (32 bytes for AES-256)
        let mut rng = rand::thread_rng();
        let master_key: [u8; 32] = rng.gen();
        
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
        
        println!("🔧 Setting up decoding with {} shards", shards.len());
        
        // Extract key shares and reconstruct the master encryption key
        // IMPORTANT: Sort by Shamir index to ensure correct Lagrange interpolation
        let mut key_shares: Vec<KeyShare> = shards.values()
            .map(|s| s.key_share.clone())
            .collect();
        
        println!("🔧 Key shares before sorting:");
        for (i, share) in key_shares.iter().enumerate() {
            println!("   Share {}: index={}", i, share.index);
        }
        
        // Sort by Shamir index to ensure deterministic reconstruction
        key_shares.sort_by_key(|share| share.index);
        
        println!("🔧 Key shares after sorting:");
        for (i, share) in key_shares.iter().enumerate() {
            println!("   Share {}: index={}", i, share.index);
        }
        
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
        println!("🔧 Reed-Solomon encoding {} bytes with threshold={}, total_shards={}", 
                 data.len(), self.threshold, self.total_shards);
        
        let shard_size = (data.len() + self.threshold - 1) / self.threshold;
        let padded_size = shard_size * self.threshold;
        
        println!("🔧 Shard size: {}, padded size: {}", shard_size, padded_size);
        
        let mut padded_data = data.to_vec();
        padded_data.resize(padded_size, 0);
        
        // Store the original length at the beginning for proper truncation
        let mut length_prefixed_data = Vec::new();
        length_prefixed_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
        length_prefixed_data.extend_from_slice(&padded_data);
        
        // Recalculate with length prefix
        let total_size = length_prefixed_data.len();
        let shard_size = (total_size + self.threshold - 1) / self.threshold;
        let padded_size = shard_size * self.threshold;
        
        length_prefixed_data.resize(padded_size, 0);
        
        println!("🔧 With length prefix: total_size={}, shard_size={}, padded_size={}", 
                 total_size, shard_size, padded_size);
        
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(self.total_shards);
        
        // Create data shards
        for i in 0..self.threshold {
            let start = i * shard_size;
            let end = start + shard_size;
            shards.push(length_prefixed_data[start..end].to_vec());
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
        
        println!("🔧 Reed-Solomon encoding complete: {} shards of {} bytes each", 
                 shards.len(), shard_size);
        
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
        println!("🔐 Encrypting {} bytes with master key[0..8]: {:02x?}", data.len(), &master_key[..8]);
        
        let mut result = Vec::new();
        
        for (chunk_index, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
            let chunk_key = Self::derive_chunk_key(master_key, chunk_index, block_id);
            println!("🔐 Chunk {}: size={}, key[0..8]={:02x?}", chunk_index, chunk.len(), &chunk_key[..8]);
            
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&chunk_key));
            
            // Derive unique nonce for each chunk
            let chunk_nonce = Self::derive_chunk_nonce(block_id, chunk_index);
            let nonce = Nonce::from_slice(&chunk_nonce);
            
            println!("🔐 Chunk {} nonce: {:02x?}", chunk_index, &chunk_nonce);
            
            let mut ciphertext = chunk.to_vec();
            let tag = cipher.encrypt_in_place_detached(nonce, b"", &mut ciphertext)
                .map_err(|e| FlowError::PersistenceError(
                    format!("Encryption failed for chunk {}: {}", chunk_index, e)
                ))?;
            
            println!("🔐 Chunk {} encrypted: tag={:02x?}, ciphertext[0..8]={:02x?}", 
                     chunk_index, &tag[..8], &ciphertext[..8.min(ciphertext.len())]);
            
            // Store tag + ciphertext for this chunk
            result.extend_from_slice(&tag);
            result.extend_from_slice(&ciphertext);
        }
        
        println!("🔐 Total encrypted size: {} bytes", result.len());
        Ok(result)
    }
    
    fn split_secret(&self, secret: &[u8; 32]) -> Result<Vec<KeyShare>, FlowError> {
        use crate::storage::galois::GF256;
        
        let gf = GF256::new();
        let mut shares = Vec::new();
        let mut rng = rand::thread_rng();
        
        // Generate random coefficients for the polynomial
        // f(x) = secret + a1*x + a2*x^2 + ... + a(k-1)*x^(k-1)
        let mut coefficients = vec![0u8; (self.threshold - 1) * 32];
        rng.fill(&mut coefficients[..]);
        
        for i in 1..=self.total_shards {
            let mut share_value = vec![0u8; 32];
            let x = i as u8;
            
            // Evaluate polynomial at point x for each byte
            for byte_idx in 0..32 {
                // Build polynomial for this byte: [secret_byte, coeff1, coeff2, ...]
                let mut poly = vec![secret[byte_idx]];
                for coeff_idx in 0..(self.threshold - 1) {
                    poly.push(coefficients[coeff_idx * 32 + byte_idx]);
                }
                
                share_value[byte_idx] = gf.eval_polynomial(&poly, x);
            }
            
            shares.push(KeyShare {
                index: x,
                value: share_value,
            });
        }
        
        Ok(shares)
    }
    
    fn reconstruct_secret(&self, shares: &[KeyShare]) -> Result<[u8; 32], FlowError> {
        use crate::storage::galois::GF256;
        
        if shares.len() < self.threshold {
            return Err(FlowError::PersistenceError(
                format!("Not enough key shares: {} < {}", shares.len(), self.threshold)
            ));
        }
        
        println!("🔑 Reconstructing secret from {} shares (threshold: {})", shares.len(), self.threshold);
        for (i, share) in shares.iter().enumerate() {
            println!("   Share {}: index={}, value[0..8]={:02x?}", i, share.index, &share.value[..8]);
        }
        
        let gf = GF256::new();
        let mut secret = [0u8; 32];
        
        // Use Lagrange interpolation to find f(0) = secret for each byte
        for byte_idx in 0..32 {
            // Collect points (x_i, y_i) for this byte
            let points: Vec<(u8, u8)> = shares[..self.threshold]
                .iter()
                .map(|share| (share.index, share.value[byte_idx]))
                .collect();
            
            if byte_idx < 8 { // Only debug first 8 bytes to avoid spam
                println!("   Byte {}: points={:?}", byte_idx, points);
            }
            
            secret[byte_idx] = gf.lagrange_interpolate_at_zero(&points)
                .map_err(|e| FlowError::PersistenceError(
                    format!("Lagrange interpolation failed for byte {}: {}", byte_idx, e)
                ))?;
        }
        
        println!("🔑 Reconstructed master key[0..8]: {:02x?}", &secret[..8]);
        Ok(secret)
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
        // For simplicity, let's reconstruct the entire encrypted data first
        // This is less efficient but more reliable for the initial implementation
        
        let _shard_size = rs_shards.values().next()
            .ok_or_else(|| FlowError::PersistenceError("No shards available".to_string()))?
            .len();
        
        // Reconstruct all shards
        let mut reconstruction_shards: Vec<Option<Vec<u8>>> = vec![None; total_shards];
        
        for (index, shard_data) in rs_shards {
            if *index < total_shards {
                reconstruction_shards[*index] = Some(shard_data.clone());
            }
        }
        
        // Reconstruct using Reed-Solomon
        let reed_solomon = ReedSolomon::new(threshold, total_shards - threshold)
            .map_err(|e| FlowError::PersistenceError(
                format!("Failed to create Reed-Solomon decoder: {}", e)
            ))?;
        
        reed_solomon.reconstruct(&mut reconstruction_shards)
            .map_err(|e| FlowError::PersistenceError(
                format!("Reed-Solomon reconstruction failed: {}", e)
            ))?;
        
        // Concatenate the data shards to get the full encrypted data
        let mut full_encrypted_data = Vec::new();
        for i in 0..threshold {
            if let Some(ref shard) = reconstruction_shards[i] {
                full_encrypted_data.extend_from_slice(shard);
            }
        }
        
        // Extract the specific chunk (each chunk is CHUNK_SIZE + 16 bytes for tag)
        let chunk_size_with_tag = CHUNK_SIZE + 16;
        let chunk_start = chunk_index * chunk_size_with_tag;
        let chunk_end = (chunk_start + chunk_size_with_tag).min(full_encrypted_data.len());
        
        if chunk_start >= full_encrypted_data.len() {
            return Ok(Vec::new());
        }
        
        let encrypted_chunk = &full_encrypted_data[chunk_start..chunk_end];
        
        // Decrypt the chunk
        Self::decrypt_chunk(encrypted_chunk, &master_key, chunk_index, &block_id)
    }
    
    fn decrypt_chunk(encrypted_chunk: &[u8], master_key: &[u8; 32], chunk_index: usize, block_id: &BlockId) -> Result<Vec<u8>, FlowError> {
        println!("🔓 Decrypting chunk {}: {} bytes, master_key[0..8]={:02x?}", 
                 chunk_index, encrypted_chunk.len(), &master_key[..8]);
        
        if encrypted_chunk.len() < 16 {
            println!("🔓 Chunk {} too small: {} bytes", chunk_index, encrypted_chunk.len());
            return Ok(Vec::new()); // Empty chunk
        }
        
        let chunk_key = ShamirErasureEncoder::derive_chunk_key(master_key, chunk_index, block_id);
        println!("🔓 Chunk {} key[0..8]: {:02x?}", chunk_index, &chunk_key[..8]);
        
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&chunk_key));
        
        // Use the same chunk-specific nonce that was used for encryption
        let chunk_nonce = ShamirErasureEncoder::derive_chunk_nonce(block_id, chunk_index);
        let nonce = Nonce::from_slice(&chunk_nonce);
        
        println!("🔓 Chunk {} nonce: {:02x?}", chunk_index, &chunk_nonce);
        
        // Extract tag and ciphertext
        let (tag, ciphertext) = encrypted_chunk.split_at(16);
        println!("🔓 Chunk {} tag: {:02x?}", chunk_index, &tag[..8]);
        println!("🔓 Chunk {} ciphertext[0..8]: {:02x?}", chunk_index, &ciphertext[..8.min(ciphertext.len())]);
        
        let mut plaintext = ciphertext.to_vec();
        
        match cipher.decrypt_in_place_detached(nonce, b"", &mut plaintext, tag.into()) {
            Ok(()) => {
                println!("🔓 Chunk {} decrypted successfully: plaintext[0..8]={:02x?}", chunk_index, &plaintext[..8.min(plaintext.len())]);
                Ok(plaintext)
            }
            Err(e) => {
                println!("🔓 Chunk {} decryption failed: {}", chunk_index, e);
                Err(FlowError::PersistenceError(
                    format!("Decryption failed for chunk {}: {}", chunk_index, e)
                ))
            }
        }
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
    
    #[test]
    fn test_key_sharing_basic() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        let original_key = [42u8; 32]; // Test key
        
        // Split the key into shares
        let shares = encoder.split_secret(&original_key).unwrap();
        assert_eq!(shares.len(), 5);
        
        // Reconstruct with exactly threshold shares
        let reconstructed_key = encoder.reconstruct_secret(&shares[..3]).unwrap();
        assert_eq!(reconstructed_key, original_key);
        
        // Test with different subset of shares
        let subset_shares = [shares[0].clone(), shares[2].clone(), shares[4].clone()];
        let reconstructed_key2 = encoder.reconstruct_secret(&subset_shares).unwrap();
        assert_eq!(reconstructed_key2, original_key);
    }
    
    #[test]
    fn test_key_sharing_insufficient_shares() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        let original_key = [123u8; 32];
        
        let shares = encoder.split_secret(&original_key).unwrap();
        
        // Try to reconstruct with insufficient shares (only 2, need 3)
        let result = encoder.reconstruct_secret(&shares[..2]);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_key_sharing_all_different_keys() {
        let encoder = ShamirErasureEncoder::new(3, 5).unwrap();
        
        // Test with several different keys
        let test_keys = [
            [0u8; 32],
            [255u8; 32],
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
             17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
        ];
        
        for (i, &original_key) in test_keys.iter().enumerate() {
            println!("Testing key {}: {:?}", i, &original_key[..8]);
            
            let shares = encoder.split_secret(&original_key).unwrap();
            let reconstructed = encoder.reconstruct_secret(&shares[..3]).unwrap();
            
            assert_eq!(reconstructed, original_key, "Failed for key {}", i);
        }
    }
    
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
