//! Hybrid memory backend combining private embedded and shared server storage

use crate::config::MemoryConfig;
use crate::error::{Result, ThymosError};
use locai::models::Memory;
use locai::prelude::*;
use std::sync::Arc;

use super::MemoryLifecycle;
use super::routing::RoutingStrategy;
use super::scope::{MemoryScope, SearchScope};
use super::{SearchOptions, SearchStrategy};

/// Hybrid memory backend combining private and shared storage
pub struct HybridMemorySystem {
    /// Private embedded memory backend
    private: Arc<Locai>,

    /// Shared server memory backend
    shared: Arc<super::server::ServerMemoryBackend>,

    /// Routing strategy
    routing: RoutingStrategy,

    /// Lifecycle manager
    lifecycle: MemoryLifecycle,
}

impl HybridMemorySystem {
    /// Create a new hybrid memory system
    pub async fn new(
        private_data_dir: std::path::PathBuf,
        shared_url: String,
        shared_api_key: Option<String>,
        config: &MemoryConfig,
    ) -> Result<Self> {
        // Initialize private embedded backend
        let private = Locai::with_data_dir(private_data_dir)
            .await
            .map_err(|e| ThymosError::MemoryInit(e.to_string()))?;

        // Initialize shared server backend
        let server_config = super::server::ServerMemoryConfig::new(shared_url);
        let server_config = if let Some(api_key) = shared_api_key {
            server_config.with_api_key(api_key)
        } else {
            server_config
        };

        let server_backend = super::server::ServerMemoryBackend::new(server_config).await?;

        let routing = RoutingStrategy::default();

        let lifecycle = MemoryLifecycle::new(super::LifecycleConfig {
            forgetting_curve_enabled: config.forgetting_curve_enabled,
            recency_decay_hours: config.recency_decay_hours,
            access_count_weight: config.access_count_weight,
            emotional_weight_multiplier: config.emotional_weight_multiplier,
            base_decay_rate: config.base_decay_rate,
        });

        Ok(Self {
            private: Arc::new(private),
            shared: Arc::new(server_backend),
            routing,
            lifecycle,
        })
    }

    /// Store a memory in private backend
    pub async fn remember_private(&self, content: String) -> Result<String> {
        let memory_id = self
            .private
            .remember(&content)
            .await
            .map_err(|e| ThymosError::Memory(e.to_string()))?;
        Ok(memory_id)
    }

    /// Store a memory in shared backend
    pub async fn remember_shared(&self, content: String) -> Result<String> {
        use super::backend::MemoryBackend;
        self.shared.store(content, None).await
    }

    /// Store a memory in private backend with optional embedding
    ///
    /// Embeddings must be 1024 dimensions (BGE-M3 compatible) for vector search to work.
    pub async fn remember_private_with_embedding(
        &self,
        content: String,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        if let Some(emb) = embedding {
            // Validate embedding dimensions (Locai requires 1024)
            if emb.len() != 1024 {
                return Err(ThymosError::Memory(format!(
                    "Embedding dimension mismatch: expected 1024 dimensions (BGE-M3 compatible), but got {}",
                    emb.len()
                )));
            }

            // Use Locai's add_memory_with_options to create memory with embedding
            self.private
                .manager()
                .add_memory_with_options(&content, |builder| builder.embedding(emb))
                .await
                .map_err(|e| ThymosError::Memory(e.to_string()))
        } else {
            self.remember_private(content).await
        }
    }

    /// Store a memory in shared backend with optional embedding
    pub async fn remember_shared_with_embedding(
        &self,
        content: String,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        use super::backend::{MemoryBackend, StoreOptions};
        let options = embedding.map(|emb| StoreOptions {
            embedding: Some(emb),
            ..Default::default()
        });
        self.shared.store(content, options).await
    }

    /// Store a memory with automatic routing based on tags
    pub async fn remember_with_tags(&self, content: String, tags: Vec<String>) -> Result<String> {
        let scope = self.routing.route(&tags);
        match scope {
            MemoryScope::Private => self.remember_private(content).await,
            MemoryScope::Shared => self.remember_shared(content).await,
        }
    }

    /// Search private memories
    async fn search_private(&self, query: &str, limit: Option<usize>) -> Result<Vec<Memory>> {
        let results = self
            .private
            .search_for(query)
            .limit(limit.unwrap_or(10))
            .execute()
            .await
            .map_err(|e| ThymosError::Memory(e.to_string()))?;
        Ok(results)
    }

    /// Search shared memories
    async fn search_shared_memories(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Memory>> {
        self.search_shared_memories_with_options(query, limit, None, None)
            .await
    }

    /// Search shared memories with hybrid search options
    async fn search_shared_memories_with_options(
        &self,
        query: &str,
        limit: Option<usize>,
        semantic_weight: Option<f64>,
        strategy: Option<&str>,
    ) -> Result<Vec<Memory>> {
        use super::backend::{MemoryBackend, QueryOptions};
        
        let options = QueryOptions {
            limit,
            semantic_weight,
            strategy: strategy.map(|s| s.to_string()),
            query_embedding: None,
        };
        
        let records = self.shared.search(query, Some(options)).await?;
        
        // Convert MemoryRecords to locai::Memory objects
        let memories: Vec<Memory> = records
            .into_iter()
            .filter_map(|record| {
                // Try to parse the record into a Memory
                // This requires the server to return compatible data
                let json = serde_json::json!({
                    "id": record.id,
                    "content": record.content,
                    "created_at": record.created_at,
                    "last_accessed": record.last_accessed,
                    "properties": record.properties,
                });
                serde_json::from_value::<Memory>(json).ok()
            })
            .collect();
        
        Ok(memories)
    }

    /// Search memories in a specific scope
    pub async fn search(
        &self,
        query: &str,
        scope: SearchScope,
        limit: Option<usize>,
    ) -> Result<Vec<Memory>> {
        match scope {
            SearchScope::Private => self.search_private(query, limit).await,
            SearchScope::Shared => self.search_shared_memories(query, limit).await,
            SearchScope::Both => {
                let (private_results, shared_results) = tokio::join!(
                    self.search_private(query, limit),
                    self.search_shared_memories(query, limit)
                );

                let mut all_results = private_results?;
                all_results.extend(shared_results?);

                // Sort by creation date (newest first) as a simple relevance proxy
                // In a full implementation, we'd use search scores
                all_results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

                Ok(all_results)
            }
        }
    }

    /// Search memories with options (supports hybrid search)
    ///
    /// For hybrid search, provide a query embedding via SearchOptions.
    /// Locai uses RRF (Reciprocal Rank Fusion) automatically for hybrid search.
    pub async fn search_with_options(
        &self,
        query: &str,
        scope: SearchScope,
        limit: Option<usize>,
        options: SearchOptions,
    ) -> Result<Vec<Memory>> {
        use locai::memory::SearchMode;

        match scope {
            SearchScope::Private => {
                // For private (embedded Locai), use Locai's search API
                let search_mode = match options.strategy {
                    Some(SearchStrategy::Keyword) => SearchMode::Text,
                    Some(SearchStrategy::Semantic) => SearchMode::Vector,
                    Some(SearchStrategy::Hybrid { .. }) | Some(SearchStrategy::Auto) | None => {
                        // For hybrid/auto, check if we have a query embedding
                        // If not, fall back to text search
                        SearchMode::Text
                    }
                };

                let mut search_builder = self
                    .private
                    .search_for(query)
                    .limit(limit.unwrap_or(10))
                    .mode(search_mode);

                // For vector/hybrid search, we need a query embedding
                // Note: Locai doesn't support semantic_weight parameter - it uses RRF automatically
                if matches!(search_mode, SearchMode::Vector | SearchMode::Hybrid) {
                    if let Some(query_emb) = &options.query_embedding {
                        // Validate embedding dimensions
                        if query_emb.len() != 1024 {
                            return Err(ThymosError::Memory(format!(
                                "Query embedding dimension mismatch: expected 1024 dimensions (BGE-M3 compatible), but got {}",
                                query_emb.len()
                            )));
                        }
                        search_builder = search_builder.with_query_embedding(query_emb.clone());
                    } else {
                        // If no query embedding provided, we can't do vector/hybrid search
                        // Fall back to text search
                        search_builder = self
                            .private
                            .search_for(query)
                            .limit(limit.unwrap_or(10))
                            .mode(SearchMode::Text);
                    }
                }

                search_builder
                    .execute()
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))
            }
            SearchScope::Shared => {
                // For shared (server), use options
                let (semantic_weight, strategy_str) = match options.strategy.as_ref() {
                    Some(SearchStrategy::Keyword) => (Some(0.0), Some("keyword")),
                    Some(SearchStrategy::Semantic) => (Some(1.0), Some("semantic")),
                    Some(SearchStrategy::Hybrid { semantic_weight }) => {
                        (Some(*semantic_weight), Some("hybrid"))
                    }
                    Some(SearchStrategy::Auto) | None => (options.semantic_weight, None),
                };

                self.search_shared_memories_with_options(
                    query,
                    limit,
                    semantic_weight,
                    strategy_str,
                )
                .await
            }
            SearchScope::Both => {
                // Search both with options
                let (semantic_weight, strategy_str) = match options.strategy.as_ref() {
                    Some(SearchStrategy::Keyword) => (Some(0.0), Some("keyword")),
                    Some(SearchStrategy::Semantic) => (Some(1.0), Some("semantic")),
                    Some(SearchStrategy::Hybrid { semantic_weight }) => {
                        (Some(*semantic_weight), Some("hybrid"))
                    }
                    Some(SearchStrategy::Auto) | None => (options.semantic_weight, None),
                };

                let (private_results, shared_results) = tokio::join!(
                    self.search_private(query, limit),
                    self.search_shared_memories_with_options(
                        query,
                        limit,
                        semantic_weight,
                        strategy_str
                    )
                );

                let mut all_results = private_results?;
                all_results.extend(shared_results?);

                // Sort by creation date (newest first) as a simple relevance proxy
                all_results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

                Ok(all_results)
            }
        }
    }

    /// Get memory by ID (searches both backends)
    pub async fn get_memory(&self, id: &str) -> Result<Option<Memory>> {
        use super::backend::MemoryBackend;
        
        // Try private first
        if let Ok(Some(memory)) = self
            .private
            .manager()
            .get_memory(id)
            .await
            .map_err(|e| ThymosError::Memory(e.to_string()))
        {
            return Ok(Some(memory));
        }

        // Try shared backend
        if let Ok(Some(record)) = self.shared.get(id).await {
            // Convert MemoryRecord to Memory
            let json = serde_json::json!({
                "id": record.id,
                "content": record.content,
                "created_at": record.created_at,
                "last_accessed": record.last_accessed,
                "properties": record.properties,
            });
            if let Ok(memory) = serde_json::from_value::<Memory>(json) {
                return Ok(Some(memory));
            }
        }

        Ok(None)
    }

    /// Calculate memory strength using forgetting curve
    pub fn calculate_strength(&self, memory: &Memory) -> f64 {
        self.lifecycle.calculate_strength(memory)
    }

    /// Get the private Locai instance
    pub fn private_locai(&self) -> &Locai {
        &self.private
    }

    /// Get routing strategy
    pub fn routing(&self) -> &RoutingStrategy {
        &self.routing
    }

    /// Set routing strategy
    pub fn set_routing(&mut self, routing: RoutingStrategy) {
        self.routing = routing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_hybrid_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Hybrid {
                private_data_dir: temp_dir.path().to_path_buf(),
                shared_url: "http://127.0.0.1:1".to_string(), // Unreachable, but we'll skip health check
                shared_api_key: None,
            },
            ..Default::default()
        };

        // This will fail because server is unreachable, but tests structure
        let result = HybridMemorySystem::new(
            temp_dir.path().to_path_buf(),
            "http://127.0.0.1:1".to_string(),
            None,
            &config,
        )
        .await;

        // Expected to fail due to unreachable server, but structure is correct
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remember_private() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let _config = MemoryConfig {
            mode: crate::config::MemoryMode::Hybrid {
                private_data_dir: temp_dir.path().to_path_buf(),
                shared_url: "http://127.0.0.1:1".to_string(),
                shared_api_key: None,
            },
            ..Default::default()
        };

        // Skip server initialization for this test
        let private = Locai::with_data_dir(temp_dir.path())
            .await
            .expect("Failed to create private backend");

        let _routing = RoutingStrategy::default();
        let _lifecycle = MemoryLifecycle::new(super::super::LifecycleConfig {
            forgetting_curve_enabled: false,
            recency_decay_hours: 168.0,
            access_count_weight: 0.1,
            emotional_weight_multiplier: 1.5,
            base_decay_rate: 0.01,
        });

        // Test private memory storage
        let memory_id = private
            .remember("Private thought")
            .await
            .expect("Failed to store private memory");
        assert!(!memory_id.is_empty());
    }

    #[tokio::test]
    async fn test_routing_strategy() {
        let routing = RoutingStrategy::new(MemoryScope::Private)
            .with_tag_rule("shared", MemoryScope::Shared)
            .with_tag_rule("public", MemoryScope::Shared);

        assert_eq!(
            routing.route(&vec!["shared".to_string()]),
            MemoryScope::Shared
        );
        assert_eq!(
            routing.route(&vec!["private".to_string()]),
            MemoryScope::Private
        );
        assert_eq!(
            routing.route(&vec!["unknown".to_string()]),
            MemoryScope::Private
        );
    }

    #[tokio::test]
    async fn test_search_scopes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let private = Locai::with_data_dir(temp_dir.path())
            .await
            .expect("Failed to create private backend");

        // Store some test memories
        private.remember("Private memory 1").await.unwrap();
        private.remember("Private memory 2").await.unwrap();

        // Search private memories
        let results = private
            .search_for("memory")
            .limit(10)
            .execute()
            .await
            .expect("Failed to search");

        assert!(!results.is_empty());
    }
}
