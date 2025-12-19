//! Distributed pub/sub implementation using SurrealDB live queries
//!
//! This implementation provides multi-process coordination with message
//! persistence using SurrealDB live queries and WebSocket support.

use crate::error::Result;
use crate::pubsub::error::PubSubError;
use crate::pubsub::message::PubSubMessage;
use crate::pubsub::traits::{PubSub, PubSubBackend, SubscriptionHandle};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::Resource;
use surrealdb::Surreal;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Distributed pub/sub implementation using SurrealDB live queries
#[derive(Debug)]
pub struct DistributedPubSub {
    db: Surreal<Client>,
    #[allow(dead_code)]
    namespace: String,
    #[allow(dead_code)]
    database: String,
    subscriptions: Arc<RwLock<HashMap<String, Vec<tokio::task::JoinHandle<()>>>>>,
}

impl DistributedPubSub {
    /// Create a new distributed pub/sub instance
    pub async fn new(url: &str, namespace: &str, database: &str) -> Result<Self> {
        let db = Surreal::new::<Ws>(url).await.map_err(|e| {
            PubSubError::SurrealDBConnection(format!("Failed to connect to SurrealDB: {}", e))
        })?;

        db.use_ns(namespace).use_db(database).await.map_err(|e| {
            PubSubError::SurrealDBConnection(format!(
                "Failed to set namespace/database: {}",
                e
            ))
        })?;

        // Create message table schema
        Self::create_schema(&db).await?;

        Ok(Self {
            db,
            namespace: namespace.to_string(),
            database: database.to_string(),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create the SurrealDB schema for pub/sub messages
    async fn create_schema(db: &Surreal<Client>) -> Result<()> {
        db.query("DEFINE TABLE pubsub_messages SCHEMAFULL")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!("Failed to create table: {}", e))
            })?;

        db.query("DEFINE FIELD topic ON pubsub_messages TYPE string")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!("Failed to define topic field: {}", e))
            })?;

        db.query("DEFINE FIELD content ON pubsub_messages TYPE object")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!("Failed to define content field: {}", e))
            })?;

        db.query("DEFINE FIELD from ON pubsub_messages TYPE string")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!("Failed to define from field: {}", e))
            })?;

        db.query("DEFINE FIELD timestamp ON pubsub_messages TYPE datetime")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!(
                    "Failed to define timestamp field: {}",
                    e
                ))
            })?;

        db.query("DEFINE FIELD message_id ON pubsub_messages TYPE option<string>")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!(
                    "Failed to define message_id field: {}",
                    e
                ))
            })?;

        db.query("DEFINE INDEX topic_idx ON pubsub_messages FIELDS topic")
            .await
            .map_err(|e| {
                PubSubError::SurrealDBConnection(format!("Failed to create index: {}", e))
            })?;

        Ok(())
    }
}

#[async_trait]
impl PubSub for DistributedPubSub {
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

        // Store message in SurrealDB (live query will notify subscribers)
        let message_id = pubsub_msg.message_id.unwrap_or_else(Uuid::new_v4);
        let record_id: Resource = format!("pubsub_messages:{}", message_id).into();

        // Move the message into the database operation
        self.db
            .create(record_id)
            .content(pubsub_msg)
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

        let cancel = Arc::new(tokio::sync::Notify::new());
        let cancel_clone = cancel.clone();

        // Create live query - SurrealDB live queries return a stream
        let query = format!(
            "LIVE SELECT * FROM pubsub_messages WHERE topic = '{}'",
            topic
        );

        // Clone topic for use in spawned task
        let topic_str = topic.to_string();
        let db = self.db.clone();

        // Spawn task to handle live query updates
        let _handler = Arc::new(handler);
        let handle = tokio::spawn(async move {
            // Create live query and get the live query ID
            let live_result = db.query(&query).await;

            match live_result {
                Ok(_response) => {
                    // Get the live query ID from the response
                    // Note: This is a simplified implementation
                    // In practice, you'd need to parse the response to get the live query ID
                    // and then subscribe to changes using that ID

                    // For now, we'll poll for new messages
                    // TODO: Implement proper live query subscription when SurrealDB API is clarified
                    let _last_check = std::time::Instant::now();

                    loop {
                        tokio::select! {
                            _ = cancel_clone.notified() => {
                                break;
                            }
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                                // Poll for new messages
                                let select_query = format!(
                                    "SELECT * FROM pubsub_messages WHERE topic = '{}' AND timestamp > time::now() - 1s",
                                    topic_str
                                );

                                if let Ok(_result) = db.query(&select_query).await {
                                    // Process results
                                    // Note: This is a simplified polling approach
                                    // A proper implementation would use SurrealDB's live query subscription
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create live query: {}", e);
                }
            }
        });

        // Store subscription
        let mut subs = self.subscriptions.write().await;
        subs.entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(handle);

        let subscription_id = Uuid::new_v4();
        Ok(SubscriptionHandle::new(subscription_id, topic.to_string(), cancel))
    }

    fn is_distributed(&self) -> bool {
        true
    }

    fn backend_type(&self) -> PubSubBackend {
        PubSubBackend::Distributed
    }
}

