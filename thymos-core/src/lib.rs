//! # Thymos - The Animating Spirit for Intelligent Agents
//!
//! Thymos (Θυμός) is a domain-agnostic agent framework for building autonomous agents with:
//! - Semantic memory (via embedded Locai)
//! - Temporal memory decay and lifecycle management
//! - Concept extraction and entity tracking
//! - Event-driven coordination
//! - Pub/sub messaging for agent-to-agent coordination
//! - Automatic agent lifecycle management based on relevance
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use thymos_core::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create an agent with embedded memory
//!     let agent = Agent::builder()
//!         .id("my_agent")
//!         .with_memory_config(MemoryConfig::default())
//!         .build()
//!         .await?;
//!     
//!     // Agent stores and retrieves memories
//!     agent.remember("Important information").await?;
//!     let memories = agent.search_memories("important").await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! Thymos is built on Locai for memory management and provides:
//! - **Agent lifecycle**: Automatic start/stop based on relevance
//! - **Memory decay**: Forgetting curves and importance scoring  
//! - **Event hooks**: React to memory changes and agent events
//! - **Concept tracking**: Domain-agnostic entity extraction
//! - **Multi-agent coordination**: Event-driven communication
//!
//! ## Feature Flags
//!
//! - `server-mode`: Connect to Locai server instead of embedded mode
//! - `pubsub`: Local pub/sub coordination using AutoAgents runtime
//! - `pubsub-distributed`: Distributed pub/sub with SurrealDB live queries
//! - `pubsub-full`: Hybrid pub/sub (local + distributed)

pub mod agent;
pub mod concepts;
pub mod config;
pub mod consolidation;
pub mod context;
pub mod conversation;
pub mod embeddings;
pub mod error;
pub mod events;
pub mod integration;
pub mod lifecycle;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod parsing;
pub mod patterns;
pub mod eval;
pub mod metrics;
pub mod pubsub;
pub mod replay;
pub mod skills;
pub mod tools;
pub mod tracing;
pub mod workflow;

/// Current library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Re-export commonly used types
pub mod prelude {
    pub use crate::agent::{Agent, AgentBuilder, AgentState, AgentStatus};
    pub use crate::concepts::{
        Alias, AliasExtractor, AliasProvenance, AliasType, BasicConceptExtractor, Concept,
        ConceptExtractionConfig, ConceptExtractor, ConceptMention, ConceptPromotionPipeline,
        ConceptTier, PromotionConfig, PromotionStats,
    };
    pub use crate::config::{
        EmbeddingProvider, EmbeddingsConfig, LLMProvider as LLMProviderType, LLMProviderConfig,
        MemoryConfig, ThymosConfig,
    };
    pub use crate::consolidation::{
        ConsolidationConfig, ConsolidationEngine, Insight, InsightType,
    };
    pub use crate::embeddings::{
        EmbeddingProvider as EmbeddingProviderTrait, EmbeddingProviderFactory,
    };
    pub use crate::error::{Result, ThymosError};
    pub use crate::events::{
        CompositeHook, ConceptExtractionHook, Event, EventHandler, HookRegistry, LoggingHook,
        MemoryHook, PubSubForwardingHook,
    };
    pub use crate::lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceScore};
    pub use crate::llm::{
        LLMConfig, LLMProvider, LLMProviderFactory, LLMRequest, LLMResponse, Message, MessageRole,
    };
    pub use crate::memory::{
        HybridMemorySystem, MemoryLifecycle, MemoryScope, MemorySystem, MemoryTypeHint,
        RememberOptions, RoutingStrategy, SearchOptions, SearchScope, SearchStrategy,
    };

    pub use crate::memory::versioning::{
        MemoryBranch, MemoryCommit, MemoryRepository, MemoryWorktree, MemoryWorktreeManager,
        Subagent, SubagentConfig, SubagentMergeResult, SubagentResult,
    };
    pub use crate::metrics::{
        AgentMetrics, LLMCostCalculator, MetricsCollector, MetricsStorage,
        InMemoryMetricsStorage, MemoryPerformanceMetrics, PerformanceCriteria, PerformanceTrend,
        PerformanceWeights, QualityPerformanceMetrics, ResourceMonitor, ResourcePerformanceMetrics,
        ResponsePerformanceMetrics, StubResourceMonitor, TaskPerformanceMetrics,
    };

    pub use crate::tools::{
        BoxedTool, Capability, CapabilityPolicy, CapabilitySet, DiscoveryResult, DiscoveryStrategy,
        McpToolInfo, PolicyDecision, RateLimitConfig, RegistryError, SubstringDiscovery,
        Tool, ToolContext, ToolError, ToolErrorKind, ToolExample, ToolExecutionContext,
        ToolHandler, ToolMetadata, ToolProvenance, ToolRegistry, ToolResult, ToolResultEnvelope,
        ToolRuntime, ToolRuntimeConfig, ToolSchema, ToolSummary, ToolWarning, ValidationError,
    };

    pub use crate::skills::{
        create_memory_skill, format_skills_for_prompt, is_valid_hyphen_case, to_hyphen_case,
        AnthropicSkillFrontmatter, MemorySearchTool, MemoryStoreTool, PromptTemplate, Skill,
        SkillBuilder, SkillError,
    };

    pub use crate::pubsub::{
        PubSub, PubSubBackend, PubSubBuilder, PubSubInstance, PubSubMessage, PubSubMode,
        SubscriptionHandle,
    };

    pub use crate::integration::{
        event_channel, AgentEvent, BasicAgent, EventEmitter, EventReceiver, EventSender, ReActAgent,
        SingleThreadedRuntime, Task, ThymosActorAgentBuilder, ThymosActorAgentExt,
        ThymosActorAgentHandle, ThymosAgentConfig, ThymosAgentCore, ThymosAgentCoreBuilder,
        ThymosAgentExecutor, ThymosAgentOutput, ThymosBasicAgent, ThymosExecutor,
        ThymosExecutorConfig, ThymosExecutorError, ThymosExecutorOutput, ThymosMemoryProvider,
        ThymosReActAgent, ThymosToolAdapter, Topic, ToolCallSummary,
    };

    pub use crate::mcp::{
        ContentBlock, JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpPrompt, McpResource,
        McpServer, McpServerBuilder, McpServerConfig, McpTool, MemoryResource, PromptRole,
        RequestId, ResourceProvider, ServerCapabilities, ServerInfo, StdioTransport, Transport,
    };

    pub use crate::workflow::{
        Aggregator, AllSuccess, Attempt, BestResult, Branch, BranchTrace, Chain, ChainBuilder,
        ChainConfig, Classification, Classifier, Evaluation, Evaluator, EvaluatorOptimizer,
        EvaluatorOptimizerBuilder, EvaluatorOptimizerConfig, EvaluatorOptimizerTrace,
        ExecutionTrace, FirstSuccess, Gate, GateCondition, Generator, KeywordClassifier,
        LLMClassifier, LLMEvaluator, LLMGenerator, LLMPlanner, Merge, Orchestrator,
        OrchestratorBuilder, OrchestratorTrace, Parallel, ParallelBuilder, ParallelConfig,
        ParallelExecutionTrace, Plan, Planner, Route, RouteHandler, Router, RouterBuilder,
        RouterExecutionTrace, RuleClassifier, StaticPlanner, Step, StepBuilder, StepOutput,
        StepTrace, StepType, SubTask, TaskResult, TokenUsageTrace, Voting, Worker, WorkerHandler,
        WorkflowError, WorkflowResult,
    };

    pub use crate::parsing::{
        JsonParser, MarkdownParser, MarkdownSection, OutputParser, ParseError, ParseResult,
        ReActParser, ReActStep, ReActStepType,
    };

    pub use crate::context::{
        ContextConfig, ContextManager, ContextState, ContextTurnResult, GroundedContext,
        QualityScorer, QualityScorerConfig, RollbackResult, SummarizationResult,
    };

    pub use crate::conversation::{
        ConversationSession, FirstLastStrategy, MessageHistory, SessionMetadata, SessionState,
        SlidingWindowStrategy, SummaryStrategy, TruncationStrategy, Turn,
    };

    pub use crate::tracing::{
        AggregatedTrace, SamplingDecision, SamplingStrategy, TraceAggregator, TraceCollector,
        TraceExporter, TraceFormat, TraceReport, TraceSampler, TraceSummary,
    };

    pub use crate::patterns::{
        ApproachEvaluator, ApproachResult, BisectRegression, BisectResult, BisectTrace,
        ConsensusMerge, ConsensusConfig, ConsensusResult, ConsensusStrategy, EvaluationScore,
        IsolatedBranchResult, LLMApproachEvaluator, ParallelIsolated, ParallelIsolatedBuilder,
        SpeculativeExecution, SpeculativeExecutionBuilder, SpeculativeTrace,
    };
}
