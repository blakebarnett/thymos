//! Local pub/sub implementation using AutoAgents runtime
//!
//! This implementation provides fast, in-process pub/sub coordination
//! using AutoAgents SingleThreadedRuntime for message routing.

use crate::error::Result;
use crate::pubsub::error::PubSubError;
use crate::pubsub::message::PubSubMessage;
use crate::pubsub::traits::{PubSub, PubSubBackend, SubscriptionHandle};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use autoagents_core::actor::{ActorMessage, CloneableMessage, Topic};
use autoagents_core::ractor::{Actor, ActorRef};
use autoagents_core::runtime::{Runtime, SingleThreadedRuntime, TypedRuntime};
use std::error::Error as StdError;

/// Wrapper message type for pub/sub that implements CloneableMessage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessageWrapper {
    pub inner: PubSubMessage,
}

impl CloneableMessage for PubSubMessageWrapper {}
impl ActorMessage for PubSubMessageWrapper {}

/// Handler function type for pub/sub messages
type MessageHandler = Arc<
    dyn Fn(serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

/// Actor that wraps a function handler for pub/sub messages
/// This actor receives PubSubMessageWrapper and calls the user's handler with the deserialized content
struct HandlerActor {
    handler: MessageHandler,
    #[allow(dead_code)]
    topic: String,
}

#[async_trait]
impl Actor for HandlerActor {
    type Msg = PubSubMessageWrapper;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> std::result::Result<Self::State, Box<dyn StdError + Send + Sync>> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> std::result::Result<(), Box<dyn StdError + Send + Sync>> {
        let handler = self.handler.clone();
        let result = handler(message.inner.content).await;
        result.map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)
    }
}

/// Local pub/sub implementation using AutoAgents SingleThreadedRuntime
#[derive(Debug, Clone)]
pub struct LocalPubSub {
    runtime: Arc<SingleThreadedRuntime>,
    /// Map of topic names to topic instances (for type-safe routing)
    topics: Arc<RwLock<HashMap<String, Arc<Topic<PubSubMessageWrapper>>>>>,
    /// Runtime handle for background task
    #[allow(dead_code)]
    runtime_handle: Arc<tokio::task::JoinHandle<()>>,
}

impl LocalPubSub {
    /// Create a new local pub/sub instance
    pub async fn new() -> Result<Self> {
        let runtime = SingleThreadedRuntime::new(None);

        // Start runtime in background
        let runtime_clone = runtime.clone();
        let runtime_handle = tokio::spawn(async move {
            if let Err(e) = runtime_clone.run().await {
                tracing::error!("Runtime error: {}", e);
            }
        });

        Ok(Self {
            runtime,
            topics: Arc::new(RwLock::new(HashMap::new())),
            runtime_handle: Arc::new(runtime_handle),
        })
    }

    /// Get or create a topic for the given topic name
    async fn get_or_create_topic(
        &self,
        topic_name: &str,
    ) -> Arc<Topic<PubSubMessageWrapper>> {
        let mut topics = self.topics.write().await;
        topics
            .entry(topic_name.to_string())
            .or_insert_with(|| Arc::new(Topic::<PubSubMessageWrapper>::new(topic_name)))
            .clone()
    }
}

#[async_trait]
impl PubSub for LocalPubSub {
    async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
    where
        M: Serialize + Send + Sync + 'static,
    {
        // Validate topic name
        if topic.is_empty() {
            return Err(PubSubError::InvalidTopic("Topic name cannot be empty".to_string()).into());
        }

        // Convert message to PubSubMessage
        let pubsub_msg = PubSubMessage::new(
            topic,
            serde_json::to_value(message)?,
            "unknown", // TODO: get from context when integrated with agents
        );

        // Wrap in AutoAgents message type
        let wrapper = PubSubMessageWrapper { inner: pubsub_msg };

        // Get or create topic
        let topic_typed = self.get_or_create_topic(topic).await;

        // Publish via AutoAgents runtime
        self.runtime
            .publish(&topic_typed, wrapper)
            .await
            .map_err(|e| {
                PubSubError::PubSub(format!("Failed to publish message: {}", e))
            })?;

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
        // Validate topic name
        if topic.is_empty() {
            return Err(PubSubError::InvalidTopic("Topic name cannot be empty".to_string()).into());
        }

        // Create a wrapper actor that calls the user's handler
        // The actor receives PubSubMessageWrapper and extracts M from the content
        let handler_arc = Arc::new(handler);
        let topic_str = topic.to_string();
        let topic_for_actor = topic_str.clone();
        let handler_wrapper = Arc::new(move |content: serde_json::Value| {
            let handler = handler_arc.clone();
            let topic = topic_for_actor.clone();
            Box::pin(async move {
                // Deserialize the content to M
                match serde_json::from_value::<M>(content) {
                    Ok(typed_msg) => handler(typed_msg).await,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to deserialize message for topic {}: {}",
                            topic,
                            e
                        );
                        Ok(())
                    }
                }
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
        });

        let actor = HandlerActor {
            handler: handler_wrapper,
            topic: topic_str,
        };

        // Spawn the actor
        let (actor_ref, _) = Actor::spawn(None, actor, ()).await.map_err(|e| {
            PubSubError::PubSub(format!("Failed to spawn handler actor: {}", e))
        })?;

        // Get or create topic
        let topic_typed = self.get_or_create_topic(topic).await;

        // Subscribe actor to topic
        self.runtime
            .subscribe(&topic_typed, actor_ref)
            .await
            .map_err(|e| {
                PubSubError::PubSub(format!("Failed to subscribe to topic: {}", e))
            })?;

        // Create subscription handle
        let subscription_id = Uuid::new_v4();
        let cancel = Arc::new(tokio::sync::Notify::new());

        // Note: For proper unsubscribe, we'd need to track the actor_ref
        // and stop it. For now, we'll use a simple cancel mechanism.
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            cancel_clone.notified().await;
            // Actor will be cleaned up when dropped
        });

        Ok(SubscriptionHandle::new(subscription_id, topic.to_string(), cancel))
    }

    fn is_distributed(&self) -> bool {
        false
    }

    fn backend_type(&self) -> PubSubBackend {
        PubSubBackend::Local
    }
}
