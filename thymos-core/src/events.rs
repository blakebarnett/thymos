//! Event system and hooks for memory operations
//!
//! This module provides a hook system for reacting to memory operations.
//! It includes both Thymos-native hooks and integration with Locai's hook system.
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::events::{HookRegistry, MemoryHook, LoggingHook};
//!
//! let mut registry = HookRegistry::new();
//! registry.register(Arc::new(LoggingHook));
//!
//! // Trigger hooks when memories change
//! registry.trigger_created(&memory).await?;
//! ```

use crate::error::Result;
use crate::pubsub::PubSub;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use locai::models::Memory;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Event emitted by the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event ID
    pub id: String,

    /// Event type
    pub event_type: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Event payload
    pub payload: serde_json::Value,

    /// Source (agent ID or system)
    pub source: String,

    /// Tags for filtering
    pub tags: Vec<String>,
}

/// Result type for hooks
pub type HookResult = Result<()>;

/// Trait for memory operation hooks
#[async_trait]
pub trait MemoryHook: Send + Sync {
    /// Called when a memory is created
    async fn on_memory_created(&self, _memory: &Memory) -> HookResult {
        Ok(())
    }

    /// Called when a memory is updated
    async fn on_memory_updated(&self, _memory: &Memory) -> HookResult {
        Ok(())
    }

    /// Called when a memory is accessed
    async fn on_memory_accessed(&self, _memory: &Memory) -> HookResult {
        Ok(())
    }

    /// Called when a memory is deleted
    async fn on_memory_deleted(&self, _memory_id: &str) -> HookResult {
        Ok(())
    }
}

/// Registry for managing hooks
pub struct HookRegistry {
    hooks: Vec<Arc<dyn MemoryHook>>,
}

impl HookRegistry {
    /// Create a new hook registry
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a hook
    pub fn register(&mut self, hook: Arc<dyn MemoryHook>) {
        self.hooks.push(hook);
    }

    /// Trigger memory created event
    pub async fn trigger_created(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_created(memory).await?;
        }
        Ok(())
    }

    /// Trigger memory updated event
    pub async fn trigger_updated(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_updated(memory).await?;
        }
        Ok(())
    }

    /// Trigger memory accessed event
    pub async fn trigger_accessed(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_accessed(memory).await?;
        }
        Ok(())
    }

    /// Trigger memory deleted event
    pub async fn trigger_deleted(&self, memory_id: &str) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_deleted(memory_id).await?;
        }
        Ok(())
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for event handlers
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: Event) -> Result<()>;
}

/// Example hook: Logging hook
pub struct LoggingHook;

#[async_trait]
impl MemoryHook for LoggingHook {
    async fn on_memory_created(&self, memory: &Memory) -> HookResult {
        tracing::info!(
            memory_id = %memory.id,
            content_length = memory.content.len(),
            "Memory created"
        );
        Ok(())
    }

    async fn on_memory_updated(&self, memory: &Memory) -> HookResult {
        tracing::info!(memory_id = %memory.id, "Memory updated");
        Ok(())
    }

    async fn on_memory_accessed(&self, memory: &Memory) -> HookResult {
        tracing::debug!(memory_id = %memory.id, "Memory accessed");
        Ok(())
    }

    async fn on_memory_deleted(&self, memory_id: &str) -> HookResult {
        tracing::info!(memory_id = %memory_id, "Memory deleted");
        Ok(())
    }
}

/// A hook that extracts concepts from memory content.
///
/// This hook integrates with Thymos's concept extraction system to automatically
/// identify entities, locations, and other concepts when memories are created.
pub struct ConceptExtractionHook {
    extractor: Arc<dyn crate::concepts::ConceptExtractor>,
}

impl ConceptExtractionHook {
    /// Create a new concept extraction hook.
    pub fn new(extractor: Arc<dyn crate::concepts::ConceptExtractor>) -> Self {
        Self { extractor }
    }
}

#[async_trait]
impl MemoryHook for ConceptExtractionHook {
    async fn on_memory_created(&self, memory: &Memory) -> HookResult {
        match self.extractor.extract(&memory.content, None).await {
            Ok(concepts) => {
                if !concepts.is_empty() {
                    tracing::debug!(
                        memory_id = %memory.id,
                        concept_count = concepts.len(),
                        "Extracted concepts from memory"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    memory_id = %memory.id,
                    error = %e,
                    "Failed to extract concepts"
                );
            }
        }
        Ok(())
    }
}

/// A hook that forwards events to a pub/sub topic.
///
/// This enables distributed event handling by publishing memory events
/// to a pub/sub topic that other agents can subscribe to.
pub struct PubSubForwardingHook {
    pubsub: Arc<crate::pubsub::PubSubInstance>,
    topic: String,
}

impl PubSubForwardingHook {
    /// Create a new forwarding hook.
    pub fn new(pubsub: Arc<crate::pubsub::PubSubInstance>, topic: impl Into<String>) -> Self {
        Self {
            pubsub,
            topic: topic.into(),
        }
    }
}

#[async_trait]
impl MemoryHook for PubSubForwardingHook {
    async fn on_memory_created(&self, memory: &Memory) -> HookResult {
        let event = serde_json::json!({
            "event_type": "memory_created",
            "memory_id": memory.id,
            "content_preview": memory.content.chars().take(100).collect::<String>(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        if let Err(e) = self.pubsub.publish(&self.topic, event).await {
            tracing::warn!(
                topic = %self.topic,
                error = %e,
                "Failed to forward memory event to pub/sub"
            );
        }
        Ok(())
    }

    async fn on_memory_deleted(&self, memory_id: &str) -> HookResult {
        let event = serde_json::json!({
            "event_type": "memory_deleted",
            "memory_id": memory_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        if let Err(e) = self.pubsub.publish(&self.topic, event).await {
            tracing::warn!(
                topic = %self.topic,
                error = %e,
                "Failed to forward memory deletion event"
            );
        }
        Ok(())
    }
}

/// A composite hook that combines multiple hooks.
///
/// This is useful for creating hook chains or grouping related hooks.
pub struct CompositeHook {
    hooks: Vec<Arc<dyn MemoryHook>>,
}

impl CompositeHook {
    /// Create a new composite hook.
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Add a hook to the composite.
    pub fn add(mut self, hook: Arc<dyn MemoryHook>) -> Self {
        self.hooks.push(hook);
        self
    }
}

impl Default for CompositeHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryHook for CompositeHook {
    async fn on_memory_created(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_created(memory).await?;
        }
        Ok(())
    }

    async fn on_memory_updated(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_updated(memory).await?;
        }
        Ok(())
    }

    async fn on_memory_accessed(&self, memory: &Memory) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_accessed(memory).await?;
        }
        Ok(())
    }

    async fn on_memory_deleted(&self, memory_id: &str) -> HookResult {
        for hook in &self.hooks {
            hook.on_memory_deleted(memory_id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locai::models::MemoryBuilder;

    struct TestHook {
        called: Arc<tokio::sync::Mutex<bool>>,
    }

    #[async_trait]
    impl MemoryHook for TestHook {
        async fn on_memory_created(&self, _memory: &Memory) -> HookResult {
            let mut called = self.called.lock().await;
            *called = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_hook_registry() {
        let called = Arc::new(tokio::sync::Mutex::new(false));
        let hook = Arc::new(TestHook {
            called: called.clone(),
        });

        let mut registry = HookRegistry::new();
        registry.register(hook);

        let memory = MemoryBuilder::new_with_content("test").build();

        registry
            .trigger_created(&memory)
            .await
            .expect("Hook failed");

        assert!(*called.lock().await);
    }
}
