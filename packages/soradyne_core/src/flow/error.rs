use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("Persistence error: {0}")]
    PersistenceError(String),

    #[error("Subscription error: {0}")]
    SubscriptionError(String),
    
    #[error("Storage backend error: {0}")]
    StorageBackendError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Device identity error: {0}")]
    DeviceIdentityError(String),
}

