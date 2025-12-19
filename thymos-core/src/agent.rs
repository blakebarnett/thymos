//! Core agent implementation
//!
//! This module provides the `Agent` type - a user-friendly API for building
//! autonomous agents with memory, tools, and execution capabilities.
//!
//! For advanced use cases requiring AutoAgents integration, see the `integration`
//! module which provides `ThymosAgentCore` with full `AgentDeriveT`, `AgentHooks`,
//! and `AgentExecutor` trait implementations.

use crate::concepts::ConceptExtractor;
use crate::config::{MemoryConfig, ThymosConfig};
use crate::embeddings::providers::EmbeddingProvider;
use crate::error::{Result, ThymosError};
use crate::integration::{
    ThymosAgentConfig, ThymosAgentCore, ThymosExecutor, ThymosExecutorOutput,
};
use crate::llm::LLMProvider;
use crate::memory::MemorySystem;
use crate::pubsub::{PubSub, PubSubInstance, SubscriptionHandle};
use crate::tools::{CapabilityPolicy, Tool};
use autoagents_core::agent::task::Task;
use autoagents_core::agent::{AgentExecutor, Context};
use autoagents_llm::LLMProvider as AutoAgentsLLMProvider;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Agent with memory, state, and lifecycle management.
///
/// Agent provides a high-level API for building autonomous agents with:
/// - Semantic memory (via Locai)
/// - Tool execution with capability-based security
/// - Task execution via AutoAgents patterns
/// - Pub/sub coordination
///
/// # Example
///
/// ```rust,ignore
/// let agent = Agent::builder()
///     .id("research_agent")
///     .description("An agent that performs research")
///     .with_memory_config(config)
///     .tool(search_tool)
///     .tool(browse_tool)
///     .policy(CapabilityPolicy::allow_all())
///     .build()
///     .await?;
///
/// // Execute a task
/// let result = agent.execute("Find information about Rust", llm).await?;
/// ```
#[derive(Clone)]
pub struct Agent {
    /// Agent unique identifier
    pub id: String,

    /// Agent description (for LLM context)
    description: String,

    /// Agent memory system
    memory: Arc<MemorySystem>,

    /// Current agent state
    state: Arc<tokio::sync::RwLock<AgentState>>,

    /// LLM provider (optional - for Thymos LLM abstraction)
    llm_provider: Option<Arc<dyn LLMProvider>>,

    /// Embedding provider (optional)
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,

    /// Concept extractor (optional)
    concept_extractor: Option<Arc<dyn ConceptExtractor>>,

    /// Pub/sub instance (optional)
    pubsub: Option<Arc<PubSubInstance>>,

    /// Tools available to this agent
    tools: Vec<Arc<dyn Tool>>,

    /// Capability policy for tool execution
    policy: CapabilityPolicy,

    /// Agent configuration for execution
    agent_config: ThymosAgentConfig,
}

/// Agent state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Current status
    pub status: AgentStatus,

    /// When agent was started
    pub started_at: Option<DateTime<Utc>>,

    /// Last activity timestamp
    pub last_active: DateTime<Utc>,

    /// Custom properties (extensible)
    pub properties: serde_json::Value,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is actively running and responding
    Active,

    /// Agent is running but passive (listening only)
    Listening,

    /// Agent is stopped, state saved
    Dormant,

    /// Agent is archived (long-term storage)
    Archived,
}

impl Agent {
    /// Create a new agent builder
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Get agent ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get agent description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get agent memory system
    pub fn memory(&self) -> &MemorySystem {
        &self.memory
    }

    /// Get the arc'd memory system (for sharing with ThymosAgentCore)
    pub fn memory_arc(&self) -> Arc<MemorySystem> {
        Arc::clone(&self.memory)
    }

    /// Get current agent state
    pub async fn state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// Get current agent status
    pub async fn status(&self) -> AgentStatus {
        self.state.read().await.status
    }

    /// Update agent status
    pub async fn set_status(&self, status: AgentStatus) -> Result<()> {
        let mut state = self.state.write().await;
        state.status = status;
        state.last_active = Utc::now();
        Ok(())
    }

    /// Get the tools registered with this agent
    pub fn tools(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    /// Get the capability policy
    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    /// Store a memory
    pub async fn remember(&self, content: impl Into<String>) -> Result<String> {
        self.memory.remember(content.into()).await
    }

    /// Store a fact memory (semantic fact, durable knowledge)
    ///
    /// Facts are intended for durable, context-independent knowledge
    /// like "Paris is the capital of France".
    pub async fn remember_fact(&self, content: impl Into<String>) -> Result<String> {
        self.memory.remember_fact(content.into()).await
    }

    /// Store a conversation memory (dialogue context)
    ///
    /// Conversation memories are intended for dialogue history
    /// and ephemeral context.
    pub async fn remember_conversation(&self, content: impl Into<String>) -> Result<String> {
        self.memory.remember_conversation(content.into()).await
    }

    /// Store a memory with additional options (tags, priority, embedding, etc.)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use thymos_core::memory::{RememberOptions, MemoryTypeHint};
    ///
    /// let options = RememberOptions::new()
    ///     .with_tag("important")
    ///     .with_priority(10);
    ///
    /// agent.remember_with_options("Critical information", options).await?;
    /// ```
    pub async fn remember_with_options(
        &self,
        content: impl Into<String>,
        options: crate::memory::RememberOptions,
    ) -> Result<String> {
        self.memory
            .remember_with_options(content.into(), options)
            .await
    }

    /// Search memories
    pub async fn search_memories(&self, query: &str) -> Result<Vec<locai::models::Memory>> {
        self.memory.search(query, None).await
    }

    /// Search memories with scope (hybrid mode only)
    pub async fn search_memories_with_scope(
        &self,
        query: &str,
        scope: crate::memory::SearchScope,
    ) -> Result<Vec<locai::models::Memory>> {
        self.memory.search_with_scope(query, scope, None).await
    }

    /// Search private memories (hybrid mode only)
    pub async fn search_private(&self, query: &str) -> Result<Vec<locai::models::Memory>> {
        self.memory
            .search_with_scope(query, crate::memory::SearchScope::Private, None)
            .await
    }

    /// Search shared memories (hybrid mode only)
    pub async fn search_shared(&self, query: &str) -> Result<Vec<locai::models::Memory>> {
        self.memory
            .search_with_scope(query, crate::memory::SearchScope::Shared, None)
            .await
    }

    /// Store a memory in private backend (hybrid mode only)
    pub async fn remember_private(&self, content: impl Into<String>) -> Result<String> {
        self.memory.remember_private(content.into()).await
    }

    /// Store a memory in shared backend (hybrid mode only)
    pub async fn remember_shared(&self, content: impl Into<String>) -> Result<String> {
        self.memory.remember_shared(content.into()).await
    }

    /// Store a memory with optional embedding
    pub async fn remember_with_embedding(
        &self,
        content: impl Into<String>,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        self.memory
            .remember_with_embedding(content.into(), embedding)
            .await
    }

    /// Store a memory in private backend with optional embedding (hybrid mode only)
    pub async fn remember_private_with_embedding(
        &self,
        content: impl Into<String>,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        self.memory
            .remember_private_with_embedding(content.into(), embedding)
            .await
    }

    /// Store a memory in shared backend with optional embedding (hybrid mode only)
    pub async fn remember_shared_with_embedding(
        &self,
        content: impl Into<String>,
        embedding: Option<Vec<f32>>,
    ) -> Result<String> {
        self.memory
            .remember_shared_with_embedding(content.into(), embedding)
            .await
    }

    /// Search memories with hybrid search options
    ///
    /// For hybrid search, provide a query embedding (1024 dimensions).
    /// Locai uses RRF (Reciprocal Rank Fusion) automatically for hybrid search.
    pub async fn search_memories_hybrid(
        &self,
        query: &str,
        query_embedding: Option<Vec<f32>>,
    ) -> Result<Vec<locai::models::Memory>> {
        use crate::memory::{SearchOptions, SearchStrategy};

        let options = SearchOptions {
            semantic_weight: None, // Locai uses RRF automatically
            strategy: query_embedding.as_ref().map(|_| SearchStrategy::Hybrid {
                semantic_weight: 0.3, // Informational only, Locai uses RRF
            }),
            query_embedding,
        };

        self.memory
            .search_with_options(query, None, Some(options))
            .await
    }

    /// Search memories using semantic similarity.
    ///
    /// If an embedding provider is configured, this will automatically generate
    /// a query embedding and perform hybrid search (BM25 + vector). Otherwise,
    /// it falls back to keyword-based search.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Automatically uses hybrid search if embedding provider is configured
    /// let results = agent.search_semantic("information about Rust").await?;
    /// ```
    pub async fn search_semantic(&self, query: &str) -> Result<Vec<locai::models::Memory>> {
        self.search_semantic_with_limit(query, None).await
    }

    /// Search memories semantically with a result limit.
    pub async fn search_semantic_with_limit(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<locai::models::Memory>> {
        use crate::memory::{SearchOptions, SearchStrategy};

        // Try to generate query embedding if we have an embedding provider
        let query_embedding = if let Some(provider) = &self.embedding_provider {
            match provider.embed(query).await {
                Ok(emb) => Some(emb),
                Err(e) => {
                    tracing::warn!("Failed to generate query embedding, falling back to keyword search: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let options = if query_embedding.is_some() {
            SearchOptions {
                semantic_weight: None,
                strategy: Some(SearchStrategy::Hybrid {
                    semantic_weight: 0.3,
                }),
                query_embedding,
            }
        } else {
            // Fall back to keyword search
            SearchOptions {
                semantic_weight: None,
                strategy: Some(SearchStrategy::Keyword),
                query_embedding: None,
            }
        };

        self.memory
            .search_with_options(query, limit, Some(options))
            .await
    }

    /// Get memory by ID
    pub async fn get_memory(&self, id: &str) -> Result<Option<locai::models::Memory>> {
        self.memory.get_memory(id).await
    }

    /// Get the LLM provider (if configured)
    pub fn llm_provider(&self) -> Option<&Arc<dyn LLMProvider>> {
        self.llm_provider.as_ref()
    }

    /// Get the embedding provider (if configured)
    pub fn embedding_provider(&self) -> Option<&Arc<dyn EmbeddingProvider>> {
        self.embedding_provider.as_ref()
    }

    /// Get the concept extractor (if configured)
    pub fn concept_extractor(&self) -> Option<&Arc<dyn ConceptExtractor>> {
        self.concept_extractor.as_ref()
    }

    /// Publish a message to a topic (requires pub/sub to be configured)
    pub async fn publish<M>(&self, topic: &str, message: M) -> Result<()>
    where
        M: serde::Serialize + Send + Sync + 'static,
    {
        if let Some(pubsub) = &self.pubsub {
            pubsub.publish(topic, message).await
        } else {
            Err(ThymosError::Configuration(
                "Pub/sub not available for this agent".to_string(),
            ))
        }
    }

    /// Subscribe to a topic with a message handler (requires pub/sub to be configured)
    pub async fn subscribe<M, F>(&self, topic: &str, handler: F) -> Result<SubscriptionHandle>
    where
        M: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
        F: Fn(M) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        if let Some(pubsub) = &self.pubsub {
            pubsub.subscribe(topic, handler).await
        } else {
            Err(ThymosError::Configuration(
                "Pub/sub not available for this agent".to_string(),
            ))
        }
    }

    /// Get the pub/sub instance (if configured)
    pub fn pubsub(&self) -> Option<&Arc<PubSubInstance>> {
        self.pubsub.as_ref()
    }

    /// Execute a task with this agent using the provided LLM.
    ///
    /// This creates a `ThymosAgentCore` internally and executes the task
    /// using AutoAgents' execution patterns.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The task prompt to execute
    /// * `llm` - The LLM provider to use for execution
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = agent.execute("Find information about Rust", llm).await?;
    /// println!("Response: {}", result.response);
    /// ```
    pub async fn execute(
        &self,
        prompt: impl Into<String>,
        llm: Arc<dyn AutoAgentsLLMProvider>,
    ) -> Result<ThymosExecutorOutput> {
        // Leak the name and description strings for 'static lifetime requirement
        // This is safe because these are typically long-lived strings
        let name: &'static str = Box::leak(self.id.clone().into_boxed_str());
        let description: &'static str = Box::leak(self.description.clone().into_boxed_str());

        // Create ThymosAgentCore from this agent
        let core = ThymosAgentCore::builder()
            .name(name)
            .description(description)
            .memory(Arc::clone(&self.memory))
            .tools(self.tools.clone())
            .policy(self.policy.clone())
            .config(self.agent_config.clone())
            .build()
            .map_err(|e| ThymosError::Configuration(e.to_string()))?;

        // Create executor
        let executor = ThymosExecutor::new(core);

        // Create task
        let task = Task::new(prompt.into());

        // Create context with LLM and tools
        let memory_provider = crate::integration::ThymosMemoryProvider::new(Arc::clone(&self.memory));
        let tool_adapters = crate::integration::tool_adapter::thymos_tools_to_autoagents(&self.tools);

        let context = Context::new(llm, None)
            .with_memory(Some(Arc::new(tokio::sync::Mutex::new(
                Box::new(memory_provider) as Box<dyn autoagents_core::agent::memory::MemoryProvider>,
            ))))
            .with_tools(tool_adapters)
            .with_config(autoagents_core::agent::AgentConfig {
                id: uuid::Uuid::new_v4(),
                name: self.id.clone(),
                description: self.description.clone(),
                output_schema: None,
            });

        // Execute
        executor
            .execute(&task, Arc::new(context))
            .await
            .map_err(|e| ThymosError::Agent(e.to_string()))
    }

    /// Convert this Agent into a ThymosAgentCore for advanced AutoAgents usage.
    ///
    /// This is useful when you need direct access to AutoAgents traits like
    /// `AgentDeriveT`, `AgentHooks`, or `AgentExecutor`.
    ///
    /// Note: The returned core shares the same memory system as this agent.
    pub fn into_core(self) -> Result<ThymosAgentCore> {
        // Leak the name and description strings for 'static lifetime requirement
        let name: &'static str = Box::leak(self.id.into_boxed_str());
        let description: &'static str = Box::leak(self.description.into_boxed_str());

        ThymosAgentCore::builder()
            .name(name)
            .description(description)
            .memory(self.memory)
            .tools(self.tools)
            .policy(self.policy)
            .config(self.agent_config)
            .build()
            .map_err(|e| ThymosError::Configuration(e.to_string()))
    }

    /// Create a ThymosAgentCore from this Agent without consuming it.
    ///
    /// The returned core shares the same memory system.
    pub fn to_core(&self) -> Result<ThymosAgentCore> {
        // Leak the name and description strings for 'static lifetime requirement
        let name: &'static str = Box::leak(self.id.clone().into_boxed_str());
        let description: &'static str = Box::leak(self.description.clone().into_boxed_str());

        ThymosAgentCore::builder()
            .name(name)
            .description(description)
            .memory(Arc::clone(&self.memory))
            .tools(self.tools.clone())
            .policy(self.policy.clone())
            .config(self.agent_config.clone())
            .build()
            .map_err(|e| ThymosError::Configuration(e.to_string()))
    }
}

/// Builder for Agent
pub struct AgentBuilder {
    id: Option<String>,
    description: Option<String>,
    memory_config: Option<MemoryConfig>,
    config: Option<ThymosConfig>,
    llm_provider: Option<Arc<dyn LLMProvider>>,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    concept_extractor: Option<Arc<dyn ConceptExtractor>>,
    pubsub: Option<Arc<PubSubInstance>>,
    tools: Vec<Arc<dyn Tool>>,
    policy: CapabilityPolicy,
    agent_config: ThymosAgentConfig,
}

impl AgentBuilder {
    /// Create a new agent builder
    pub fn new() -> Self {
        Self {
            id: None,
            description: None,
            memory_config: None,
            config: None,
            llm_provider: None,
            embedding_provider: None,
            concept_extractor: None,
            pubsub: None,
            tools: Vec::new(),
            policy: CapabilityPolicy::deny_all(),
            agent_config: ThymosAgentConfig::default(),
        }
    }

    /// Set agent ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set agent description (used for LLM system prompt)
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set memory configuration
    pub fn with_memory_config(mut self, config: MemoryConfig) -> Self {
        self.memory_config = Some(config);
        self
    }

    /// Set full Thymos configuration (will auto-create providers if configured)
    pub fn config(mut self, config: ThymosConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set LLM provider (Thymos abstraction)
    pub fn llm_provider(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.llm_provider = Some(provider);
        self
    }

    /// Set embedding provider
    pub fn embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }

    /// Set concept extractor
    pub fn concept_extractor(mut self, extractor: Arc<dyn ConceptExtractor>) -> Self {
        self.concept_extractor = Some(extractor);
        self
    }

    /// Set pub/sub instance
    pub fn with_pubsub(mut self, pubsub: Arc<PubSubInstance>) -> Self {
        self.pubsub = Some(pubsub);
        self
    }

    /// Add a tool to this agent
    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tools at once
    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Set the capability policy for tool execution
    pub fn policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set the agent execution configuration
    pub fn agent_config(mut self, config: ThymosAgentConfig) -> Self {
        self.agent_config = config;
        self
    }

    /// Set maximum turns for execution
    pub fn max_turns(mut self, max_turns: usize) -> Self {
        self.agent_config = self.agent_config.with_max_turns(max_turns);
        self
    }

    /// Build the agent
    pub async fn build(self) -> Result<Agent> {
        let id = self
            .id
            .ok_or_else(|| ThymosError::Configuration("Agent ID is required".to_string()))?;

        let description = self
            .description
            .unwrap_or_else(|| format!("Agent {}", id));

        // Use config if provided, otherwise use defaults
        let memory_config = if let Some(ref config) = self.config {
            config.memory.clone()
        } else {
            self.memory_config.unwrap_or_default()
        };

        // Create providers from config if not explicitly set
        let llm_provider = if self.llm_provider.is_some() {
            self.llm_provider
        } else if let Some(ref config) = self.config {
            crate::llm::LLMProviderFactory::from_config(config.llm.as_ref()).await?
        } else {
            None
        };

        let embedding_provider = if self.embedding_provider.is_some() {
            self.embedding_provider
        } else if let Some(ref config) = self.config {
            crate::embeddings::EmbeddingProviderFactory::from_config(config.embeddings.as_ref())
                .await?
        } else {
            None
        };

        // Initialize memory system
        let memory = MemorySystem::new(memory_config).await?;

        let state = AgentState {
            status: AgentStatus::Active,
            started_at: Some(Utc::now()),
            last_active: Utc::now(),
            properties: serde_json::json!({}),
        };

        Ok(Agent {
            id,
            description,
            memory: Arc::new(memory),
            state: Arc::new(tokio::sync::RwLock::new(state)),
            llm_provider,
            embedding_provider,
            concept_extractor: self.concept_extractor,
            pubsub: self.pubsub,
            tools: self.tools,
            policy: self.policy,
            agent_config: self.agent_config,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id("test_agent")
            .with_memory_config(config)
            .build()
            .await
            .expect("Failed to create agent");

        assert_eq!(agent.id(), "test_agent");
        assert_eq!(agent.status().await, AgentStatus::Active);
    }

    #[tokio::test]
    async fn test_agent_status_change() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id("test_agent")
            .with_memory_config(config)
            .build()
            .await
            .expect("Failed to create agent");

        agent
            .set_status(AgentStatus::Listening)
            .await
            .expect("Failed to set status");

        assert_eq!(agent.status().await, AgentStatus::Listening);
    }

    #[tokio::test]
    async fn test_agent_with_config() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

        let mut thymos_config = ThymosConfig::default();
        thymos_config.memory.mode = crate::config::MemoryMode::Embedded {
            data_dir: temp_dir.path().to_path_buf(),
        };

        let agent = Agent::builder()
            .id("config_agent")
            .config(thymos_config)
            .build()
            .await
            .expect("Failed to create agent");

        assert_eq!(agent.id(), "config_agent");
        assert_eq!(agent.status().await, AgentStatus::Active);
        // Providers should be None when not configured
        assert!(agent.llm_provider().is_none());
        assert!(agent.embedding_provider().is_none());
    }

    #[tokio::test]
    async fn test_agent_provider_accessors() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id("test_agent")
            .with_memory_config(config)
            .build()
            .await
            .expect("Failed to create agent");

        // Test provider accessors (should return None when not set)
        assert!(agent.llm_provider().is_none());
        assert!(agent.embedding_provider().is_none());
        assert!(agent.concept_extractor().is_none());
    }
}
