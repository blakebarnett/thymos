//! Builder for creating pub/sub instances

use crate::error::Result;
use serde::{Deserialize, Serialize};

use super::local::LocalPubSub;
use super::traits::PubSub;

#[cfg(feature = "pubsub-distributed")]
use super::distributed::DistributedPubSub;

#[cfg(feature = "pubsub-distributed")]
use super::hybrid::HybridPubSub;

/// Builder for creating pub/sub instances
pub struct PubSubBuilder {
    mode: PubSubMode,
    distributed_url: Option<String>,
    namespace: Option<String>,
    database: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PubSubMode {
    /// Local-only (AutoAgents, always available)
    Local,

    /// Distributed-only (SurrealDB, requires feature flag)
    #[cfg(feature = "pubsub-distributed")]
    Distributed,

    /// Hybrid (both local and distributed)
    #[cfg(feature = "pubsub-distributed")]
    Hybrid,
}

impl PubSubBuilder {
    /// Create a new pub/sub builder
    pub fn new() -> Self {
        Self {
            mode: PubSubMode::Local,
            distributed_url: None,
            namespace: None,
            database: None,
        }
    }

    /// Use local-only pub/sub (default)
    pub fn local(mut self) -> Self {
        self.mode = PubSubMode::Local;
        self
    }

    /// Use distributed pub/sub (requires feature flag)
    #[cfg(feature = "pubsub-distributed")]
    pub fn distributed(mut self, url: impl Into<String>) -> Self {
        self.mode = PubSubMode::Distributed;
        self.distributed_url = Some(url.into());
        self
    }

    /// Use hybrid pub/sub (requires feature flag)
    #[cfg(feature = "pubsub-distributed")]
    pub fn hybrid(mut self, distributed_url: impl Into<String>) -> Self {
        self.mode = PubSubMode::Hybrid;
        self.distributed_url = Some(distributed_url.into());
        self
    }

    /// Set SurrealDB namespace (for distributed mode)
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set SurrealDB database (for distributed mode)
    pub fn database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Build the pub/sub instance
    pub async fn build(self) -> Result<PubSubInstance> {
        match self.mode {
            PubSubMode::Local => Ok(PubSubInstance::Local(LocalPubSub::new().await?)),

            #[cfg(feature = "pubsub-distributed")]
            PubSubMode::Distributed => {
                let url = self.distributed_url.ok_or_else(|| {
                    crate::error::ThymosError::Configuration(
                        "Distributed mode requires URL".to_string(),
                    )
                })?;

                let namespace = self.namespace.unwrap_or_else(|| "thymos".to_string());
                let database = self.database.unwrap_or_else(|| "pubsub".to_string());

                Ok(PubSubInstance::Distributed(
                    DistributedPubSub::new(&url, &namespace, &database).await?,
                ))
            }

            #[cfg(feature = "pubsub-distributed")]
            PubSubMode::Hybrid => {
                let url = self.distributed_url.ok_or_else(|| {
                    crate::error::ThymosError::Configuration(
                        "Hybrid mode requires distributed URL".to_string(),
                    )
                })?;

                Ok(PubSubInstance::Hybrid(HybridPubSub::new(Some(&url)).await?))
            }
        }
    }
}

/// Enum wrapper for different pub/sub implementations
#[derive(Debug)]
pub enum PubSubInstance {
    Local(LocalPubSub),
    #[cfg(feature = "pubsub-distributed")]
    Distributed(DistributedPubSub),
    #[cfg(feature = "pubsub-distributed")]
    Hybrid(HybridPubSub),
}

#[async_trait::async_trait]
impl PubSub for PubSubInstance {
    async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
    where
        M: serde::Serialize + Send + Sync + 'static,
    {
        match self {
            PubSubInstance::Local(inner) => inner.publish(topic, message).await,
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Distributed(inner) => inner.publish(topic, message).await,
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Hybrid(inner) => inner.publish(topic, message).await,
        }
    }

    async fn subscribe<M, F>(
        &self,
        topic: &str,
        handler: F,
    ) -> Result<super::traits::SubscriptionHandle>
    where
        M: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
        F: Fn(M) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        match self {
            PubSubInstance::Local(inner) => inner.subscribe(topic, handler).await,
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Distributed(inner) => inner.subscribe(topic, handler).await,
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Hybrid(inner) => inner.subscribe(topic, handler).await,
        }
    }

    fn is_distributed(&self) -> bool {
        match self {
            PubSubInstance::Local(inner) => inner.is_distributed(),
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Distributed(inner) => inner.is_distributed(),
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Hybrid(inner) => inner.is_distributed(),
        }
    }

    fn backend_type(&self) -> super::traits::PubSubBackend {
        match self {
            PubSubInstance::Local(inner) => inner.backend_type(),
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Distributed(inner) => inner.backend_type(),
            #[cfg(feature = "pubsub-distributed")]
            PubSubInstance::Hybrid(inner) => inner.backend_type(),
        }
    }
}

impl Default for PubSubBuilder {
    fn default() -> Self {
        Self::new()
    }
}

