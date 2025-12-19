# Unified Pub/Sub Abstraction Design

**Date**: January 2025  
**Status**: ✅ Implemented  
**Purpose**: Design a unified pub/sub API that abstracts AutoAgents (local) and SurrealDB (distributed) implementations

**Implementation**: Complete in `thymos-core/src/pubsub/`

---

## Design Goals

1. **Single API**: One unified interface regardless of backend
2. **Embeddable**: Works without external dependencies (local-only mode)
3. **Distributed**: Optional SurrealDB support for multi-process coordination
4. **Feature-Flagged**: SurrealDB support is optional and compile-time configurable
5. **Transparent**: Users don't need to know which backend is used

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Unified PubSub Trait                        │
│  ┌──────────────────────────────────────────────────┐   │
│  │  pub trait PubSub {                              │   │
│  │    async fn publish(&self, topic, message)       │   │
│  │    async fn subscribe(&self, topic, handler)     │   │
│  │  }                                                │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┴─────────────────┐
        │                                     │
        ▼                                     ▼
┌──────────────────┐              ┌──────────────────┐
│ LocalPubSub      │              │ HybridPubSub     │
│ (AutoAgents)     │              │ (AutoAgents +    │
│ Always available │              │  SurrealDB)      │
│                  │              │ Feature-flagged │
└──────────────────┘              └──────────────────┘
```

---

## API Design

### Core Trait

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
        F: Fn(M) -> std::pin::Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static;
    
    /// Check if distributed coordination is enabled
    fn is_distributed(&self) -> bool;
    
    /// Get backend type for debugging/monitoring
    fn backend_type(&self) -> PubSubBackend;
}

/// Handle to a subscription (can be used to unsubscribe)
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    id: uuid::Uuid,
    topic: String,
    // Internal: used to cancel subscription
    #[doc(hidden)]
    cancel: Arc<tokio::sync::Notify>,
}

impl SubscriptionHandle {
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
```

### Message Type

```rust
/// Standard pub/sub message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessage {
    /// Topic name
    pub topic: String,
    
    /// Message content (JSON)
    pub content: serde_json::Value,
    
    /// Sender agent ID
    pub from: String,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Optional message ID for deduplication
    #[serde(default)]
    pub message_id: Option<uuid::Uuid>,
    
    /// Optional correlation ID for request/response tracking
    #[serde(default)]
    pub correlation_id: Option<uuid::Uuid>,
}

impl PubSubMessage {
    pub fn new(topic: impl Into<String>, content: serde_json::Value, from: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            content,
            from: from.into(),
            timestamp: chrono::Utc::now(),
            message_id: Some(uuid::Uuid::new_v4()),
            correlation_id: None,
        }
    }
}
```

---

## Implementation: Local Pub/Sub (AutoAgents)

**Always available** - no feature flags needed

```rust
#[cfg(feature = "pubsub")]
mod local {
    use super::*;
    use autoagents_core::actor::{Topic, ActorMessage, CloneableMessage};
    use autoagents_core::runtime::{SingleThreadedRuntime, TypedRuntime};
    use ractor::ActorRef;
    use std::collections::HashMap;
    use std::sync::Arc;
    
    /// Local pub/sub implementation using AutoAgents
    #[derive(Debug, Clone)]
    pub struct LocalPubSub {
        runtime: Arc<SingleThreadedRuntime>,
        subscriptions: Arc<tokio::sync::RwLock<HashMap<String, Vec<SubscriptionHandle>>>>,
    }
    
    impl LocalPubSub {
        pub fn new() -> Result<Self> {
            let runtime = SingleThreadedRuntime::new(None);
            
            // Start runtime in background
            let runtime_clone = runtime.clone();
            tokio::spawn(async move {
                let _ = runtime_clone.run().await;
            });
            
            Ok(Self {
                runtime,
                subscriptions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            })
        }
    }
    
    #[async_trait]
    impl PubSub for LocalPubSub {
        async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
        where
            M: Serialize + Send + Sync + 'static,
        {
            // Convert to PubSubMessage for consistency
            let pubsub_msg = PubSubMessage::new(
                topic,
                serde_json::to_value(message)?,
                "unknown", // TODO: get from context
            );
            
            // Create AutoAgents topic and message
            let topic_typed = Topic::<PubSubMessage>::new(topic);
            
            // Publish to all subscribers via runtime
            // Note: This is simplified - actual implementation would need
            // to track subscribers and route messages
            self.runtime.publish(&topic_typed, pubsub_msg).await
                .map_err(|e| ThymosError::PubSubError(e.to_string()))?;
            
            Ok(())
        }
        
        async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
        where
            M: for<'de> Deserialize<'de> + Send + Sync + 'static,
            F: Fn(M) -> std::pin::Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static,
        {
            // Implementation would create an actor that handles messages
            // and calls the user's handler function
            // This is a simplified version
            
            let handle = SubscriptionHandle {
                id: uuid::Uuid::new_v4(),
                topic: topic.to_string(),
                cancel: Arc::new(tokio::sync::Notify::new()),
            };
            
            // Store subscription
            let mut subs = self.subscriptions.write().await;
            subs.entry(topic.to_string())
                .or_insert_with(Vec::new)
                .push(handle.clone());
            
            Ok(handle)
        }
        
        fn is_distributed(&self) -> bool {
            false
        }
        
        fn backend_type(&self) -> PubSubBackend {
            PubSubBackend::Local
        }
    }
}
```

---

## Implementation: Distributed Pub/Sub (SurrealDB)

**Feature-flagged** - only available with `pubsub-distributed` feature

```rust
#[cfg(feature = "pubsub-distributed")]
mod distributed {
    use super::*;
    use surrealdb::engine::remote::ws::{Ws, Client};
    use surrealdb::Surreal;
    use futures::StreamExt;
    
    /// Distributed pub/sub implementation using SurrealDB live queries
    #[derive(Debug)]
    pub struct DistributedPubSub {
        db: Surreal<Client>,
        namespace: String,
        database: String,
        subscriptions: Arc<tokio::sync::RwLock<HashMap<String, Vec<tokio::task::JoinHandle<()>>>>>,
    }
    
    impl DistributedPubSub {
        pub async fn new(url: &str, namespace: &str, database: &str) -> Result<Self> {
            let db = Surreal::new::<Ws>(url).await
                .map_err(|e| ThymosError::PubSubError(format!("Failed to connect to SurrealDB: {}", e)))?;
            
            db.use_ns(namespace).use_db(database).await
                .map_err(|e| ThymosError::PubSubError(format!("Failed to set namespace/database: {}", e)))?;
            
            // Create message table schema
            db.query("DEFINE TABLE pubsub_messages SCHEMAFULL").await?;
            db.query("DEFINE FIELD topic ON pubsub_messages TYPE string").await?;
            db.query("DEFINE FIELD content ON pubsub_messages TYPE object").await?;
            db.query("DEFINE FIELD from ON pubsub_messages TYPE string").await?;
            db.query("DEFINE FIELD timestamp ON pubsub_messages TYPE datetime").await?;
            db.query("DEFINE FIELD message_id ON pubsub_messages TYPE option<string>").await?;
            db.query("DEFINE INDEX topic_idx ON pubsub_messages FIELDS topic").await?;
            
            Ok(Self {
                db,
                namespace: namespace.to_string(),
                database: database.to_string(),
                subscriptions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            })
        }
    }
    
    #[async_trait]
    impl PubSub for DistributedPubSub {
        async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
        where
            M: Serialize + Send + Sync + 'static,
        {
            let pubsub_msg = PubSubMessage::new(
                topic,
                serde_json::to_value(message)?,
                "unknown", // TODO: get from context
            );
            
            // Store message in SurrealDB (live query will notify subscribers)
            self.db
                .create(("pubsub_messages", pubsub_msg.message_id.unwrap_or(uuid::Uuid::new_v4())))
                .content(&pubsub_msg)
                .await
                .map_err(|e| ThymosError::PubSubError(format!("Failed to publish: {}", e)))?;
            
            Ok(())
        }
        
        async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
        where
            M: for<'de> Deserialize<'de> + Send + Sync + 'static,
            F: Fn(M) -> std::pin::Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static,
        {
            let cancel = Arc::new(tokio::sync::Notify::new());
            let cancel_clone = cancel.clone();
            
            // Create live query
            let query = format!(
                "LIVE SELECT * FROM pubsub_messages WHERE topic = '{}'",
                topic
            );
            
            let mut stream = self.db.query(&query).await
                .map_err(|e| ThymosError::PubSubError(format!("Failed to create live query: {}", e)))?;
            
            // Spawn task to handle live query updates
            let handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_clone.notified() => {
                            break;
                        }
                        result = stream.next() => {
                            match result {
                                Some(Ok(record)) => {
                                    // Parse message and call handler
                                    if let Ok(msg) = serde_json::from_value::<PubSubMessage>(record) {
                                        if let Ok(content) = serde_json::from_value::<M>(msg.content) {
                                            if let Err(e) = handler(content).await {
                                                tracing::error!("Handler error: {}", e);
                                            }
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    tracing::error!("Live query error: {}", e);
                                    break;
                                }
                                None => {
                                    break;
                                }
                            }
                        }
                    }
                }
            });
            
            // Store subscription
            let mut subs = self.subscriptions.write().await;
            subs.entry(topic.to_string())
                .or_insert_with(Vec::new)
                .push(handle);
            
            Ok(SubscriptionHandle {
                id: uuid::Uuid::new_v4(),
                topic: topic.to_string(),
                cancel,
            })
        }
        
        fn is_distributed(&self) -> bool {
            true
        }
        
        fn backend_type(&self) -> PubSubBackend {
            PubSubBackend::Distributed
        }
    }
}
```

---

## Implementation: Hybrid Pub/Sub

**Feature-flagged** - combines local + distributed

```rust
#[cfg(all(feature = "pubsub", feature = "pubsub-distributed"))]
mod hybrid {
    use super::*;
    
    /// Hybrid pub/sub that uses both local (fast) and distributed (persistent)
    #[derive(Debug)]
    pub struct HybridPubSub {
        local: LocalPubSub,
        distributed: DistributedPubSub,
    }
    
    impl HybridPubSub {
        pub async fn new(distributed_url: Option<&str>) -> Result<Self> {
            let local = LocalPubSub::new()?;
            
            let distributed = if let Some(url) = distributed_url {
                DistributedPubSub::new(url, "thymos", "pubsub").await?
            } else {
                return Err(ThymosError::PubSubError(
                    "Hybrid mode requires distributed URL".to_string()
                ));
            };
            
            Ok(Self { local, distributed })
        }
    }
    
    #[async_trait]
    impl PubSub for HybridPubSub {
        async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
        where
            M: Serialize + Send + Sync + 'static,
        {
            // Publish to both backends
            let (local_result, distributed_result) = tokio::join!(
                self.local.publish(topic, message.clone()),
                self.distributed.publish(topic, message)
            );
            
            // Local failure is non-fatal, but log it
            if let Err(e) = local_result {
                tracing::warn!("Local pub/sub failed: {}", e);
            }
            
            // Distributed failure is more serious
            distributed_result?;
            
            Ok(())
        }
        
        async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
        where
            M: for<'de> Deserialize<'de> + Send + Sync + 'static,
            F: Fn(M) -> std::pin::Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static,
        {
            // Subscribe to both backends
            let handler_clone = Arc::new(handler);
            
            let local_handle = self.local.subscribe(topic, {
                let handler = handler_clone.clone();
                move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move { handler(msg).await })
                }
            }).await?;
            
            let distributed_handle = self.distributed.subscribe(topic, {
                let handler = handler_clone.clone();
                move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move { handler(msg).await })
                }
            }).await?;
            
            // Return combined handle
            Ok(SubscriptionHandle {
                id: uuid::Uuid::new_v4(),
                topic: topic.to_string(),
                cancel: Arc::new(tokio::sync::Notify::new()),
            })
        }
        
        fn is_distributed(&self) -> bool {
            true
        }
        
        fn backend_type(&self) -> PubSubBackend {
            PubSubBackend::Hybrid
        }
    }
}
```

---

## Builder Pattern

```rust
/// Builder for creating pub/sub instances
pub struct PubSubBuilder {
    mode: PubSubMode,
    distributed_url: Option<String>,
    namespace: Option<String>,
    database: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PubSubMode {
    /// Local-only (AutoAgents, always available)
    Local,
    
    /// Distributed-only (SurrealDB, requires feature flag)
    #[cfg(feature = "pubsub-distributed")]
    Distributed,
    
    /// Hybrid (both local and distributed)
    #[cfg(all(feature = "pubsub", feature = "pubsub-distributed"))]
    Hybrid,
}

impl PubSubBuilder {
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
    
    /// Use hybrid pub/sub (requires feature flags)
    #[cfg(all(feature = "pubsub", feature = "pubsub-distributed"))]
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
    pub async fn build(self) -> Result<Box<dyn PubSub>> {
        match self.mode {
            PubSubMode::Local => {
                Ok(Box::new(LocalPubSub::new()?))
            }
            
            #[cfg(feature = "pubsub-distributed")]
            PubSubMode::Distributed => {
                let url = self.distributed_url.ok_or_else(|| {
                    ThymosError::PubSubError("Distributed mode requires URL".to_string())
                })?;
                
                let namespace = self.namespace.unwrap_or_else(|| "thymos".to_string());
                let database = self.database.unwrap_or_else(|| "pubsub".to_string());
                
                Ok(Box::new(DistributedPubSub::new(&url, &namespace, &database).await?))
            }
            
            #[cfg(all(feature = "pubsub", feature = "pubsub-distributed"))]
            PubSubMode::Hybrid => {
                let url = self.distributed_url.ok_or_else(|| {
                    ThymosError::PubSubError("Hybrid mode requires distributed URL".to_string())
                })?;
                
                Ok(Box::new(HybridPubSub::new(Some(&url)).await?))
            }
        }
    }
}

impl Default for PubSubBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Feature Flags

### Cargo.toml

```toml
[features]
default = []

# Pub/sub support (local only, AutoAgents)
pubsub = ["dep:autoagents-core", "dep:ractor"]

# Distributed pub/sub support (SurrealDB)
pubsub-distributed = [
    "pubsub",
    "surrealdb/protocol-ws",
    "surrealdb/protocol-http",
]

# Full pub/sub with hybrid support
pubsub-full = ["pubsub-distributed"]
```

### Usage Examples

**Local-only (embeddable, no external deps)**:
```toml
[dependencies]
thymos-core = { version = "0.1.0", features = ["pubsub"] }
```

**Distributed (requires SurrealDB server)**:
```toml
[dependencies]
thymos-core = { version = "0.1.0", features = ["pubsub-distributed"] }
```

**Hybrid (best of both)**:
```toml
[dependencies]
thymos-core = { version = "0.1.0", features = ["pubsub-full"] }
```

---

## Usage Examples

### Example 1: Local-Only (Embeddable)

```rust
use thymos_core::pubsub::{PubSubBuilder, PubSub};

#[tokio::main]
async fn main() -> Result<()> {
    // Create local-only pub/sub (no external dependencies)
    let pubsub = PubSubBuilder::new()
        .local()
        .build()
        .await?;
    
    // Publish a message
    pubsub.publish("research", serde_json::json!({
        "finding": "New discovery",
        "agent": "researcher_1"
    })).await?;
    
    // Subscribe to topic
    pubsub.subscribe("research", |msg: serde_json::Value| {
        Box::pin(async move {
            println!("Received: {:?}", msg);
            Ok(())
        })
    }).await?;
    
    Ok(())
}
```

### Example 2: Distributed (Multi-Process)

```rust
#[cfg(feature = "pubsub-distributed")]
#[tokio::main]
async fn main() -> Result<()> {
    // Create distributed pub/sub (requires SurrealDB server)
    let pubsub = PubSubBuilder::new()
        .distributed("ws://localhost:8000")
        .namespace("thymos")
        .database("pubsub")
        .build()
        .await?;
    
    // Same API, but now works across processes
    pubsub.publish("research", serde_json::json!({
        "finding": "New discovery"
    })).await?;
    
    Ok(())
}
```

### Example 3: Hybrid (Best Performance)

```rust
#[cfg(feature = "pubsub-full")]
#[tokio::main]
async fn main() -> Result<()> {
    // Create hybrid pub/sub (local + distributed)
    let pubsub = PubSubBuilder::new()
        .hybrid("ws://localhost:8000")
        .build()
        .await?;
    
    // Fast local delivery + persistent distributed delivery
    pubsub.publish("research", serde_json::json!({
        "finding": "New discovery"
    })).await?;
    
    Ok(())
}
```

### Example 4: Integration with Agent

```rust
use thymos_core::agent::Agent;
use thymos_core::pubsub::{PubSubBuilder, PubSub};

pub struct AgentWithPubSub {
    agent: Agent,
    pubsub: Box<dyn PubSub>,
}

impl AgentWithPubSub {
    pub async fn new(agent_id: &str) -> Result<Self> {
        let agent = Agent::builder()
            .id(agent_id)
            .build()
            .await?;
        
        // Use local pub/sub by default (embeddable)
        let pubsub = PubSubBuilder::new()
            .local()
            .build()
            .await?;
        
        Ok(Self { agent, pubsub })
    }
    
    pub async fn coordinate(&self, topic: &str, message: serde_json::Value) -> Result<()> {
        // Publish to coordination topic
        self.pubsub.publish(topic, message).await?;
        Ok(())
    }
}
```

---

## Configuration Integration

### Config File Support

```rust
// In config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubConfig {
    /// Pub/sub mode
    pub mode: PubSubMode,
    
    /// SurrealDB URL (for distributed/hybrid modes)
    #[serde(default)]
    pub distributed_url: Option<String>,
    
    /// SurrealDB namespace
    #[serde(default = "default_namespace")]
    pub namespace: String,
    
    /// SurrealDB database
    #[serde(default = "default_database")]
    pub database: String,
}

fn default_namespace() -> String {
    "thymos".to_string()
}

fn default_database() -> String {
    "pubsub".to_string()
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            mode: PubSubMode::Local,
            distributed_url: None,
            namespace: default_namespace(),
            database: default_database(),
        }
    }
}
```

### TOML Configuration

```toml
[pubsub]
mode = "local"  # or "distributed" or "hybrid"

# For distributed/hybrid modes:
# distributed_url = "ws://localhost:8000"
# namespace = "thymos"
# database = "pubsub"
```

---

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum PubSubError {
    #[error("Pub/sub error: {0}")]
    PubSubError(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Distributed backend not available (feature flag missing)")]
    DistributedNotAvailable,
    
    #[error("SurrealDB connection error: {0}")]
    SurrealDBConnection(String),
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_local_pubsub() {
        let pubsub = PubSubBuilder::new().local().build().await.unwrap();
        
        let received = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();
        
        pubsub.subscribe("test", move |msg: serde_json::Value| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.lock().await.push(msg);
                Ok(())
            })
        }).await.unwrap();
        
        pubsub.publish("test", serde_json::json!({"data": "test"})).await.unwrap();
        
        // Wait for message
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        assert_eq!(received.lock().await.len(), 1);
    }
    
    #[cfg(feature = "pubsub-distributed")]
    #[tokio::test]
    async fn test_distributed_pubsub() {
        // Requires SurrealDB server running
        let pubsub = PubSubBuilder::new()
            .distributed("ws://localhost:8000")
            .build()
            .await
            .unwrap();
        
        // Test distributed pub/sub
    }
}
```

---

## Migration Path

### Phase 1: Local Only
- Implement `LocalPubSub` with AutoAgents
- Add `pubsub` feature flag
- Integrate with Agent

### Phase 2: Distributed Support
- Add `DistributedPubSub` with SurrealDB
- Add `pubsub-distributed` feature flag
- Test multi-process coordination

### Phase 3: Hybrid Mode
- Combine local + distributed
- Optimize for performance
- Add monitoring/metrics

---

## Summary

**Key Design Decisions**:

1. ✅ **Single API**: One `PubSub` trait regardless of backend
2. ✅ **Feature-Flagged**: SurrealDB optional via `pubsub-distributed`
3. ✅ **Embeddable**: Local-only mode requires no external deps
4. ✅ **Transparent**: Users don't need to know backend details
5. ✅ **Builder Pattern**: Easy to construct with different modes
6. ✅ **Configuration**: TOML config support for all modes

**Benefits**:
- Embeddable solution (local-only)
- Distributed coordination when needed (SurrealDB)
- Best performance (hybrid mode)
- Single API for all use cases
- Compile-time feature flags

