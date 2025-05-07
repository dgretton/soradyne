use std::path::Path;
use uuid::Uuid;
use crate::flow::error::FlowError;

/// Trait for storage backends that can persist flow data
pub trait StorageBackend {
    /// Store data for a specific flow
    fn store(&self, flow_id: Uuid, data: &[u8]) -> Result<(), FlowError>;
    
    /// Load data for a specific flow
    fn load(&self, flow_id: Uuid) -> Result<Vec<u8>, FlowError>;
    
    /// Check if data exists for a specific flow
    fn exists(&self, flow_id: Uuid) -> bool;
    
    /// Delete data for a specific flow
    fn delete(&self, flow_id: Uuid) -> Result<(), FlowError>;
}

/// Trait for flow authenticators that can sign and verify flow data
pub trait FlowAuthenticator<T> {
    /// Sign the provided data
    fn sign(&self, data: &T) -> Result<Vec<u8>, FlowError>;
    
    /// Verify the signature for the provided data
    fn verify(&self, data: &T, signature: &[u8]) -> bool;
}

/// Enum representing different types of flows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowType {
    /// Real-time scalar data (e.g., heart rate)
    RealTimeScalar,
    
    /// File catalog (e.g., photo album)
    FileCatalog,
    
    /// Chat conversation
    Chat,
    
    /// Robot state
    RobotState,
    
    /// Custom flow type
    Custom,
}
