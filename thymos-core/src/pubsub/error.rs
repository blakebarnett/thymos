//! Error types for pub/sub operations

use crate::error::ThymosError;
use thiserror::Error;

/// Pub/sub specific errors
#[derive(Debug, Error)]
pub enum PubSubError {
    /// General pub/sub error
    #[error("Pub/sub error: {0}")]
    PubSub(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Distributed backend not available (feature flag missing)
    #[error("Distributed backend not available (feature flag missing)")]
    DistributedNotAvailable,

    /// SurrealDB connection error
    #[error("SurrealDB connection error: {0}")]
    SurrealDBConnection(String),

    /// Subscription not found
    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(String),

    /// Invalid topic name
    #[error("Invalid topic name: {0}")]
    InvalidTopic(String),

    /// Message type mismatch
    #[error("Message type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

impl From<PubSubError> for ThymosError {
    fn from(err: PubSubError) -> Self {
        match err {
            PubSubError::PubSub(msg) => ThymosError::Other(format!("Pub/sub error: {}", msg)),
            PubSubError::Serialization(e) => ThymosError::Serialization(e),
            PubSubError::DistributedNotAvailable => {
                ThymosError::Configuration("Distributed pub/sub not available".to_string())
            }
            PubSubError::SurrealDBConnection(msg) => {
                ThymosError::Other(format!("SurrealDB connection error: {}", msg))
            }
            PubSubError::SubscriptionNotFound(msg) => {
                ThymosError::Other(format!("Subscription not found: {}", msg))
            }
            PubSubError::InvalidTopic(msg) => ThymosError::Configuration(format!("Invalid topic: {}", msg)),
            PubSubError::TypeMismatch { expected, actual } => {
                ThymosError::Other(format!("Type mismatch: expected {}, got {}", expected, actual))
            }
        }
    }
}


