//! AutoAgents-Core Deep Integration
//!
//! This module provides deep integration between Thymos and AutoAgents-core,
//! enabling Thymos agents to leverage AutoAgents' battle-tested execution patterns
//! (ReAct loops, hooks, actor lifecycle) while preserving Thymos's superior features.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    User-Facing API                          │
//! │  ┌──────────────────────────────────────────────────────┐  │
//! │  │ ThymosAgent (convenience wrapper)                     │  │
//! │  │   - .remember(), .search(), .tools(), .execute()     │  │
//! │  └──────────────────────────────────────────────────────┘  │
//! │                           ↓                                 │
//! │  ┌──────────────────────────────────────────────────────┐  │
//! │  │ ThymosAgentCore                                       │  │
//! │  │   implements: AgentDeriveT + AgentExecutor + AgentHooks│ │
//! │  │   contains: MemorySystem, ToolRegistry, Config        │  │
//! │  └──────────────────────────────────────────────────────┘  │
//! │                           ↓                                 │
//! │  ┌────────────────────┐  ┌────────────────────────────┐   │
//! │  │ ThymosToolAdapter  │  │ ThymosMemoryProvider       │   │
//! │  │  Tool → ToolT      │  │ MemorySystem → MemoryProvider│  │
//! │  └────────────────────┘  └────────────────────────────┘   │
//! │                           ↓                                 │
//! │  ┌──────────────────────────────────────────────────────┐  │
//! │  │ AutoAgents Runtime (when using ActorAgent)           │  │
//! │  └──────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Components
//!
//! - [`ThymosToolAdapter`]: Bridges Thymos's capability-based `Tool` to AutoAgents' `ToolT`
//! - [`ThymosMemoryProvider`]: Adapts Thymos's `MemorySystem` to AutoAgents' `MemoryProvider`
//! - [`ThymosAgentCore`]: Core agent implementing AutoAgents' `AgentDeriveT`, `AgentHooks`, `AgentExecutor`
//! - [`ThymosExecutor`]: Generic executor wrapper with turn-based execution and streaming
//!
//! # Executors
//!
//! Thymos provides multiple executor options:
//!
//! - [`ThymosExecutor<T>`]: Generic wrapper for any `AgentDeriveT + AgentHooks` with Thymos patterns
//! - [`ThymosAgentExecutor`]: Type alias for `ThymosExecutor<ThymosAgentCore>`
//! - [`ThymosReActAgent`]: Type alias for using AutoAgents' ReAct executor with `ThymosAgentCore`
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::integration::{ThymosAgentCore, ThymosAgentConfig, ThymosExecutor};
//!
//! let agent = ThymosAgentCore::builder()
//!     .name("research_agent")
//!     .description("An agent that performs research tasks")
//!     .memory(memory_system)
//!     .tools(vec![search_tool, browse_tool])
//!     .build()?;
//!
//! // Option 1: Use ThymosExecutor for Thymos-style execution
//! let executor = ThymosExecutor::new(agent);
//! let result = executor.execute(&task, context).await?;
//!
//! // Option 2: Use ReActAgent for AutoAgents ReAct-style execution
//! let react_agent = ThymosReActAgent::new(agent);
//! let result = react_agent.execute(&task, context).await?;
//! ```

mod actor;
mod agent_core;
mod config;
mod events;
mod executor;
mod memory_provider;
pub mod tool_adapter;

// Core types
pub use agent_core::{
    ThymosAgentCore, ThymosAgentCoreBuilder, ThymosAgentError, ThymosAgentErrorKind,
    ThymosAgentOutput, ToolCallSummary,
};
pub use config::ThymosAgentConfig;
pub use memory_provider::ThymosMemoryProvider;
pub use tool_adapter::{thymos_tools_to_autoagents, ThymosToolAdapter};

// Actor types
pub use actor::{ThymosActorAgentBuilder, ThymosActorAgentExt, ThymosActorAgentHandle};

// Event types (Phase 4: Event Protocol)
pub use events::{event_channel, AgentEvent, EventEmitter, EventReceiver, EventSender};

// Executor types
pub use executor::{
    ThymosAgentExecutor, ThymosExecutor, ThymosExecutorConfig, ThymosExecutorError,
    ThymosExecutorOutput,
};

// Re-export AutoAgents executor types for convenience
pub use autoagents_core::agent::prebuilt::executor::{
    BasicAgent, BasicAgentOutput, BasicExecutorError, ReActAgent, ReActAgentOutput,
    ReActExecutorError,
};

// Re-export AutoAgents actor types for convenience
pub use autoagents_core::actor::Topic;
pub use autoagents_core::agent::task::Task;
pub use autoagents_core::runtime::{Runtime, SingleThreadedRuntime};

/// Type alias for using AutoAgents' ReAct executor with ThymosAgentCore.
///
/// This provides the battle-tested ReAct (Reason + Act) execution pattern
/// from AutoAgents while using Thymos's advanced memory, tools, and policy systems.
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::integration::{ThymosAgentCore, ThymosReActAgent};
///
/// let agent = ThymosAgentCore::builder()
///     .name("react_agent")
///     .description("An agent using ReAct pattern")
///     .memory(memory)
///     .tools(tools)
///     .build()?;
///
/// let react_agent = ThymosReActAgent::new(agent);
/// let result = react_agent.execute(&task, context).await?;
/// ```
pub type ThymosReActAgent = ReActAgent<ThymosAgentCore>;

/// Type alias for using AutoAgents' Basic executor with ThymosAgentCore.
///
/// The Basic executor performs a single LLM call without tool support,
/// useful for simple question-answering or text generation tasks.
pub type ThymosBasicAgent = BasicAgent<ThymosAgentCore>;


