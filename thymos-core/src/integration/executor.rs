//! Thymos Executor - Turn-based execution with Thymos patterns
//!
//! This module provides `ThymosExecutor<T>`, a generic wrapper that implements
//! `AgentExecutor` for any type implementing `AgentDeriveT + AgentHooks`.
//!
//! The executor integrates Thymos-specific patterns:
//! - Memory system integration for conversation history
//! - Policy enforcement via hooks
//! - Provenance tracking through tool results
//! - Turn-based execution with configurable max turns
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::integration::{ThymosExecutor, ThymosAgentCore, ThymosExecutorConfig};
//!
//! let agent = ThymosAgentCore::builder()
//!     .name("research_agent")
//!     .description("An agent that performs research")
//!     .memory(memory)
//!     .build()?;
//!
//! let executor = ThymosExecutor::new(agent)
//!     .with_config(ThymosExecutorConfig::default().with_max_turns(15));
//!
//! let result = executor.execute(&task, context).await?;
//! ```

use async_trait::async_trait;
use autoagents_core::agent::task::Task;
use autoagents_core::agent::{
    AgentDeriveT, AgentExecutor, AgentHooks, Context, ExecutorConfig, HookOutcome, TurnResult,
};
use autoagents_core::tool::{ToolCallResult, ToolT};
use autoagents_llm::chat::{ChatMessage, ChatRole, FunctionTool, MessageType, Tool as LLMTool};
use autoagents_llm::ToolCall;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Configuration for ThymosExecutor
#[derive(Debug, Clone)]
pub struct ThymosExecutorConfig {
    /// Maximum number of turns before stopping
    pub max_turns: usize,
    /// Whether to store conversation in memory
    pub store_conversation: bool,
    /// Whether to emit verbose logging
    pub verbose: bool,
}

impl Default for ThymosExecutorConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            store_conversation: true,
            verbose: false,
        }
    }
}

impl ThymosExecutorConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max turns
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Enable/disable conversation storage
    pub fn with_store_conversation(mut self, store: bool) -> Self {
        self.store_conversation = store;
        self
    }

    /// Enable/disable verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// Output of the Thymos executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThymosExecutorOutput {
    /// The final response text
    pub response: String,
    /// Tool calls made during execution
    pub tool_calls: Vec<ToolCallSummary>,
    /// Number of turns taken
    pub turns: usize,
    /// Whether execution completed successfully
    pub done: bool,
}

impl From<ThymosExecutorOutput> for Value {
    fn from(output: ThymosExecutorOutput) -> Self {
        serde_json::to_value(output).unwrap_or(Value::Null)
    }
}

impl From<ThymosExecutorOutput> for String {
    fn from(output: ThymosExecutorOutput) -> Self {
        output.response
    }
}

/// Summary of a tool call made during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    /// Name of the tool
    pub tool_name: String,
    /// Whether the call succeeded
    pub success: bool,
    /// Execution duration in milliseconds
    pub duration_ms: Option<u64>,
}

/// Error type for ThymosExecutor
#[derive(Error, Debug)]
pub enum ThymosExecutorError {
    #[error("LLM error: {0}")]
    LLMError(String),

    #[error("Maximum turns exceeded: {max_turns}")]
    MaxTurnsExceeded { max_turns: usize },

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("Hook aborted execution")]
    HookAborted,

    #[error("Other error: {0}")]
    Other(String),
}

/// Execution state tracked across turns
#[derive(Debug, Default)]
struct ExecutionState {
    turn_count: usize,
    tool_calls: Vec<ToolCallSummary>,
    messages: Vec<ChatMessage>,
}

/// Generic executor wrapper that provides Thymos-style execution patterns.
///
/// `ThymosExecutor<T>` wraps any type implementing `AgentDeriveT + AgentHooks`
/// and provides turn-based execution with:
/// - Configurable max turns
/// - Hook integration at turn and tool boundaries
/// - Memory of conversation history
/// - Tool execution with result tracking
///
/// # Type Parameters
///
/// - `T`: The inner agent type, must implement `AgentDeriveT + AgentHooks`
#[derive(Debug)]
pub struct ThymosExecutor<T: AgentDeriveT + AgentHooks> {
    inner: Arc<T>,
    config: ThymosExecutorConfig,
    state: RwLock<ExecutionState>,
}

impl<T: AgentDeriveT + AgentHooks> Clone for ThymosExecutor<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            config: self.config.clone(),
            state: RwLock::new(ExecutionState::default()),
        }
    }
}

impl<T: AgentDeriveT + AgentHooks> ThymosExecutor<T> {
    /// Create a new executor wrapping the given agent
    pub fn new(agent: T) -> Self {
        Self {
            inner: Arc::new(agent),
            config: ThymosExecutorConfig::default(),
            state: RwLock::new(ExecutionState::default()),
        }
    }

    /// Create a new executor from an Arc'd agent
    pub fn from_arc(agent: Arc<T>) -> Self {
        Self {
            inner: agent,
            config: ThymosExecutorConfig::default(),
            state: RwLock::new(ExecutionState::default()),
        }
    }

    /// Set the executor configuration
    pub fn with_config(mut self, config: ThymosExecutorConfig) -> Self {
        self.config = config;
        self
    }

    /// Get a reference to the inner agent
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get the executor configuration
    pub fn executor_config(&self) -> &ThymosExecutorConfig {
        &self.config
    }

    /// Process a single turn of execution
    async fn process_turn(
        &self,
        context: &Context,
        tools: &[Box<dyn ToolT>],
        llm_tools: &[LLMTool],
    ) -> Result<TurnResult<ThymosExecutorOutput>, ThymosExecutorError> {
        let state = self.state.read().await;
        let messages = state.messages.clone();
        drop(state);

        // Call LLM
        let tools_ref: Option<&[LLMTool]> = if llm_tools.is_empty() {
            None
        } else {
            Some(llm_tools)
        };

        let response = context
            .llm()
            .chat(&messages, tools_ref, None)
            .await
            .map_err(|e| ThymosExecutorError::LLMError(e.to_string()))?;

        // Check for tool calls
        if let Some(tool_calls) = response.tool_calls() {
            self.handle_tool_calls(context, tools, tool_calls, response.text())
                .await
        } else if let Some(text) = response.text() {
            // No tool calls, got final response
            self.handle_final_response(text).await
        } else {
            // Empty response, continue
            Ok(TurnResult::Continue(None))
        }
    }

    /// Handle tool calls from LLM response
    async fn handle_tool_calls(
        &self,
        context: &Context,
        tools: &[Box<dyn ToolT>],
        tool_calls: Vec<ToolCall>,
        response_text: Option<String>,
    ) -> Result<TurnResult<ThymosExecutorOutput>, ThymosExecutorError> {
        let mut tool_results = Vec::new();

        for tool_call in &tool_calls {
            // Check hook before tool execution
            let hook_outcome = self.inner.on_tool_call(tool_call, context).await;
            if hook_outcome == HookOutcome::Abort {
                if self.config.verbose {
                    tracing::warn!(
                        tool = %tool_call.function.name,
                        "Tool call aborted by hook"
                    );
                }
                continue;
            }

            self.inner.on_tool_start(tool_call, context).await;

            // Find and execute tool
            let start_time = std::time::Instant::now();
            let args: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(Value::Null);

            let result = if let Some(tool) = tools.iter().find(|t| t.name() == tool_call.function.name) {
                match tool.execute(args.clone()).await {
                    Ok(value) => ToolCallResult {
                        tool_name: tool_call.function.name.clone(),
                        success: true,
                        arguments: args,
                        result: value,
                    },
                    Err(e) => ToolCallResult {
                        tool_name: tool_call.function.name.clone(),
                        success: false,
                        arguments: args,
                        result: serde_json::json!({"error": e.to_string()}),
                    },
                }
            } else {
                ToolCallResult {
                    tool_name: tool_call.function.name.clone(),
                    success: false,
                    arguments: args,
                    result: serde_json::json!({"error": "Tool not found"}),
                }
            };

            let duration_ms = start_time.elapsed().as_millis() as u64;

            // Record tool call
            {
                let mut state = self.state.write().await;
                state.tool_calls.push(ToolCallSummary {
                    tool_name: result.tool_name.clone(),
                    success: result.success,
                    duration_ms: Some(duration_ms),
                });
            }

            // Call hooks
            if result.success {
                self.inner.on_tool_result(tool_call, &result, context).await;
            } else {
                self.inner.on_tool_error(tool_call, result.result.clone(), context).await;
            }

            tool_results.push(result);
        }

        // Add tool results to conversation
        {
            let mut state = self.state.write().await;
            for result in &tool_results {
                state.messages.push(ChatMessage {
                    role: ChatRole::Tool,
                    message_type: MessageType::Text,
                    content: serde_json::to_string(&result.result).unwrap_or_default(),
                });
            }
        }

        // Return partial result with tool calls
        let state = self.state.read().await;
        Ok(TurnResult::Continue(Some(ThymosExecutorOutput {
            response: response_text.unwrap_or_default(),
            tool_calls: state.tool_calls.clone(),
            turns: state.turn_count,
            done: false,
        })))
    }

    /// Handle final text response (no tool calls)
    async fn handle_final_response(
        &self,
        text: String,
    ) -> Result<TurnResult<ThymosExecutorOutput>, ThymosExecutorError> {
        let state = self.state.read().await;
        Ok(TurnResult::Complete(ThymosExecutorOutput {
            response: text,
            tool_calls: state.tool_calls.clone(),
            turns: state.turn_count,
            done: true,
        }))
    }

    /// Initialize execution state for a new task
    async fn initialize_execution(&self, task: &Task, context: &Context) {
        let agent_config = context.config();
        
        let system_message = ChatMessage {
            role: ChatRole::System,
            message_type: MessageType::Text,
            content: agent_config.description.clone(),
        };

        let user_message = if let Some((mime, image_data)) = &task.image {
            ChatMessage {
                role: ChatRole::User,
                message_type: MessageType::Image((*mime, image_data.clone())),
                content: task.prompt.clone(),
            }
        } else {
            ChatMessage {
                role: ChatRole::User,
                message_type: MessageType::Text,
                content: task.prompt.clone(),
            }
        };

        let mut state = self.state.write().await;
        state.turn_count = 0;
        state.tool_calls.clear();
        state.messages = vec![system_message, user_message];
    }

    /// Convert tools to LLM format
    fn tools_to_llm_format(tools: &[Box<dyn ToolT>]) -> Vec<LLMTool> {
        tools
            .iter()
            .map(|t| LLMTool {
                tool_type: "function".to_string(),
                function: FunctionTool {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    parameters: t.args_schema(),
                },
            })
            .collect()
    }
}

impl<T: AgentDeriveT + AgentHooks> Deref for ThymosExecutor<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Delegate AgentDeriveT to inner type
#[async_trait]
impl<T: AgentDeriveT + AgentHooks> AgentDeriveT for ThymosExecutor<T> {
    type Output = <T as AgentDeriveT>::Output;

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn description(&self) -> &'static str {
        self.inner.description()
    }

    fn tools(&self) -> Vec<Box<dyn ToolT>> {
        self.inner.tools()
    }

    fn output_schema(&self) -> Option<Value> {
        self.inner.output_schema()
    }
}

/// Delegate AgentHooks to inner type
#[async_trait]
impl<T: AgentDeriveT + AgentHooks + Send + Sync + 'static> AgentHooks for ThymosExecutor<T> {
    async fn on_agent_create(&self) {
        self.inner.on_agent_create().await
    }

    async fn on_run_start(&self, task: &Task, ctx: &Context) -> HookOutcome {
        self.inner.on_run_start(task, ctx).await
    }

    async fn on_run_complete(&self, task: &Task, result: &Self::Output, ctx: &Context) {
        self.inner.on_run_complete(task, result, ctx).await
    }

    async fn on_turn_start(&self, turn_index: usize, ctx: &Context) {
        self.inner.on_turn_start(turn_index, ctx).await
    }

    async fn on_turn_complete(&self, turn_index: usize, ctx: &Context) {
        self.inner.on_turn_complete(turn_index, ctx).await
    }

    async fn on_tool_call(&self, tool_call: &ToolCall, ctx: &Context) -> HookOutcome {
        self.inner.on_tool_call(tool_call, ctx).await
    }

    async fn on_tool_start(&self, tool_call: &ToolCall, ctx: &Context) {
        self.inner.on_tool_start(tool_call, ctx).await
    }

    async fn on_tool_result(&self, tool_call: &ToolCall, result: &ToolCallResult, ctx: &Context) {
        self.inner.on_tool_result(tool_call, result, ctx).await
    }

    async fn on_tool_error(&self, tool_call: &ToolCall, err: Value, ctx: &Context) {
        self.inner.on_tool_error(tool_call, err, ctx).await
    }

    async fn on_agent_shutdown(&self) {
        self.inner.on_agent_shutdown().await
    }
}

/// Implement AgentExecutor for ThymosExecutor
#[async_trait]
impl<T: AgentDeriveT + AgentHooks + Send + Sync + 'static> AgentExecutor for ThymosExecutor<T> {
    type Output = ThymosExecutorOutput;
    type Error = ThymosExecutorError;

    fn config(&self) -> ExecutorConfig {
        ExecutorConfig {
            max_turns: self.config.max_turns,
        }
    }

    async fn execute(
        &self,
        task: &Task,
        context: Arc<Context>,
    ) -> Result<Self::Output, Self::Error> {
        // Initialize execution
        self.initialize_execution(task, &context).await;

        // Run start hook
        let hook_outcome = self.inner.on_run_start(task, &context).await;
        if hook_outcome == HookOutcome::Abort {
            return Err(ThymosExecutorError::HookAborted);
        }

        // Get tools
        let tools = context.tools();
        let llm_tools = Self::tools_to_llm_format(tools);

        // Execute turns
        let mut accumulated_tool_calls = Vec::new();
        let mut final_response = String::new();

        for turn in 0..self.config.max_turns {
            // Update turn count
            {
                let mut state = self.state.write().await;
                state.turn_count = turn + 1;
            }

            // Turn start hook
            self.inner.on_turn_start(turn, &context).await;

            if self.config.verbose {
                tracing::debug!(turn = turn, "Starting turn");
            }

            // Process turn
            match self.process_turn(&context, tools, &llm_tools).await? {
                TurnResult::Complete(result) => {
                    self.inner.on_turn_complete(turn, &context).await;
                    
                    // Merge accumulated tool calls if any
                    let mut final_result = result;
                    if !accumulated_tool_calls.is_empty() {
                        final_result.tool_calls = accumulated_tool_calls;
                    }
                    
                    return Ok(final_result);
                }
                TurnResult::Continue(Some(partial)) => {
                    accumulated_tool_calls.extend(partial.tool_calls);
                    if !partial.response.is_empty() {
                        final_response = partial.response;
                    }
                }
                TurnResult::Continue(None) => {}
            }

            self.inner.on_turn_complete(turn, &context).await;
        }

        // Max turns reached
        if !final_response.is_empty() || !accumulated_tool_calls.is_empty() {
            Ok(ThymosExecutorOutput {
                response: final_response,
                tool_calls: accumulated_tool_calls,
                turns: self.config.max_turns,
                done: true,
            })
        } else {
            Err(ThymosExecutorError::MaxTurnsExceeded {
                max_turns: self.config.max_turns,
            })
        }
    }

    async fn execute_stream(
        &self,
        task: &Task,
        context: Arc<Context>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output, Self::Error>> + Send>>, Self::Error>
    {
        // Initialize execution
        self.initialize_execution(task, &context).await;

        // Run start hook
        let hook_outcome = self.inner.on_run_start(task, &context).await;
        if hook_outcome == HookOutcome::Abort {
            return Err(ThymosExecutorError::HookAborted);
        }

        // Get tools
        let tools = context.tools();
        let llm_tools = Self::tools_to_llm_format(tools);

        // Create channel for streaming results
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<ThymosExecutorOutput, ThymosExecutorError>>(32);

        // Clone what we need for the spawned task
        let executor = self.clone();
        let context_clone = context.clone();
        let max_turns = self.config.max_turns;
        let verbose = self.config.verbose;

        // Spawn execution task
        tokio::spawn(async move {
            let tools = context_clone.tools();
            let mut accumulated_tool_calls = Vec::new();
            let mut final_response = String::new();

            for turn in 0..max_turns {
                // Update turn count
                {
                    let mut state = executor.state.write().await;
                    state.turn_count = turn + 1;
                }

                // Turn start hook
                executor.inner.on_turn_start(turn, &context_clone).await;

                if verbose {
                    tracing::debug!(turn = turn, "Starting turn (streaming)");
                }

                // Process turn
                match executor.process_turn(&context_clone, tools, &llm_tools).await {
                    Ok(TurnResult::Complete(result)) => {
                        executor.inner.on_turn_complete(turn, &context_clone).await;
                        
                        let mut final_result = result;
                        if !accumulated_tool_calls.is_empty() {
                            final_result.tool_calls = accumulated_tool_calls;
                        }
                        
                        let _ = tx.send(Ok(final_result)).await;
                        return;
                    }
                    Ok(TurnResult::Continue(Some(partial))) => {
                        // Stream partial result
                        let _ = tx.send(Ok(partial.clone())).await;
                        
                        accumulated_tool_calls.extend(partial.tool_calls);
                        if !partial.response.is_empty() {
                            final_response = partial.response;
                        }
                    }
                    Ok(TurnResult::Continue(None)) => {}
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                }

                executor.inner.on_turn_complete(turn, &context_clone).await;
            }

            // Max turns - send final result or error
            if !final_response.is_empty() || !accumulated_tool_calls.is_empty() {
                let _ = tx.send(Ok(ThymosExecutorOutput {
                    response: final_response,
                    tool_calls: accumulated_tool_calls,
                    turns: max_turns,
                    done: true,
                })).await;
            } else {
                let _ = tx.send(Err(ThymosExecutorError::MaxTurnsExceeded { max_turns })).await;
            }
        });

        // Convert receiver to stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}

/// Type alias for ThymosExecutor wrapping ThymosAgentCore
pub type ThymosAgentExecutor = ThymosExecutor<super::ThymosAgentCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, MemoryMode};
    use crate::memory::MemorySystem;
    use crate::tools::{
        CapabilityPolicy, Tool, ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema,
    };
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
            _ctx: &crate::tools::ToolExecutionContext,
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

    #[test]
    fn test_executor_config_default() {
        let config = ThymosExecutorConfig::default();
        assert_eq!(config.max_turns, 10);
        assert!(config.store_conversation);
        assert!(!config.verbose);
    }

    #[test]
    fn test_executor_config_builder() {
        let config = ThymosExecutorConfig::new()
            .with_max_turns(20)
            .with_store_conversation(false)
            .with_verbose(true);

        assert_eq!(config.max_turns, 20);
        assert!(!config.store_conversation);
        assert!(config.verbose);
    }

    #[test]
    fn test_executor_output_serialization() {
        let output = ThymosExecutorOutput {
            response: "Test response".to_string(),
            tool_calls: vec![ToolCallSummary {
                tool_name: "test_tool".to_string(),
                success: true,
                duration_ms: Some(100),
            }],
            turns: 2,
            done: true,
        };

        let value: Value = output.clone().into();
        assert_eq!(value["response"], "Test response");
        assert_eq!(value["turns"], 2);
        assert!(value["done"].as_bool().unwrap());

        let string: String = output.into();
        assert_eq!(string, "Test response");
    }

    #[test]
    fn test_tool_call_summary() {
        let summary = ToolCallSummary {
            tool_name: "test_tool".to_string(),
            success: true,
            duration_ms: Some(150),
        };

        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["tool_name"], "test_tool");
        assert_eq!(json["success"], true);
        assert_eq!(json["duration_ms"], 150);
    }

    #[test]
    fn test_executor_error_display() {
        let llm_err = ThymosExecutorError::LLMError("Connection failed".to_string());
        assert!(llm_err.to_string().contains("Connection failed"));

        let max_turns_err = ThymosExecutorError::MaxTurnsExceeded { max_turns: 10 };
        assert!(max_turns_err.to_string().contains("10"));

        let hook_err = ThymosExecutorError::HookAborted;
        assert!(hook_err.to_string().contains("Hook"));
    }

    #[tokio::test]
    async fn test_executor_creation() {
        use super::super::ThymosAgentCore;

        let (memory, _temp_dir) = create_test_memory().await;
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let agent = ThymosAgentCore::builder()
            .name("test_agent")
            .description("A test agent")
            .memory(memory)
            .tool(tool)
            .policy(CapabilityPolicy::allow_all())
            .build()
            .expect("Failed to build agent");

        let executor = ThymosExecutor::new(agent)
            .with_config(ThymosExecutorConfig::default().with_max_turns(5));

        assert_eq!(executor.name(), "test_agent");
        assert_eq!(executor.executor_config().max_turns, 5);
        assert_eq!(executor.config().max_turns, 5);
    }

    #[tokio::test]
    async fn test_executor_deref() {
        use super::super::ThymosAgentCore;

        let (memory, _temp_dir) = create_test_memory().await;

        let agent = ThymosAgentCore::builder()
            .name("deref_agent")
            .description("Testing deref")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        let executor = ThymosExecutor::new(agent);

        // Access inner agent via deref
        assert_eq!(executor.agent_name(), "deref_agent");
    }

    #[tokio::test]
    async fn test_executor_clone() {
        use super::super::ThymosAgentCore;

        let (memory, _temp_dir) = create_test_memory().await;

        let agent = ThymosAgentCore::builder()
            .name("clone_agent")
            .description("Testing clone")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        let executor = ThymosExecutor::new(agent)
            .with_config(ThymosExecutorConfig::default().with_max_turns(15));

        let cloned = executor.clone();

        assert_eq!(cloned.name(), "clone_agent");
        assert_eq!(cloned.executor_config().max_turns, 15);
    }

    #[tokio::test]
    async fn test_executor_from_arc() {
        use super::super::ThymosAgentCore;

        let (memory, _temp_dir) = create_test_memory().await;

        let agent = Arc::new(
            ThymosAgentCore::builder()
                .name("arc_agent")
                .description("Testing from_arc")
                .memory(memory)
                .build()
                .expect("Failed to build agent"),
        );

        let executor = ThymosExecutor::from_arc(agent);

        assert_eq!(executor.name(), "arc_agent");
    }

    #[test]
    fn test_tools_to_llm_format() {
        use super::super::ThymosToolAdapter;

        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let adapter = Box::new(ThymosToolAdapter::new(tool)) as Box<dyn ToolT>;
        let tools = vec![adapter];

        let llm_tools = ThymosExecutor::<super::super::ThymosAgentCore>::tools_to_llm_format(&tools);

        assert_eq!(llm_tools.len(), 1);
        assert_eq!(llm_tools[0].function.name, "echo");
        assert_eq!(llm_tools[0].tool_type, "function");
    }
}


