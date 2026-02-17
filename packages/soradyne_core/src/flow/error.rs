use thiserror::Error;
use uuid::Uuid;

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

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Unknown flow type: {0}")]
    UnknownFlowType(String),

    #[error("Flow not found: {0}")]
    FlowNotFound(Uuid),

    #[error("Invalid stream name: {0}")]
    InvalidStreamName(String),

    #[error("Stream not implemented: {0}")]
    StreamNotImplemented(String),

    #[error("Host unavailable: {0}")]
    HostUnavailable(Uuid),

    #[error("Not the host: {0}")]
    NotHost(Uuid),

    #[error("Sync error: {0}")]
    SyncError(String),
}

