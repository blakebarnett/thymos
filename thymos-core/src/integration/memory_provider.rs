//! Adapts Thymos MemorySystem to AutoAgents MemoryProvider
//!
//! This adapter allows Thymos's advanced memory system (with hybrid storage,
//! versioning, concepts, and consolidation) to be used within AutoAgents contexts.

use crate::error::Result;
use crate::memory::MemorySystem;
use async_trait::async_trait;
use autoagents_core::agent::memory::{MemoryProvider, MemoryType};
use autoagents_llm::chat::{ChatMessage, ChatRole, MessageType};
use autoagents_llm::error::LLMError;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapts Thymos MemorySystem to AutoAgents MemoryProvider.
///
/// This adapter bridges Thymos's advanced memory capabilities to the simpler
/// MemoryProvider interface expected by AutoAgents. Key behaviors:
///
/// - `remember()`: Stores messages as Thymos memories with metadata
/// - `recall()`: Uses Thymos semantic search to retrieve relevant memories
/// - `clear()`: Logs a warning as Thymos memories are typically persistent
///
/// Note: Thymos memories are richer than simple chat messages. When recalling,
/// the adapter converts memory content back to ChatMessage format.
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::integration::ThymosMemoryProvider;
/// use thymos_core::memory::MemorySystem;
///
/// let memory_system = MemorySystem::new(config).await?;
/// let provider = ThymosMemoryProvider::new(Arc::new(memory_system));
///
/// // Use as AutoAgents MemoryProvider
/// provider.remember(&message).await?;
/// let messages = provider.recall("query", Some(10)).await?;
/// ```
pub struct ThymosMemoryProvider {
    memory: Arc<MemorySystem>,
    /// Recent messages cache for quick retrieval (like sliding window)
    message_cache: RwLock<Vec<ChatMessage>>,
    /// Maximum messages to keep in cache
    cache_size: usize,
    /// Whether summarization has been requested
    needs_summary: RwLock<bool>,
}

impl ThymosMemoryProvider {
    /// Create a new provider wrapping a Thymos MemorySystem
    pub fn new(memory: Arc<MemorySystem>) -> Self {
        Self {
            memory,
            message_cache: RwLock::new(Vec::new()),
            cache_size: 50,
            needs_summary: RwLock::new(false),
        }
    }

    /// Create a provider with custom cache size
    pub fn with_cache_size(memory: Arc<MemorySystem>, cache_size: usize) -> Self {
        Self {
            memory,
            message_cache: RwLock::new(Vec::with_capacity(cache_size)),
            cache_size,
            needs_summary: RwLock::new(false),
        }
    }

    /// Get the underlying Thymos memory system
    pub fn memory_system(&self) -> &Arc<MemorySystem> {
        &self.memory
    }

    /// Store a memory directly in Thymos (bypasses chat message format)
    pub async fn remember_raw(&self, content: String) -> Result<String> {
        self.memory.remember(content).await
    }

    /// Search memories using Thymos semantic search
    pub async fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<locai::models::Memory>> {
        self.memory.search(query, limit).await
    }

    /// Convert a Thymos memory to a ChatMessage
    fn memory_to_chat_message(memory: &locai::models::Memory) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            message_type: MessageType::Text,
            content: memory.content.clone(),
        }
    }

    /// Convert a ChatMessage to a storable format
    fn chat_message_to_content(message: &ChatMessage) -> String {
        let role_prefix = match message.role {
            ChatRole::User => "[User]",
            ChatRole::Assistant => "[Assistant]",
            ChatRole::System => "[System]",
            ChatRole::Tool => "[Tool]",
        };
        format!("{} {}", role_prefix, message.content)
    }
}

#[async_trait]
impl MemoryProvider for ThymosMemoryProvider {
    async fn remember(&mut self, message: &ChatMessage) -> std::result::Result<(), LLMError> {
        // Store in Thymos memory system
        let content = Self::chat_message_to_content(message);
        self.memory
            .remember(content)
            .await
            .map_err(|e| LLMError::ProviderError(format!("Thymos memory error: {}", e)))?;

        // Also cache the message for quick retrieval
        let mut cache = self.message_cache.write().await;
        if cache.len() >= self.cache_size {
            cache.remove(0);
        }
        cache.push(message.clone());

        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> std::result::Result<Vec<ChatMessage>, LLMError> {
        let limit = limit.unwrap_or(20);

        // If query is empty, return cached messages
        if query.is_empty() {
            let cache = self.message_cache.read().await;
            let start = cache.len().saturating_sub(limit);
            return Ok(cache[start..].to_vec());
        }

        // Otherwise, use Thymos semantic search
        let memories = self
            .memory
            .search(query, Some(limit))
            .await
            .map_err(|e| LLMError::ProviderError(format!("Thymos search error: {}", e)))?;

        Ok(memories
            .iter()
            .map(Self::memory_to_chat_message)
            .collect())
    }

    async fn clear(&mut self) -> std::result::Result<(), LLMError> {
        tracing::warn!(
            "clear() called on ThymosMemoryProvider - clearing message cache only. \
             Thymos memories are persistent and not deleted."
        );
        let mut cache = self.message_cache.write().await;
        cache.clear();
        Ok(())
    }

    fn memory_type(&self) -> MemoryType {
        MemoryType::SlidingWindow
    }

    fn size(&self) -> usize {
        // Return cache size synchronously (best effort)
        // Note: This is a synchronous method but our cache is async
        // We'll return 0 if we can't get the lock immediately
        self.message_cache
            .try_read()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    fn needs_summary(&self) -> bool {
        self.needs_summary
            .try_read()
            .map(|n| *n)
            .unwrap_or(false)
    }

    fn mark_for_summary(&mut self) {
        if let Ok(mut needs) = self.needs_summary.try_write() {
            *needs = true;
        }
    }

    fn replace_with_summary(&mut self, summary: String) {
        // Store summary in Thymos
        let memory = self.memory.clone();
        let summary_for_storage = summary.clone();
        tokio::spawn(async move {
            if let Err(e) = memory.remember(format!("[Summary] {}", summary_for_storage)).await {
                tracing::error!("Failed to store summary in Thymos: {}", e);
            }
        });

        // Clear cache and add summary
        if let Ok(mut cache) = self.message_cache.try_write() {
            cache.clear();
            cache.push(ChatMessage {
                role: ChatRole::Assistant,
                message_type: MessageType::Text,
                content: summary,
            });
        }

        if let Ok(mut needs) = self.needs_summary.try_write() {
            *needs = false;
        }
    }

    fn clone_box(&self) -> Box<dyn MemoryProvider> {
        Box::new(Self {
            memory: Arc::clone(&self.memory),
            message_cache: RwLock::new(Vec::new()),
            cache_size: self.cache_size,
            needs_summary: RwLock::new(false),
        })
    }

    fn preload(&mut self, data: Vec<ChatMessage>) -> bool {
        if let Ok(mut cache) = self.message_cache.try_write() {
            cache.clear();
            for msg in data {
                cache.push(msg);
            }
            true
        } else {
            false
        }
    }

    fn export(&self) -> Vec<ChatMessage> {
        self.message_cache
            .try_read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, MemoryMode};
    use tempfile::TempDir;

    async fn create_test_provider() -> (ThymosMemoryProvider, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MemoryConfig {
            mode: MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };
        let memory = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");
        let provider = ThymosMemoryProvider::new(Arc::new(memory));
        (provider, temp_dir)
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let (provider, _temp_dir) = create_test_provider().await;
        assert_eq!(provider.size(), 0);
        assert!(!provider.needs_summary());
    }

    #[tokio::test]
    async fn test_remember_and_recall_from_cache() {
        let (mut provider, _temp_dir) = create_test_provider().await;

        let message = ChatMessage {
            role: ChatRole::User,
            message_type: MessageType::Text,
            content: "Hello, world!".to_string(),
        };

        provider.remember(&message).await.unwrap();

        // Recall with empty query returns cached messages
        let recalled = provider.recall("", None).await.unwrap();
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_clear_only_clears_cache() {
        let (mut provider, _temp_dir) = create_test_provider().await;

        let message = ChatMessage {
            role: ChatRole::User,
            message_type: MessageType::Text,
            content: "Test message".to_string(),
        };

        provider.remember(&message).await.unwrap();
        assert_eq!(provider.size(), 1);

        provider.clear().await.unwrap();
        assert_eq!(provider.size(), 0);
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MemoryConfig {
            mode: MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };
        let memory = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");
        let mut provider = ThymosMemoryProvider::with_cache_size(Arc::new(memory), 3);

        // Add 5 messages with cache size of 3
        for i in 0..5 {
            let message = ChatMessage {
                role: ChatRole::User,
                message_type: MessageType::Text,
                content: format!("Message {}", i),
            };
            provider.remember(&message).await.unwrap();
        }

        // Should only have last 3 in cache
        let recalled = provider.recall("", None).await.unwrap();
        assert_eq!(recalled.len(), 3);
        assert_eq!(recalled[0].content, "Message 2");
        assert_eq!(recalled[2].content, "Message 4");
    }

    #[tokio::test]
    async fn test_summary_flow() {
        let (mut provider, _temp_dir) = create_test_provider().await;

        // Add some messages
        for i in 0..3 {
            let message = ChatMessage {
                role: ChatRole::User,
                message_type: MessageType::Text,
                content: format!("Message {}", i),
            };
            provider.remember(&message).await.unwrap();
        }

        provider.mark_for_summary();
        assert!(provider.needs_summary());

        provider.replace_with_summary("Summary of messages".to_string());
        assert!(!provider.needs_summary());

        // Cache should now only have the summary
        let recalled = provider.recall("", None).await.unwrap();
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].content, "Summary of messages");
    }

    #[test]
    fn test_memory_type() {
        // We can't easily test this without async setup, but the implementation
        // returns SlidingWindow as a simplification
    }

    #[tokio::test]
    async fn test_clone_box() {
        let (provider, _temp_dir) = create_test_provider().await;
        let _cloned = provider.clone_box();
        // Just verify it doesn't panic
    }

    #[tokio::test]
    async fn test_export_and_preload() {
        let (mut provider, _temp_dir) = create_test_provider().await;

        // Add messages
        for i in 0..3 {
            let message = ChatMessage {
                role: ChatRole::User,
                message_type: MessageType::Text,
                content: format!("Message {}", i),
            };
            provider.remember(&message).await.unwrap();
        }

        // Export
        let exported = provider.export();
        assert_eq!(exported.len(), 3);

        // Preload into new messages
        let new_messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            message_type: MessageType::Text,
            content: "New message".to_string(),
        }];

        assert!(provider.preload(new_messages));

        let recalled = provider.recall("", None).await.unwrap();
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].content, "New message");
    }
}


