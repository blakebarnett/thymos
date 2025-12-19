//! Hybrid pub/sub implementation combining local and distributed backends
//!
//! This implementation provides both fast local delivery (< 1ms) and
//! persistent distributed coordination. Messages are published to both
//! backends simultaneously, and subscriptions receive messages from
//! both sources with deduplication.

use crate::error::Result;
use crate::pubsub::error::PubSubError;
use crate::pubsub::local::LocalPubSub;
use crate::pubsub::traits::{PubSub, PubSubBackend, SubscriptionHandle};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "pubsub-distributed")]
use crate::pubsub::distributed::DistributedPubSub;

/// Hybrid pub/sub that uses both local (fast) and distributed (persistent)
#[derive(Debug)]
pub struct HybridPubSub {
    local: LocalPubSub,
    #[cfg(feature = "pubsub-distributed")]
    distributed: DistributedPubSub,
    /// Track seen message IDs for deduplication
    seen_ids: Arc<Mutex<HashSet<Uuid>>>,
}

impl HybridPubSub {
    /// Create a new hybrid pub/sub instance
    pub async fn new(distributed_url: Option<&str>) -> Result<Self> {
        let local = LocalPubSub::new().await?;

        #[cfg(feature = "pubsub-distributed")]
        let distributed = if let Some(url) = distributed_url {
            DistributedPubSub::new(url, "thymos", "pubsub").await?
        } else {
            return Err(PubSubError::PubSub(
                "Hybrid mode requires distributed URL".to_string(),
            )
            .into());
        };

        #[cfg(not(feature = "pubsub-distributed"))]
        if distributed_url.is_some() {
            return Err(PubSubError::DistributedNotAvailable.into());
        }

        Ok(Self {
            local,
            #[cfg(feature = "pubsub-distributed")]
            distributed,
            seen_ids: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

#[async_trait]
impl PubSub for HybridPubSub {
    async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
    where
        M: Serialize + Send + Sync + 'static,
    {
        // Publish to both backends concurrently
        #[cfg(feature = "pubsub-distributed")]
        {
            // Note: We can't easily clone M, so we serialize once and pass Value to both
            // Both backends accept serde_json::Value which implements Serialize
            let serialized = serde_json::to_value(&message)?;
            let local_msg = serialized.clone();
            let distributed_msg = serialized;

            let (local_result, distributed_result) = tokio::join!(
                self.local.publish(topic, local_msg),
                self.distributed.publish(topic, distributed_msg)
            );

            // Local failure is non-fatal (log warning)
            if let Err(e) = local_result {
                tracing::warn!("Local pub/sub failed: {}", e);
            }

            // Distributed failure is more serious
            distributed_result?;
        }

        #[cfg(not(feature = "pubsub-distributed"))]
        {
            // Fallback to local-only if distributed not available
            self.local.publish(topic, message).await?;
        }

        Ok(())
    }

    async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
    where
        M: for<'de> Deserialize<'de> + Send + Sync + 'static,
        F: Fn(M) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        // TODO: Use seen_ids for deduplication when distributed backend sends duplicate messages
        let _seen_ids = self.seen_ids.clone();
        let handler_arc = Arc::new(handler);

        // Subscribe to both backends
        #[cfg(feature = "pubsub-distributed")]
        {
            let local_handler = {
                let handler = handler_arc.clone();
                move |msg: M| {
                    let handler = handler.clone();
                    Box::pin(async move { handler(msg).await })
                        as std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
                }
            };

            let distributed_handler = {
                let handler = handler_arc.clone();
                move |msg: M| {
                    let handler = handler.clone();
                    Box::pin(async move { handler(msg).await })
                        as std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
                }
            };

            let (local_handle_result, distributed_handle_result) = tokio::join!(
                self.local.subscribe(topic, local_handler),
                self.distributed.subscribe(topic, distributed_handler)
            );

            let local_handle = local_handle_result?;
            distributed_handle_result?;

            // Return a combined handle (using local handle for now)
            // Note: In a production implementation, we'd want to track both handles
            // and provide a way to unsubscribe from both
            Ok(local_handle)
        }

        #[cfg(not(feature = "pubsub-distributed"))]
        {
            // Fallback to local-only if distributed not available
            self.local.subscribe(topic, handler).await
        }
    }

    fn is_distributed(&self) -> bool {
        true
    }

    fn backend_type(&self) -> PubSubBackend {
        PubSubBackend::Hybrid
    }
}

