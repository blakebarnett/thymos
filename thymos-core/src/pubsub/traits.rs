//! Core pub/sub trait definitions

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;

/// Unified pub/sub interface for agent coordination
#[async_trait]
pub trait PubSub: Send + Sync + Debug {
    /// Publish a message to a topic
    async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
    where
        M: Serialize + Send + Sync + 'static;

    /// Subscribe to a topic with a message handler
    async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
    where
        M: for<'de> Deserialize<'de> + Send + Sync + 'static,
        F: Fn(M) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static;

    /// Check if distributed coordination is enabled
    fn is_distributed(&self) -> bool;

    /// Get backend type for debugging/monitoring
    fn backend_type(&self) -> PubSubBackend;
}

/// Handle to a subscription (can be used to unsubscribe)
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    id: Uuid,
    topic: String,
    cancel: Arc<Notify>,
}

impl SubscriptionHandle {
    /// Create a new subscription handle
    pub(crate) fn new(id: Uuid, topic: String, cancel: Arc<Notify>) -> Self {
        Self { id, topic, cancel }
    }

    /// Get the subscription ID
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get the topic name
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// Unsubscribe from the topic
    pub async fn unsubscribe(self) -> Result<()> {
        self.cancel.notify_waiters();
        Ok(())
    }
}

/// Backend type for monitoring/debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSubBackend {
    /// Local-only (AutoAgents, in-process)
    Local,
    /// Distributed (SurrealDB, multi-process)
    Distributed,
    /// Hybrid (both local and distributed)
    Hybrid,
}


