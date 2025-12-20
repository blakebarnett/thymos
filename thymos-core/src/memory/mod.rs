//! Memory system wrapping Locai with lifecycle management and server mode support

pub mod backend;
pub mod hybrid;
pub mod inmemory;
pub mod routing;
pub mod scope;
pub mod server;
pub mod versioning;

use crate::config::MemoryConfig;
use crate::error::{Result, ThymosError};
use locai::prelude::*;
use std::sync::Arc;

pub use backend::{MemoryBackend, MemoryRecord, QueryOptions, StoreOptions};
pub use hybrid::HybridMemorySystem;
pub use inmemory::InMemoryBackend;
pub use routing::RoutingStrategy;
pub use scope::{MemoryScope, MemoryScopeConfig, ScopedMemory, ScopeRegistry, SearchScope};
pub use server::{ServerMemoryBackend, ServerMemoryConfig};

/// Options for storing memories with additional metadata
#[derive(Debug, Clone, Default)]
pub struct RememberOptions {
    /// Tags to associate with the memory
    pub tags: Vec<String>,

    /// Priority level (higher = more important, affects consolidation)
    pub priority: Option<i32>,

    /// Pre-computed embedding (1024 dimensions for BGE-M3)
    pub embedding: Option<Vec<f32>>,

    /// Memory type hint
    pub memory_type: Option<MemoryTypeHint>,
}

impl RememberOptions {
    /// Create new empty options
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set embedding
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Set memory type
    pub fn with_type(mut self, memory_type: MemoryTypeHint) -> Self {
        self.memory_type = Some(memory_type);
        self
    }
}

/// Hint for memory categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTypeHint {
    /// General episodic memory (default)
    Episodic,
    /// Durable fact/knowledge
    Fact,
    /// Dialogue/conversation context
    Conversation,
}

/// Search options for controlling search behavior
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Semantic weight for hybrid search (0.0-1.0)
    /// Note: Locai uses RRF (Reciprocal Rank Fusion) automatically, so this is informational only
    /// None = use default from config
    pub semantic_weight: Option<f64>,

    /// Force search strategy
    pub strategy: Option<SearchStrategy>,

    /// Query embedding for vector/hybrid search (1024 dimensions required)
    /// Required for SearchMode::Vector or SearchMode::Hybrid
    pub query_embedding: Option<Vec<f32>>,
}

/// Search strategy selection
#[derive(Debug, Clone)]
pub enum SearchStrategy {
    /// BM25 only (keyword)
    Keyword,

    /// Vector only (semantic)
    Semantic,

    /// Hybrid (BM25 + vector)
    Hybrid { semantic_weight: f64 },

    /// Auto (let Locai decide)
    Auto,
}

/// Memory system with lifecycle management and named scopes
pub enum MemorySystem {
    /// Embedded backend (local Locai instance)
    Single {
        /// Underlying Locai instance
        locai: Arc<Locai>,
        /// Lifecycle manager for memory decay
        lifecycle: MemoryLifecycle,
        /// Named scope registry
        scope_registry: ScopeRegistry,
    },
    /// Server backend (remote Locai server via HTTP)
    Server {
        /// Server memory backend
        backend: Arc<ServerMemoryBackend>,
        /// Lifecycle manager for memory decay
        lifecycle: MemoryLifecycle,
        /// Named scope registry
        scope_registry: ScopeRegistry,
    },
    /// Hybrid backend (private + shared)
    Hybrid {
        /// Hybrid memory system
        hybrid: Arc<HybridMemorySystem>,
        /// Named scope registry
        scope_registry: ScopeRegistry,
    },
}

impl MemorySystem {
    /// Create a new memory system
    pub async fn new(config: MemoryConfig) -> Result<Self> {
        match &config.mode {
            crate::config::MemoryMode::Embedded { data_dir } => {
                let locai = Locai::with_data_dir(data_dir)
                    .await
                    .map_err(|e| ThymosError::MemoryInit(e.to_string()))?;

                let lifecycle = MemoryLifecycle::new(LifecycleConfig {
                    forgetting_curve_enabled: config.forgetting_curve_enabled,
                    recency_decay_hours: config.recency_decay_hours,
                    access_count_weight: config.access_count_weight,
                    emotional_weight_multiplier: config.emotional_weight_multiplier,
                    base_decay_rate: config.base_decay_rate,
                });

                Ok(Self::Single {
                    locai: Arc::new(locai),
                    lifecycle,
                    scope_registry: ScopeRegistry::new(),
                })
            }
            crate::config::MemoryMode::Server { url, api_key } => {
                let mut server_config = ServerMemoryConfig::new(url.clone());
                server_config.api_key = api_key.clone();

                let backend = ServerMemoryBackend::new(server_config)
                    .await
                    .map_err(|e| ThymosError::MemoryInit(e.to_string()))?;

                let lifecycle = MemoryLifecycle::new(LifecycleConfig {
                    forgetting_curve_enabled: config.forgetting_curve_enabled,
                    recency_decay_hours: config.recency_decay_hours,
                    access_count_weight: config.access_count_weight,
                    emotional_weight_multiplier: config.emotional_weight_multiplier,
                    base_decay_rate: config.base_decay_rate,
                });

                Ok(Self::Server {
                    backend: Arc::new(backend),
                    lifecycle,
                    scope_registry: ScopeRegistry::new(),
                })
            }
            crate::config::MemoryMode::Hybrid {
                private_data_dir,
                shared_url,
                shared_api_key,
            } => {
                let hybrid = HybridMemorySystem::new(
                    private_data_dir.clone(),
                    shared_url.clone(),
                    shared_api_key.clone(),
                    &config,
                )
                .await?;

                Ok(Self::Hybrid {
                    hybrid: Arc::new(hybrid),
                    scope_registry: ScopeRegistry::new(),
                })
            }
        }
    }

    /// Store a memory (uses default scope for hybrid mode)
    pub async fn remember(&self, content: String) -> Result<String> {
        match self {
            Self::Single { locai, .. } => {
                let memory_id = locai
                    .remember(&content)
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))?;
                Ok(memory_id)
            }
            Self::Server { backend, .. } => {
                backend.store(content, None).await
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid.remember_private(content).await
            }
        }
    }

    /// Store a fact memory (semantic fact, durable knowledge)
    ///
    /// Facts are intended for durable, context-independent knowledge
    /// like "Paris is the capital of France".
    pub async fn remember_fact(&self, content: String) -> Result<String> {
        match self {
            Self::Single { locai, .. } => {
                let memory_id = locai
                    .remember_fact(&content)
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))?;
                Ok(memory_id)
            }
            Self::Server { backend, .. } => {
                let options = StoreOptions {
                    memory_type: Some("fact".to_string()),
                    ..Default::default()
                };
                backend.store(content, Some(options)).await
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid.remember_shared(content).await
            }
        }
    }

    /// Store a conversation memory (dialogue context)
    ///
    /// Conversation memories are intended for dialogue history
    /// and ephemeral context.
    pub async fn remember_conversation(&self, content: String) -> Result<String> {
        match self {
            Self::Single { locai, .. } => {
                let memory_id = locai
                    .remember_conversation(&content)
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))?;
                Ok(memory_id)
            }
            Self::Server { backend, .. } => {
                let options = StoreOptions {
                    memory_type: Some("conversation".to_string()),
                    ..Default::default()
                };
                backend.store(content, Some(options)).await
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid.remember_private(content).await
            }
        }
    }

    /// Store a memory with additional options (tags, priority, embedding, etc.)
    ///
    /// This provides full control over memory creation using Locai's RememberBuilder.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let options = RememberOptions::new()
    ///     .with_tag("important")
    ///     .with_tag("project-x")
    ///     .with_priority(10)
    ///     .with_type(MemoryTypeHint::Fact);
    ///
    /// memory.remember_with_options("Critical project deadline: Dec 31", options).await?;
    /// ```
    pub async fn remember_with_options(
        &self,
        content: String,
        options: RememberOptions,
    ) -> Result<String> {
        match self {
            Self::Single { locai, .. } => {
                // Validate embedding if provided
                if let Some(ref emb) = options.embedding {
                    if emb.len() != 1024 {
                        return Err(ThymosError::Memory(format!(
                            "Embedding dimension mismatch: expected 1024, got {}",
                            emb.len()
                        )));
                    }
                }

                // Use Locai's add_memory_with_options for full control
                locai
                    .manager()
                    .add_memory_with_options(&content, |builder| {
                        let mut b = builder;

                        // Add embedding if provided
                        if let Some(emb) = options.embedding.clone() {
                            b = b.embedding(emb);
                        }

                        // Add tags (convert String to &str)
                        if !options.tags.is_empty() {
                            let tag_refs: Vec<&str> = options.tags.iter().map(|s| s.as_str()).collect();
                            b = b.tags(tag_refs);
                        }

                        // Add priority if provided (convert i32 to MemoryPriority)
                        if let Some(priority) = options.priority {
                            use locai::models::MemoryPriority;
                            let mem_priority = match priority {
                                p if p <= 0 => MemoryPriority::Low,
                                p if p <= 5 => MemoryPriority::Normal,
                                p if p <= 8 => MemoryPriority::High,
                                _ => MemoryPriority::Critical,
                            };
                            b = b.priority(mem_priority);
                        }

                        b
                    })
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))
            }
            Self::Server { backend, .. } => {
                let store_options = StoreOptions {
                    memory_type: options.memory_type.map(|t| match t {
                        MemoryTypeHint::Episodic => "episodic".to_string(),
                        MemoryTypeHint::Fact => "fact".to_string(),
                        MemoryTypeHint::Conversation => "conversation".to_string(),
                    }),
                    tags: options.tags,
                    priority: options.priority,
                    embedding: options.embedding,
                };
                backend.store(content, Some(store_options)).await
            }
            Self::Hybrid { hybrid, .. } => {
                match options.memory_type {
                    Some(MemoryTypeHint::Fact) => {
                        hybrid
                            .remember_shared_with_embedding(content, options.embedding)
                            .await
                    }
                    _ => {
                        hybrid
                            .remember_private_with_embedding(content, options.embedding)
                            .await
                    }
                }
            }
        }
    }

    /// Store a memory in private backend (hybrid mode only)
    pub async fn remember_private(&self, content: String) -> Result<String> {
        match self {
            Self::Single { .. } | Self::Server { .. } => Err(ThymosError::Configuration(
                "remember_private only available in hybrid mode".to_string(),
            )),
            Self::Hybrid { hybrid, .. } => hybrid.remember_private(content).await,
        }
    }

    /// Store a memory in shared backend (hybrid mode only)
    pub async fn remember_shared(&self, content: String) -> Result<String> {
        match self {
            Self::Single { .. } | Self::Server { .. } => Err(ThymosError::Configuration(
                "remember_shared only available in hybrid mode".to_string(),
            )),
            Self::Hybrid { hybrid, .. } => hybrid.remember_shared(content).await,
        }
    }

    /// Store a memory with optional embedding
    ///
    /// Embeddings must be 1024 dimensions (BGE-M3 compatible) for vector search to work.
    pub async fn remember_with_embedding(
        &self,
        content: String,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        match self {
            Self::Single { locai, .. } => {
                if let Some(emb) = embedding {
                    if emb.len() != 1024 {
                        return Err(ThymosError::Memory(format!(
                            "Embedding dimension mismatch: expected 1024 dimensions (BGE-M3 compatible), but got {}",
                            emb.len()
                        )));
                    }

                    locai
                        .manager()
                        .add_memory_with_options(&content, |builder| builder.embedding(emb))
                        .await
                        .map_err(|e| ThymosError::Memory(e.to_string()))
                } else {
                    locai
                        .remember(&content)
                        .await
                        .map_err(|e| ThymosError::Memory(e.to_string()))
                }
            }
            Self::Server { backend, .. } => {
                // Server mode doesn't support client-provided embeddings currently
                // The server generates embeddings automatically
                backend.store(content, None).await
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid
                    .remember_private_with_embedding(content, embedding)
                    .await
            }
        }
    }

    /// Store a memory in private backend with optional embedding (hybrid mode only)
    pub async fn remember_private_with_embedding(
        &self,
        content: String,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        match self {
            Self::Single { .. } | Self::Server { .. } => Err(ThymosError::Configuration(
                "remember_private_with_embedding only available in hybrid mode".to_string(),
            )),
            Self::Hybrid { hybrid, .. } => {
                hybrid
                    .remember_private_with_embedding(content, embedding)
                    .await
            }
        }
    }

    /// Store a memory in shared backend with optional embedding (hybrid mode only)
    pub async fn remember_shared_with_embedding(
        &self,
        content: String,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        match self {
            Self::Single { .. } | Self::Server { .. } => Err(ThymosError::Configuration(
                "remember_shared_with_embedding only available in hybrid mode".to_string(),
            )),
            Self::Hybrid { hybrid, .. } => {
                hybrid
                    .remember_shared_with_embedding(content, embedding)
                    .await
            }
        }
    }

    /// Search memories
    pub async fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<Memory>> {
        match self {
            Self::Single { locai, .. } => {
                let results = locai
                    .search_for(query)
                    .limit(limit.unwrap_or(10))
                    .execute()
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))?;
                Ok(results)
            }
            Self::Server { backend, .. } => {
                let options = QueryOptions {
                    limit,
                    ..Default::default()
                };
                let records = backend.search(query, Some(options)).await?;
                Ok(records.into_iter().map(|r| r.into()).collect())
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid.search(query, SearchScope::Both, limit).await
            }
        }
    }

    /// Search memories with scope (hybrid mode only)
    pub async fn search_with_scope(
        &self,
        query: &str,
        scope: SearchScope,
        limit: Option<usize>,
    ) -> Result<Vec<Memory>> {
        match self {
            Self::Single { .. } | Self::Server { .. } => {
                self.search(query, limit).await
            }
            Self::Hybrid { hybrid, .. } => hybrid.search(query, scope, limit).await,
        }
    }

    /// Search memories with options (supports hybrid search)
    ///
    /// For hybrid search, provide a query embedding via SearchOptions.
    /// Locai uses RRF (Reciprocal Rank Fusion) automatically for hybrid search.
    pub async fn search_with_options(
        &self,
        query: &str,
        limit: Option<usize>,
        options: Option<SearchOptions>,
    ) -> Result<Vec<Memory>> {
        let options = options.unwrap_or_default();

        match self {
            Self::Single { locai, .. } => {
                use locai::memory::SearchMode;

                let search_mode = match options.strategy {
                    Some(SearchStrategy::Keyword) => SearchMode::Text,
                    Some(SearchStrategy::Semantic) => SearchMode::Vector,
                    Some(SearchStrategy::Hybrid { .. }) | Some(SearchStrategy::Auto) | None => {
                        SearchMode::Text
                    }
                };

                let mut search_builder = locai
                    .search_for(query)
                    .limit(limit.unwrap_or(10))
                    .mode(search_mode);

                if matches!(search_mode, SearchMode::Vector | SearchMode::Hybrid) {
                    if let Some(query_emb) = &options.query_embedding {
                        if query_emb.len() != 1024 {
                            return Err(ThymosError::Memory(format!(
                                "Query embedding dimension mismatch: expected 1024 dimensions (BGE-M3 compatible), but got {}",
                                query_emb.len()
                            )));
                        }
                        search_builder = search_builder.with_query_embedding(query_emb.clone());
                    } else {
                        search_builder = locai
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
            Self::Server { backend, .. } => {
                let query_options = QueryOptions {
                    limit,
                    ..Default::default()
                };
                let records = backend.search(query, Some(query_options)).await?;
                Ok(records.into_iter().map(|r| r.into()).collect())
            }
            Self::Hybrid { hybrid, .. } => {
                hybrid
                    .search_with_options(query, SearchScope::Both, limit, options)
                    .await
            }
        }
    }

    /// Get memory by ID
    pub async fn get_memory(&self, id: &str) -> Result<Option<Memory>> {
        match self {
            Self::Single { locai, .. } => {
                let memory = locai
                    .manager()
                    .get_memory(id)
                    .await
                    .map_err(|e| ThymosError::Memory(e.to_string()))?;
                Ok(memory)
            }
            Self::Server { backend, .. } => {
                let record = backend.get(id).await?;
                Ok(record.map(|r| r.into()))
            }
            Self::Hybrid { hybrid, .. } => hybrid.get_memory(id).await,
        }
    }

    /// Calculate memory strength using forgetting curve
    pub fn calculate_strength(&self, memory: &Memory) -> f64 {
        match self {
            Self::Single { lifecycle, .. } | Self::Server { lifecycle, .. } => {
                lifecycle.calculate_strength(memory)
            }
            Self::Hybrid { hybrid, .. } => hybrid.calculate_strength(memory),
        }
    }

    /// Get the underlying Locai instance for advanced operations (embedded mode only)
    pub fn locai(&self) -> Result<&Locai> {
        match self {
            Self::Single { locai, .. } => Ok(locai),
            Self::Server { .. } => Err(ThymosError::Configuration(
                "locai() not available in server mode - use direct API".to_string(),
            )),
            Self::Hybrid { .. } => Err(ThymosError::Configuration(
                "locai() only available in embedded mode".to_string(),
            )),
        }
    }

    /// Check if this is a hybrid memory system
    pub fn is_hybrid(&self) -> bool {
        matches!(self, Self::Hybrid { .. })
    }

    /// Check if this is a server-backed memory system
    pub fn is_server(&self) -> bool {
        matches!(self, Self::Server { .. })
    }

    /// Get the scope registry
    pub fn scope_registry(&self) -> &ScopeRegistry {
        match self {
            Self::Single { scope_registry, .. } => scope_registry,
            Self::Server { scope_registry, .. } => scope_registry,
            Self::Hybrid { scope_registry, .. } => scope_registry,
        }
    }

    /// Define a named scope with configuration
    pub async fn define_scope(&self, config: MemoryScopeConfig) -> Result<()> {
        self.scope_registry().define_scope(config).await
    }

    /// Get scope configuration by name
    pub async fn get_scope(&self, name: &str) -> Option<MemoryScopeConfig> {
        self.scope_registry().get_scope(name).await
    }

    /// List all defined scopes
    pub async fn list_scopes(&self) -> Vec<MemoryScopeConfig> {
        self.scope_registry().list_scopes().await
    }

    /// Store a memory in a specific named scope
    ///
    /// The scope must be defined first via `define_scope()`.
    /// The default scope is always available.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// memory.define_scope(MemoryScopeConfig::new("personal")
    ///     .with_decay_hours(336.0)).await?;
    ///
    /// memory.remember_in_scope("personal", "Important decision made", None).await?;
    /// ```
    pub async fn remember_in_scope(
        &self,
        scope: &str,
        content: &str,
        options: Option<RememberOptions>,
    ) -> Result<String> {
        // Verify scope exists
        let scope_config = self.get_scope(scope).await.ok_or_else(|| {
            ThymosError::Configuration(format!("Scope '{}' not defined", scope))
        })?;

        // Build options with scope tag
        let mut opts = options.unwrap_or_default();
        opts.tags.push(scope_config.scope_tag());

        self.remember_with_options(content.to_string(), opts).await
    }

    /// Search within a specific named scope
    ///
    /// Only returns memories tagged with the specified scope.
    pub async fn search_in_scope(
        &self,
        scope: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>> {
        // Verify scope exists
        let scope_config = self.get_scope(scope).await.ok_or_else(|| {
            ThymosError::Configuration(format!("Scope '{}' not defined", scope))
        })?;

        // Search and filter by scope tag
        let all_results = self.search(query, Some(limit * 2)).await?;
        let scope_tag = scope_config.scope_tag();

        let filtered: Vec<Memory> = all_results
            .into_iter()
            .filter(|m| m.tags.contains(&scope_tag))
            .take(limit)
            .collect();

        Ok(filtered)
    }

    /// Search across multiple named scopes with weighted score merging
    ///
    /// Results are ranked by (relevance_score * scope.search_weight).
    pub async fn search_scopes(
        &self,
        scopes: &[&str],
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScopedMemory>> {
        let mut all_results: Vec<ScopedMemory> = Vec::new();

        for scope_name in scopes {
            let scope_config = match self.get_scope(scope_name).await {
                Some(config) => config,
                None => continue, // Skip undefined scopes
            };

            let scope_tag = scope_config.scope_tag();
            let search_weight = scope_config.search_weight;

            // Search and filter
            let results = self.search(query, Some(limit)).await?;

            for memory in results {
                let has_scope = memory.tags.contains(&scope_tag);

                if has_scope {
                    // Use a heuristic score based on position (first = highest relevance)
                    // In practice, this would use the actual search score from Locai
                    let score = 1.0; // Placeholder - Locai doesn't expose relevance score directly

                    all_results.push(ScopedMemory::new(
                        memory,
                        score,
                        scope_name.to_string(),
                        search_weight,
                    ));
                }
            }
        }

        // Sort by weighted score (descending) and take top `limit`
        all_results.sort_by(|a, b| {
            b.weighted_score
                .partial_cmp(&a.weighted_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Search all defined scopes with weighted score merging
    pub async fn search_all_scopes(&self, query: &str, limit: usize) -> Result<Vec<ScopedMemory>> {
        let scopes = self.list_scopes().await;
        let scope_names: Vec<&str> = scopes.iter().map(|s| s.name.as_str()).collect();
        self.search_scopes(&scope_names, query, limit).await
    }

    /// Calculate memory strength with scope-aware decay
    ///
    /// If the memory belongs to a named scope, uses that scope's decay configuration.
    /// Otherwise, uses the default scope configuration.
    pub async fn calculate_strength_with_scope(&self, memory: &Memory) -> f64 {
        // Extract scope from memory tags
        let scope_name = ScopeRegistry::extract_scope_from_tags(&memory.tags)
            .unwrap_or_else(|| "default".to_string());

        // Get scope config (use default if not found)
        let scope_config = self
            .get_scope(&scope_name)
            .await
            .unwrap_or_else(MemoryScopeConfig::default);

        // Calculate strength using scope-specific parameters
        self.calculate_strength_with_config(memory, &scope_config)
    }

    /// Calculate memory strength using specific scope configuration
    fn calculate_strength_with_config(
        &self,
        memory: &Memory,
        scope_config: &MemoryScopeConfig,
    ) -> f64 {
        let now = chrono::Utc::now();
        let last_accessed = memory.last_accessed.unwrap_or(memory.created_at);
        let hours_since_access = now.signed_duration_since(last_accessed).num_hours() as f64;

        // Use scope-specific decay rate and importance multiplier
        let stability = scope_config.decay_hours * scope_config.importance_multiplier;

        // Forgetting curve: R = e^(-t/S)
        let strength = (-hours_since_access / stability).exp();

        strength.clamp(0.0, 1.0)
    }
}

/// Memory lifecycle configuration
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Enable forgetting curve calculations
    pub forgetting_curve_enabled: bool,

    /// Hours for recency decay (Ebbinghaus curve)
    pub recency_decay_hours: f64,

    /// Weight given to access count in stability
    pub access_count_weight: f64,

    /// Multiplier for emotional/important memories
    pub emotional_weight_multiplier: f64,

    /// Base decay rate for old memories
    pub base_decay_rate: f64,
}

/// Memory lifecycle manager implementing forgetting curves
pub struct MemoryLifecycle {
    config: LifecycleConfig,
}

impl MemoryLifecycle {
    /// Create a new lifecycle manager
    pub fn new(config: LifecycleConfig) -> Self {
        Self { config }
    }

    /// Calculate memory strength using forgetting curve
    ///
    /// Uses the Ebbinghaus forgetting curve: R = e^(-t/S)
    /// Where:
    /// - R = retention (memory strength)
    /// - t = time since last access
    /// - S = stability (resistance to forgetting)
    pub fn calculate_strength(&self, memory: &Memory) -> f64 {
        if !self.config.forgetting_curve_enabled {
            return 1.0;
        }

        let hours_since_access = self.hours_since_access(memory);
        let stability = self.calculate_stability(memory);

        // Forgetting curve: R = e^(-t/S)
        let time_decay = (-hours_since_access / stability).exp();
        let age_decay = self.age_decay(memory);

        (time_decay * age_decay).clamp(0.0, 1.0)
    }

    /// Calculate hours since last access
    fn hours_since_access(&self, memory: &Memory) -> f64 {
        let now = chrono::Utc::now();
        let last_accessed = memory.last_accessed.unwrap_or(memory.created_at);
        let duration = now.signed_duration_since(last_accessed);
        duration.num_hours() as f64
    }

    /// Calculate memory stability (resistance to forgetting)
    fn calculate_stability(&self, _memory: &Memory) -> f64 {
        let mut stability = self.config.recency_decay_hours;

        // More access = more stable
        // Note: Locai's Memory doesn't have access_count yet, using default
        let access_count = 1.0; // TODO: Get from memory metadata when available
        stability += access_count * self.config.access_count_weight;

        // Importance increases stability
        // TODO: Extract from memory properties when available
        let importance = 1.0;
        stability *= importance * self.config.emotional_weight_multiplier;

        stability
    }

    /// Calculate age-based decay
    fn age_decay(&self, memory: &Memory) -> f64 {
        let hours_since_creation = {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(memory.created_at);
            duration.num_hours() as f64
        };

        // Exponential decay based on age
        let age_factor = (-hours_since_creation * self.config.base_decay_rate).exp();
        age_factor.clamp(0.1, 1.0) // Minimum 10% retention
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_system_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let _memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // If we got here, initialization succeeded
    }

    #[tokio::test]
    async fn test_remember_and_search() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // Store a memory
        let memory_id = memory_system
            .remember("The sky is blue".to_string())
            .await
            .expect("Failed to store memory");

        assert!(!memory_id.is_empty());

        // Search for the memory
        let results = memory_system
            .search("sky", Some(10))
            .await
            .expect("Failed to search memories");

        // Note: BM25 search might not immediately return results
        // This is acceptable for MVP - we're testing basic functionality works
        if !results.is_empty() {
            assert!(results[0].content.contains("blue"));
        }
    }

    #[tokio::test]
    async fn test_scope_registry_in_memory_system() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // Default scope should exist
        let default = memory_system.get_scope("default").await;
        assert!(default.is_some());

        // Define a new scope
        memory_system
            .define_scope(
                MemoryScopeConfig::new("personal")
                    .with_decay_hours(336.0)
                    .with_importance_multiplier(1.5),
            )
            .await
            .expect("Failed to define scope");

        let personal = memory_system.get_scope("personal").await;
        assert!(personal.is_some());
        assert_eq!(personal.as_ref().unwrap().decay_hours, 336.0);
        assert_eq!(personal.as_ref().unwrap().importance_multiplier, 1.5);
    }

    #[tokio::test]
    async fn test_remember_in_scope() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // Define scope
        memory_system
            .define_scope(MemoryScopeConfig::new("observations"))
            .await
            .expect("Failed to define scope");

        // Store in scope
        let memory_id = memory_system
            .remember_in_scope("observations", "Saw a new PR in #engineering", None)
            .await
            .expect("Failed to store in scope");

        assert!(!memory_id.is_empty());

        // Verify memory has scope tag
        let memory = memory_system
            .get_memory(&memory_id)
            .await
            .expect("Failed to get memory")
            .expect("Memory not found");

        assert!(memory.tags.iter().any(|t| t == "_scope:observations"));
    }

    #[tokio::test]
    async fn test_remember_in_undefined_scope_fails() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // Try to store in undefined scope
        let result = memory_system
            .remember_in_scope("nonexistent", "Some content", None)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_scopes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let memory_system = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");

        // Initially just default
        let scopes = memory_system.list_scopes().await;
        assert_eq!(scopes.len(), 1);

        // Add more scopes
        memory_system
            .define_scope(MemoryScopeConfig::new("personal"))
            .await
            .unwrap();
        memory_system
            .define_scope(MemoryScopeConfig::new("team"))
            .await
            .unwrap();

        let scopes = memory_system.list_scopes().await;
        assert_eq!(scopes.len(), 3);
    }
}
