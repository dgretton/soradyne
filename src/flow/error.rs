use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("Persistence error: {0}")]
    PersistenceError(String),

    #[error("Subscription error: {0}")]
    SubscriptionError(String),
}

