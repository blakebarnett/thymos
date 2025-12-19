//! Actor-based agent integration with AutoAgents
//!
//! This module provides `ThymosActorAgentBuilder` which wraps `ThymosAgentCore` as an
//! AutoAgents `ActorAgent`, enabling message-driven execution with topic subscriptions.
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::integration::{ThymosAgentCore, ThymosActorAgentBuilder, Topic, Task};
//! use autoagents_core::runtime::SingleThreadedRuntime;
//!
//! // Create the core agent
//! let agent = ThymosAgentCore::builder()
//!     .name("task_processor")
//!     .description("Processes tasks from topics")
//!     .memory(memory)
//!     .tools(tools)
//!     .build()?;
//!
//! // Create runtime
//! let runtime = SingleThreadedRuntime::new(None);
//!
//! // Build as actor agent with topic subscriptions
//! let handle = ThymosActorAgentBuilder::new(agent)
//!     .llm(llm_provider)
//!     .runtime(runtime.clone())
//!     .subscribe(Topic::new("research_tasks"))
//!     .subscribe(Topic::new("analysis_tasks"))
//!     .build()
//!     .await?;
//!
//! // Agent now receives Task messages on subscribed topics
//! // and executes using ThymosAgentCore's executor
//! ```

use super::ThymosAgentCore;
use autoagents_core::actor::Topic;
use autoagents_core::agent::task::Task;
use autoagents_core::agent::{ActorAgent, ActorAgentHandle, AgentBuilder, BaseAgent};
use autoagents_core::agent::memory::MemoryProvider;
use autoagents_core::error::Error;
use autoagents_core::runtime::Runtime;
use autoagents_llm::LLMProvider;
use std::sync::Arc;

/// Handle for a Thymos actor agent, providing access to both the agent and its actor reference.
///
/// This type alias provides convenient access to the underlying `ActorAgentHandle`
/// specialized for `ThymosAgentCore`.
pub type ThymosActorAgentHandle = ActorAgentHandle<ThymosAgentCore>;

/// Builder for creating actor-based Thymos agents with topic subscriptions.
///
/// This builder wraps `ThymosAgentCore` and builds it as an AutoAgents `ActorAgent`,
/// enabling message-driven task execution through topic subscriptions.
///
/// # Example
///
/// ```rust,ignore
/// let handle = ThymosActorAgentBuilder::new(agent)
///     .llm(llm_provider)
///     .runtime(runtime)
///     .subscribe(Topic::new("tasks"))
///     .build()
///     .await?;
/// ```
pub struct ThymosActorAgentBuilder {
    inner: AgentBuilder<ThymosAgentCore, ActorAgent>,
}

impl ThymosActorAgentBuilder {
    /// Create a new builder wrapping a ThymosAgentCore
    pub fn new(agent: ThymosAgentCore) -> Self {
        Self {
            inner: AgentBuilder::new(agent),
        }
    }

    /// Set the LLM provider (required)
    pub fn llm(mut self, llm: Arc<dyn LLMProvider>) -> Self {
        self.inner = self.inner.llm(llm);
        self
    }

    /// Set a custom memory provider
    ///
    /// If not set, a `ThymosMemoryProvider` will be created from the agent's memory system.
    pub fn memory_provider(mut self, provider: Box<dyn MemoryProvider>) -> Self {
        self.inner = self.inner.memory(provider);
        self
    }

    /// Set the runtime (required)
    pub fn runtime(mut self, runtime: Arc<dyn Runtime>) -> Self {
        self.inner = self.inner.runtime(runtime);
        self
    }

    /// Subscribe to a topic for receiving Task messages
    ///
    /// The agent will receive and process any `Task` messages published to this topic.
    pub fn subscribe(mut self, topic: Topic<Task>) -> Self {
        self.inner = self.inner.subscribe(topic);
        self
    }

    /// Enable streaming execution mode
    pub fn stream(mut self, stream: bool) -> Self {
        self.inner = self.inner.stream(stream);
        self
    }

    /// Build the actor agent and return a handle
    ///
    /// This spawns the actor and subscribes it to all configured topics.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - LLM provider is not set
    /// - Runtime is not set
    /// - Actor spawning fails
    /// - Topic subscription fails
    pub async fn build(self) -> Result<ThymosActorAgentHandle, Error> {
        self.inner.build().await
    }
}

/// Extension trait for ThymosAgentCore to create actor agents
pub trait ThymosActorAgentExt {
    /// Create a builder for an actor-based agent
    fn into_actor_builder(self) -> ThymosActorAgentBuilder;
}

impl ThymosActorAgentExt for ThymosAgentCore {
    fn into_actor_builder(self) -> ThymosActorAgentBuilder {
        ThymosActorAgentBuilder::new(self)
    }
}

/// Helper type alias for the underlying BaseAgent with ActorAgent type
#[allow(dead_code)]
pub type ThymosBaseActorAgent = BaseAgent<ThymosAgentCore, ActorAgent>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, MemoryMode};
    use crate::memory::MemorySystem;
    use crate::tools::{
        CapabilityPolicy, Tool, ToolExecutionContext, ToolMetadata, ToolProvenance,
        ToolResultEnvelope, ToolSchema,
    };
    use async_trait::async_trait;
    use serde_json::Value;
    use tempfile::TempDir;

    struct EchoTool {
        metadata: ToolMetadata,
    }

    impl EchoTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("echo", "Echoes input back"),
            }
        }
    }

    #[async_trait]
    impl Tool for EchoTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::new(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }))
        }

        async fn execute(
            &self,
            args: Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, crate::tools::ToolError> {
            let message = args.get("message").cloned().unwrap_or(Value::Null);
            let provenance = ToolProvenance::new("echo", "test_hash");
            Ok(ToolResultEnvelope::success(message, provenance))
        }
    }

    async fn create_test_memory() -> (Arc<MemorySystem>, TempDir) {
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
        (Arc::new(memory), temp_dir)
    }

    #[tokio::test]
    async fn test_builder_creation() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let agent = ThymosAgentCore::builder()
            .name("test_actor_agent")
            .description("A test actor agent")
            .memory(memory)
            .tool(tool)
            .policy(CapabilityPolicy::allow_all())
            .build()
            .expect("Failed to build agent");

        // Just verify the builder can be created
        let _builder = ThymosActorAgentBuilder::new(agent);
    }

    #[tokio::test]
    async fn test_extension_trait() {
        let (memory, _temp_dir) = create_test_memory().await;

        let agent = ThymosAgentCore::builder()
            .name("ext_agent")
            .description("Testing extension trait")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        // Verify extension trait works
        let _builder = agent.into_actor_builder();
    }

    #[tokio::test]
    async fn test_builder_chaining() {
        let (memory, _temp_dir) = create_test_memory().await;

        let agent = ThymosAgentCore::builder()
            .name("chain_agent")
            .description("Testing builder chaining")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        let topic1 = Topic::<Task>::new("tasks");
        let topic2 = Topic::<Task>::new("analysis");

        // Verify chaining works (we can't fully build without LLM/runtime)
        let _builder = ThymosActorAgentBuilder::new(agent)
            .subscribe(topic1)
            .subscribe(topic2)
            .stream(true);
    }
}

