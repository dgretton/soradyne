//! Examples and demonstrations of the dissolution storage system

use std::path::PathBuf;
use crate::storage::{
    DissolutionStorageFactory, DissolutionFile,
    dissolution::{BackendConfig, DissolutionStorage}
};
use crate::flow::FlowError;

/// Example: Basic dissolution storage usage
pub async fn basic_dissolution_example() -> Result<(), FlowError> {
    // Create configuration for soradyne erasure backend
    let config = DissolutionStorageFactory::create_sdyn_erasure_config(
        3, // threshold: need 3 shards to reconstruct
        5, // total_shards: create 5 shards total
        PathBuf::from("/tmp/dissolution_metadata.json"),
    ).await?;
    
    // Create storage backend
    let storage = DissolutionStorageFactory::create(config).await?;
    
    // Store some data
    let test_data = b"Hello, dissolution storage!";
    let block_id = storage.store(test_data).await?;
    println!("Stored data with block ID: {}", hex::encode(block_id));
    
    // Retrieve the data
    let retrieved_data = storage.retrieve(&block_id).await?;
    assert_eq!(test_data, retrieved_data.as_slice());
    println!("Successfully retrieved data: {}", String::from_utf8_lossy(&retrieved_data));
    
    // Get block information
    let block_info = storage.block_info(&block_id).await?;
    println!("Block info: {} bytes, {} shards, can reconstruct: {}", 
             block_info.size, block_info.shard_count, block_info.can_reconstruct);
    
    Ok(())
}

/// Example: Demonstrate fault tolerance
pub async fn fault_tolerance_demo() -> Result<(), FlowError> {
    let config = DissolutionStorageFactory::create_sdyn_erasure_config(
        3, 5, PathBuf::from("/tmp/dissolution_metadata.json")
    ).await?;
    
    let storage = DissolutionStorageFactory::create(config).await?;
    
    // Store test data
    let test_data = b"This data will survive device failures!";
    let block_id = storage.store(test_data).await?;
    
    // Demonstrate that we can lose 2 shards and still reconstruct
    let demo = storage.demonstrate_dissolution(&block_id, vec![0, 1]).await?;
    
    println!("Fault Tolerance Demo Results:");
    println!("  Original shards: {}", demo.original_shards);
    println!("  Simulated missing: {:?}", demo.simulated_missing);
    println!("  Available shards: {}", demo.available_shards);
    println!("  Threshold required: {}", demo.threshold_required);
    println!("  Can reconstruct: {}", demo.can_reconstruct);
    println!("  Reconstruction successful: {}", demo.reconstruction_successful);
    println!("  Data integrity verified: {}", demo.data_integrity_verified);
    
    Ok(())
}

/// Example: High-level file interface
pub async fn file_interface_example() -> Result<(), FlowError> {
    let config = DissolutionStorageFactory::create_sdyn_erasure_config(
        2, 3, PathBuf::from("/tmp/dissolution_metadata.json")
    ).await?;
    
    let storage = DissolutionStorageFactory::create(config).await?;
    
    // Create a dissolution file
    let mut file = DissolutionFile::new(storage.clone());
    
    // Write data to the file
    let file_content = b"This is a file stored using dissolution!";
    file.write(file_content).await?;
    
    println!("File written, size: {} bytes", file.size());
    if let Some(root_block) = file.root_block() {
        println!("Root block ID: {}", hex::encode(root_block));
    }
    
    // Read the file back
    let read_content = file.read().await?;
    assert_eq!(file_content, read_content.as_slice());
    println!("File content: {}", String::from_utf8_lossy(&read_content));
    
    // Check file info
    if let Some(info) = file.info().await? {
        println!("File info: {} bytes, {} available shards", 
                 info.size, info.available_shards);
    }
    
    Ok(())
}

/// Example: Backend detection and selection
pub async fn backend_detection_example() -> Result<(), FlowError> {
    let available_backends = DissolutionStorageFactory::detect_available_backends().await;
    
    println!("Available dissolution storage backends:");
    for backend in &available_backends {
        println!("  - {}", backend);
    }
    
    // Create default configuration based on available backends
    let config = DissolutionStorageFactory::create_default_config(
        2, 4, PathBuf::from("/tmp/dissolution_metadata.json")
    ).await?;
    
    match &config.backend_config {
        BackendConfig::SdynErasure { rimsd_paths, .. } => {
            println!("Using soradyne erasure backend with {} devices", rimsd_paths.len());
        }
        BackendConfig::BcacheFS { device_paths, .. } => {
            println!("Using bcachefs backend with {} devices", device_paths.len());
        }
        _ => {
            println!("Using other backend type");
        }
    }
    
    Ok(())
}

/// Run all examples
pub async fn run_all_examples() -> Result<(), FlowError> {
    println!("=== Dissolution Storage Examples ===\n");
    
    println!("1. Backend Detection:");
    backend_detection_example().await?;
    println!();
    
    println!("2. Basic Dissolution Storage:");
    basic_dissolution_example().await?;
    println!();
    
    println!("3. Fault Tolerance Demo:");
    fault_tolerance_demo().await?;
    println!();
    
    println!("4. High-Level File Interface:");
    file_interface_example().await?;
    println!();
    
    println!("All examples completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_backend_detection() {
        let backends = DissolutionStorageFactory::detect_available_backends().await;
        assert!(!backends.is_empty());
        assert!(backends.contains(&"sdyn_erasure".to_string()));
    }
    
    #[tokio::test]
    #[ignore] // Requires actual SD cards
    async fn test_basic_dissolution() {
        basic_dissolution_example().await.unwrap();
    }
}
