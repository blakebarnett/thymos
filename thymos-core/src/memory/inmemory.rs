//! In-memory backend for testing and lightweight deployments
//!
//! This backend stores memories in a HashMap with simple keyword search.
//! It's useful for:
//!
//! - Unit testing
//! - WASM offline mode
//! - Quick prototyping
//! - Lightweight deployments without persistence needs

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::Utc;

use super::backend::{MemoryBackend, MemoryRecord, QueryOptions, StoreOptions};
use crate::error::Result;

/// In-memory backend for testing and lightweight use
pub struct InMemoryBackend {
    memories: RwLock<HashMap<String, MemoryRecord>>,
    next_id: AtomicU64,
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self {
            memories: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Clear all memories
    pub fn clear(&self) {
        let mut memories = self.memories.write().unwrap();
        memories.clear();
        self.next_id.store(1, Ordering::SeqCst);
    }

    /// Generate a new memory ID
    fn generate_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("mem_{}", id)
    }

    /// Simple keyword-based scoring for search
    fn score_match(content: &str, query: &str) -> f64 {
        let content_lower = content.to_lowercase();
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        if query_terms.is_empty() {
            return 0.0;
        }

        let mut matches = 0;
        for term in &query_terms {
            if content_lower.contains(term) {
                matches += 1;
            }
        }

        matches as f64 / query_terms.len() as f64
    }
}

#[async_trait]
impl MemoryBackend for InMemoryBackend {
    async fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String> {
        let id = self.generate_id();
        let timestamp = Utc::now().to_rfc3339();

        let mut properties = serde_json::Map::new();

        if let Some(opts) = options {
            if let Some(memory_type) = opts.memory_type {
                properties.insert("type".to_string(), serde_json::json!(memory_type));
            }
            if !opts.tags.is_empty() {
                properties.insert("tags".to_string(), serde_json::json!(opts.tags));
            }
            if let Some(priority) = opts.priority {
                properties.insert("priority".to_string(), serde_json::json!(priority));
            }
        }

        let record = MemoryRecord {
            id: id.clone(),
            content,
            created_at: timestamp,
            last_accessed: None,
            properties: serde_json::Value::Object(properties),
            score: None,
        };

        let mut memories = self.memories.write().unwrap();
        memories.insert(id.clone(), record);

        Ok(id)
    }

    async fn search(
        &self,
        query: &str,
        options: Option<QueryOptions>,
    ) -> Result<Vec<MemoryRecord>> {
        let limit = options.and_then(|o| o.limit).unwrap_or(10);

        let memories = self.memories.read().unwrap();

        let mut scored: Vec<_> = memories
            .values()
            .map(|m| {
                let score = Self::score_match(&m.content, query);
                let mut record = m.clone();
                record.score = Some(score);
                (score, record)
            })
            .filter(|(score, _)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let results: Vec<MemoryRecord> = scored.into_iter().take(limit).map(|(_, m)| m).collect();

        Ok(results)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryRecord>> {
        let memories = self.memories.read().unwrap();
        Ok(memories.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let mut memories = self.memories.write().unwrap();
        Ok(memories.remove(id).is_some())
    }

    async fn count(&self) -> Result<u64> {
        let memories = self.memories.read().unwrap();
        Ok(memories.len() as u64)
    }

    async fn health_check(&self) -> Result<()> {
        // In-memory backend is always healthy
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get() {
        let backend = InMemoryBackend::new();

        let id = backend.store("Hello world".to_string(), None).await.unwrap();
        assert!(id.starts_with("mem_"));

        let record = backend.get(&id).await.unwrap();
        assert!(record.is_some());
        assert_eq!(record.unwrap().content, "Hello world");
    }

    #[tokio::test]
    async fn test_search() {
        let backend = InMemoryBackend::new();

        backend
            .store("The sky is blue".to_string(), None)
            .await
            .unwrap();
        backend
            .store("The grass is green".to_string(), None)
            .await
            .unwrap();
        backend
            .store("Water is wet".to_string(), None)
            .await
            .unwrap();

        let results = backend.search("sky blue", None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("sky"));
    }

    #[tokio::test]
    async fn test_delete() {
        let backend = InMemoryBackend::new();

        let id = backend.store("To be deleted".to_string(), None).await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 1);

        let deleted = backend.delete(&id).await.unwrap();
        assert!(deleted);
        assert_eq!(backend.count().await.unwrap(), 0);

        // Deleting again should return false
        let deleted_again = backend.delete(&id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_store_with_options() {
        let backend = InMemoryBackend::new();

        let options = StoreOptions {
            memory_type: Some("fact".to_string()),
            tags: vec!["important".to_string(), "science".to_string()],
            priority: Some(10),
            embedding: None,
        };

        let id = backend
            .store("Water boils at 100°C".to_string(), Some(options))
            .await
            .unwrap();

        let record = backend.get(&id).await.unwrap().unwrap();
        assert_eq!(record.properties["type"], "fact");
        assert_eq!(record.properties["priority"], 10);
    }

    #[tokio::test]
    async fn test_clear() {
        let backend = InMemoryBackend::new();

        backend.store("Memory 1".to_string(), None).await.unwrap();
        backend.store("Memory 2".to_string(), None).await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 2);

        backend.clear();
        assert_eq!(backend.count().await.unwrap(), 0);
    }
}

