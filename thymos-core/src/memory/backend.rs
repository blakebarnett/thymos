//! Memory backend trait for pluggable storage implementations
//!
//! This module defines the `MemoryBackend` trait that all memory storage
//! implementations must implement. This allows for:
//!
//! - Embedded mode (Locai with SurrealDB)
//! - Server mode (HTTP client to Locai server)
//! - In-memory mode (for testing and WASM offline use)
//!
//! The trait is designed to be async-friendly and works across different
//! runtime contexts including native and WASM.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Core memory record returned by backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Unique identifier
    pub id: String,

    /// Memory content
    pub content: String,

    /// ISO 8601 timestamp when created
    pub created_at: String,

    /// ISO 8601 timestamp when last accessed
    pub last_accessed: Option<String>,

    /// Additional properties as JSON
    #[serde(default)]
    pub properties: serde_json::Value,

    /// Optional relevance score (from search results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

/// Options for storing memories
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreOptions {
    /// Memory type hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,

    /// Tags to associate with the memory
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Priority level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,

    /// Pre-computed embedding (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

/// Options for searching memories
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryOptions {
    /// Maximum number of results
    pub limit: Option<usize>,

    /// Semantic weight for hybrid search (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_weight: Option<f64>,

    /// Search strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,

    /// Query embedding for vector search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_embedding: Option<Vec<f32>>,
}

/// The core memory backend trait
///
/// Implementors provide storage and retrieval of memories. This trait
/// is designed to be implementable for:
///
/// - Native backends (Locai embedded, HTTP client)
/// - WASM backends (wasi:http client, in-memory)
/// - Test backends (in-memory with full control)
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    /// Store a memory and return its ID
    async fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String>;

    /// Search memories by query
    async fn search(
        &self,
        query: &str,
        options: Option<QueryOptions>,
    ) -> Result<Vec<MemoryRecord>>;

    /// Get a specific memory by ID
    async fn get(&self, id: &str) -> Result<Option<MemoryRecord>>;

    /// Delete a memory by ID, returns true if it existed
    async fn delete(&self, id: &str) -> Result<bool>;

    /// Get total memory count
    async fn count(&self) -> Result<u64>;

    /// Health check - verify the backend is operational
    async fn health_check(&self) -> Result<()>;
}

impl From<MemoryRecord> for locai::models::Memory {
    fn from(record: MemoryRecord) -> Self {
        use chrono::{DateTime, Utc};
        use locai::models::MemoryBuilder;

        let created_at = record
            .created_at
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());

        let last_accessed = record
            .last_accessed
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());

        let mut memory = MemoryBuilder::new_with_content(&record.content).build();
        memory.id = record.id;
        memory.created_at = created_at;
        memory.last_accessed = last_accessed;
        memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_record_serialization() {
        let record = MemoryRecord {
            id: "mem_1".to_string(),
            content: "Test content".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_accessed: None,
            properties: serde_json::json!({"type": "episodic"}),
            score: Some(0.95),
        };

        let json = serde_json::to_string(&record).unwrap();
        let parsed: MemoryRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, record.id);
        assert_eq!(parsed.content, record.content);
    }
}

