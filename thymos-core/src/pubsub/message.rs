//! Standard pub/sub message format

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Standard pub/sub message format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PubSubMessage {
    /// Topic name
    pub topic: String,

    /// Message content (JSON)
    pub content: serde_json::Value,

    /// Sender agent ID
    pub from: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional message ID for deduplication
    #[serde(default)]
    pub message_id: Option<Uuid>,

    /// Optional correlation ID for request/response tracking
    #[serde(default)]
    pub correlation_id: Option<Uuid>,
}

impl PubSubMessage {
    /// Create a new pub/sub message
    pub fn new(
        topic: impl Into<String>,
        content: serde_json::Value,
        from: impl Into<String>,
    ) -> Self {
        Self {
            topic: topic.into(),
            content,
            from: from.into(),
            timestamp: Utc::now(),
            message_id: Some(Uuid::new_v4()),
            correlation_id: None,
        }
    }

    /// Create a new message with correlation ID
    pub fn with_correlation_id(
        topic: impl Into<String>,
        content: serde_json::Value,
        from: impl Into<String>,
        correlation_id: Uuid,
    ) -> Self {
        Self {
            topic: topic.into(),
            content,
            from: from.into(),
            timestamp: Utc::now(),
            message_id: Some(Uuid::new_v4()),
            correlation_id: Some(correlation_id),
        }
    }
}


