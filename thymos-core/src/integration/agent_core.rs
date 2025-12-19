//! Core Thymos agent implementing AutoAgents traits
//!
//! ThymosAgentCore is the central integration point that implements AutoAgents'
//! AgentDeriveT, AgentHooks, and AgentExecutor traits while preserving Thymos's
//! advanced features like memory, provenance tracking, and capability-based security.

use super::config::ThymosAgentConfig;
use super::events::EventEmitter;
use super::memory_provider::ThymosMemoryProvider;
use super::tool_adapter::thymos_tools_to_autoagents;
use crate::memory::MemorySystem;
use crate::replay::{ReplayCapture, ToolCallEvent, ToolCallStatus};
use crate::tools::{CapabilityPolicy, Tool, ToolExecutionContext};
use async_trait::async_trait;
use autoagents_core::agent::task::Task;
use autoagents_core::agent::{
    AgentDeriveT, AgentExecutor, AgentHooks, AgentOutputT, Context, ExecutorConfig, HookOutcome,
};
use autoagents_core::tool::{ToolCallResult, ToolT};
use autoagents_llm::ToolCall;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use std::error::Error as StdError;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Output type for Thymos agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThymosAgentOutput {
    /// The generated response text
    pub response: String,
    /// Tool calls made during execution
    pub tool_calls: Vec<ToolCallSummary>,
    /// Metadata about the execution
    pub metadata: Value,
}

impl From<ThymosAgentOutput> for Value {
    fn from(output: ThymosAgentOutput) -> Self {
        serde_json::to_value(output).unwrap_or(Value::Null)
    }
}

impl AgentOutputT for ThymosAgentOutput {
    fn output_schema() -> &'static str {
        r#"{"type":"object","properties":{"response":{"type":"string"},"tool_calls":{"type":"array"},"metadata":{"type":"object"}},"required":["response"]}"#
    }

    fn structured_output_format() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "response": {"type": "string", "description": "The agent's response"},
                "tool_calls": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool_name": {"type": "string"},
                            "success": {"type": "boolean"},
                            "duration_ms": {"type": "integer", "nullable": true}
                        }
                    }
                },
                "metadata": {"type": "object"}
            },
            "required": ["response"]
        })
    }
}

/// Summary of a tool call made during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub success: bool,
    pub duration_ms: Option<u64>,
}

/// Error type for Thymos agent execution
#[derive(Debug)]
pub struct ThymosAgentError {
    pub message: String,
    pub kind: ThymosAgentErrorKind,
}

#[derive(Debug, Clone)]
pub enum ThymosAgentErrorKind {
    ExecutionError,
    ToolError,
    MemoryError,
    PolicyViolation,
    ConfigurationError,
}

impl Display for ThymosAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl StdError for ThymosAgentError {}

impl ThymosAgentError {
    pub fn execution(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ThymosAgentErrorKind::ExecutionError,
        }
    }

    pub fn tool(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ThymosAgentErrorKind::ToolError,
        }
    }

    pub fn memory(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ThymosAgentErrorKind::MemoryError,
        }
    }

    pub fn policy(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ThymosAgentErrorKind::PolicyViolation,
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ThymosAgentErrorKind::ConfigurationError,
        }
    }
}

/// Core Thymos agent that implements AutoAgents traits.
///
/// This struct provides the bridge between Thymos's rich feature set and
/// AutoAgents' execution patterns. It implements:
///
/// - `AgentDeriveT`: Agent metadata and tool discovery
/// - `AgentHooks`: Lifecycle hooks with policy enforcement and provenance
/// - `AgentExecutor`: Task execution strategy
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::integration::{ThymosAgentCore, ThymosAgentConfig};
///
/// let agent = ThymosAgentCore::builder()
///     .name("research_agent")
///     .description("An agent that performs research tasks")
///     .memory(memory_system)
///     .tools(vec![search_tool, browse_tool])
///     .policy(CapabilityPolicy::allow_all())
///     .build()?;
/// ```
pub struct ThymosAgentCore {
    /// Agent name
    name: &'static str,
    /// Agent description
    description: &'static str,
    /// Thymos memory system
    memory: Arc<MemorySystem>,
    /// Thymos tools
    tools: Vec<Arc<dyn Tool>>,
    /// Cached AutoAgents tool adapters
    tool_adapters: Vec<Box<dyn ToolT>>,
    /// Capability policy for tool execution
    policy: CapabilityPolicy,
    /// Agent configuration
    config: ThymosAgentConfig,
    /// Replay capture (optional)
    replay_capture: Option<Arc<ReplayCapture>>,
    /// Event emitter for real-time observability (optional)
    event_emitter: Option<EventEmitter>,
    /// Execution state
    state: RwLock<AgentExecutionState>,
}

#[derive(Debug, Default)]
struct AgentExecutionState {
    turn_count: usize,
    tool_calls: Vec<ToolCallSummary>,
    current_task_id: Option<String>,
    task_start_time: Option<std::time::Instant>,
}

impl Debug for ThymosAgentCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThymosAgentCore")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("tool_count", &self.tools.len())
            .finish()
    }
}

impl ThymosAgentCore {
    /// Create a new builder for ThymosAgentCore
    pub fn builder() -> ThymosAgentCoreBuilder {
        ThymosAgentCoreBuilder::new()
    }

    /// Get the agent's name
    pub fn agent_name(&self) -> &'static str {
        self.name
    }

    /// Get the agent's description
    pub fn agent_description(&self) -> &'static str {
        self.description
    }

    /// Get a reference to the memory system
    pub fn memory(&self) -> &MemorySystem {
        &self.memory
    }

    /// Get the capability policy
    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    /// Get the configuration
    pub fn config(&self) -> &ThymosAgentConfig {
        &self.config
    }

    /// Get replay capture if enabled
    pub fn replay_capture(&self) -> Option<&Arc<ReplayCapture>> {
        self.replay_capture.as_ref()
    }

    /// Get event emitter if configured
    pub fn event_emitter(&self) -> Option<&EventEmitter> {
        self.event_emitter.as_ref()
    }

    /// Store a memory
    pub async fn remember(&self, content: impl Into<String>) -> crate::error::Result<String> {
        self.memory.remember(content.into()).await
    }

    /// Search memories
    pub async fn search_memories(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> crate::error::Result<Vec<locai::models::Memory>> {
        self.memory.search(query, limit).await
    }

    /// Find a tool by name
    pub fn find_tool(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name)
    }

    /// Execute a tool by name with policy checking
    pub async fn execute_tool(
        &self,
        name: &str,
        args: Value,
    ) -> Result<crate::tools::ToolResultEnvelope, ThymosAgentError> {
        let tool = self
            .find_tool(name)
            .ok_or_else(|| ThymosAgentError::tool(format!("Tool not found: {}", name)))?;

        // Check capability policy
        let required = tool.required_capabilities();
        if let Err(denied) = self.policy.check_all(&required) {
            return Err(ThymosAgentError::policy(format!(
                "Policy denies tool '{}' - denied capabilities: {:?}",
                name, denied
            )));
        }

        let ctx = ToolExecutionContext::default();
        tool.execute(args, &ctx)
            .await
            .map_err(|e| ThymosAgentError::tool(e.to_string()))
    }

    /// Create a ThymosMemoryProvider for use with AutoAgents contexts
    pub fn create_memory_provider(&self) -> ThymosMemoryProvider {
        ThymosMemoryProvider::new(Arc::clone(&self.memory))
    }
}

// ============================================================================
// AgentDeriveT Implementation
// ============================================================================

#[async_trait]
impl AgentDeriveT for ThymosAgentCore {
    type Output = ThymosAgentOutput;

    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn tools(&self) -> Vec<Box<dyn ToolT>> {
        thymos_tools_to_autoagents(&self.tools)
    }

    fn output_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "response": { "type": "string", "description": "The agent's response" },
                "tool_calls": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool_name": { "type": "string" },
                            "success": { "type": "boolean" },
                            "duration_ms": { "type": "integer", "nullable": true }
                        }
                    }
                },
                "metadata": { "type": "object" }
            },
            "required": ["response"]
        }))
    }
}

// ============================================================================
// AgentHooks Implementation
// ============================================================================

#[async_trait]
impl AgentHooks for ThymosAgentCore {
    async fn on_agent_create(&self) {
        tracing::info!(
            agent = %self.name,
            "ThymosAgentCore created"
        );
    }

    async fn on_run_start(&self, task: &Task, _ctx: &Context) -> HookOutcome {
        // Use task's submission_id as task ID
        let task_id = task.submission_id.to_string();

        if self.config.verbose {
            tracing::info!(
                agent = %self.name,
                task = %task.prompt,
                task_id = %task_id,
                "Starting task execution"
            );
        }

        // Reset execution state
        {
            let mut state = self.state.write().await;
            state.turn_count = 0;
            state.tool_calls.clear();
            state.current_task_id = Some(task_id.clone());
            state.task_start_time = Some(std::time::Instant::now());
        }

        // Start replay capture if enabled
        if let Some(capture) = &self.replay_capture {
            capture.start_session().await;
        }

        // Emit task started event
        if let Some(emitter) = &self.event_emitter {
            emitter.task_started(&task_id, &task.prompt).await;
        }

        HookOutcome::Continue
    }

    async fn on_run_complete(&self, task: &Task, result: &Self::Output, _ctx: &Context) {
        let state = self.state.read().await;
        let task_id = state.current_task_id.clone();
        let duration_ms = state
            .task_start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let turns = state.turn_count;
        let tool_call_count = result.tool_calls.len();
        drop(state);

        if self.config.verbose {
            tracing::info!(
                agent = %self.name,
                task = %task.prompt,
                tool_calls = %tool_call_count,
                duration_ms = %duration_ms,
                "Task execution completed"
            );
        }

        // End replay capture if enabled
        if let Some(capture) = &self.replay_capture {
            let _record = capture.end_session().await;
        }

        // Emit task completed event
        if let (Some(emitter), Some(task_id)) = (&self.event_emitter, task_id) {
            emitter
                .task_completed(
                    &task_id,
                    true,
                    Some(result.response.clone()),
                    None,
                    turns,
                    tool_call_count,
                    duration_ms,
                )
                .await;
        }
    }

    async fn on_turn_start(&self, turn_index: usize, _ctx: &Context) {
        {
            let mut state = self.state.write().await;
            state.turn_count = turn_index;
        }

        if self.config.verbose {
            tracing::debug!(
                agent = %self.name,
                turn = turn_index,
                "Turn started"
            );
        }

        // Emit turn started event
        if let Some(emitter) = &self.event_emitter {
            emitter.turn_started(turn_index).await;
        }
    }

    async fn on_turn_complete(&self, turn_index: usize, _ctx: &Context) {
        if self.config.verbose {
            tracing::debug!(
                agent = %self.name,
                turn = turn_index,
                "Turn completed"
            );
        }

        // Emit turn completed event (is_final and had_tool_calls determined by caller)
        if let Some(emitter) = &self.event_emitter {
            let state = self.state.read().await;
            let had_tool_calls = !state.tool_calls.is_empty();
            emitter.turn_completed(turn_index, had_tool_calls, false).await;
        }
    }

    async fn on_tool_call(&self, tool_call: &ToolCall, _ctx: &Context) -> HookOutcome {
        // Check capability policy before allowing tool execution
        if let Some(tool) = self.find_tool(&tool_call.function.name) {
            let required = tool.required_capabilities();
            if let Err(denied) = self.policy.check_all(&required) {
                tracing::warn!(
                    tool = %tool_call.function.name,
                    denied = ?denied,
                    "Tool blocked by capability policy"
                );

                // Emit policy blocked event
                if let Some(emitter) = &self.event_emitter {
                    let denied_caps: Vec<String> =
                        denied.iter().map(|c| c.to_string()).collect();
                    emitter
                        .policy_blocked(&tool_call.function.name, denied_caps)
                        .await;
                }

                return HookOutcome::Abort;
            }
        } else {
            tracing::warn!(
                tool = %tool_call.function.name,
                "Tool call for unknown tool"
            );
        }

        HookOutcome::Continue
    }

    async fn on_tool_start(&self, tool_call: &ToolCall, _ctx: &Context) {
        if self.config.verbose {
            tracing::debug!(
                tool = %tool_call.function.name,
                "Tool execution starting"
            );
        }

        // Emit tool call started event
        if let Some(emitter) = &self.event_emitter {
            let args: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(Value::Null);
            emitter
                .tool_call_started(&tool_call.function.name, args)
                .await;
        }
    }

    async fn on_tool_result(&self, tool_call: &ToolCall, result: &ToolCallResult, _ctx: &Context) {
        // Record tool call in state
        {
            let mut state = self.state.write().await;
            state.tool_calls.push(ToolCallSummary {
                tool_name: tool_call.function.name.clone(),
                success: result.success,
                duration_ms: None,
            });
        }

        // Capture provenance for replay
        if let Some(capture) = &self.replay_capture {
            let args: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(Value::Null);

            let event = ToolCallEvent {
                tool_name: tool_call.function.name.clone(),
                tool_version: None,
                args: args.clone(),
                args_hash: format!(
                    "{:x}",
                    sha2::Sha256::digest(tool_call.function.arguments.as_bytes())
                )[..16]
                    .to_string(),
                result: Some(result.result.clone()),
                status: if result.success {
                    ToolCallStatus::Success
                } else {
                    ToolCallStatus::Error
                },
                duration_ms: 0,
                error: None,
                trace_id: None,
            };

            capture
                .emit(crate::replay::ReplayEvent::ToolCall(event))
                .await;
        }

        // Emit tool call completed event
        if let Some(emitter) = &self.event_emitter {
            emitter
                .tool_call_completed(&tool_call.function.name, result.success, 0, None)
                .await;
        }

        // Store tool result in memory if configured
        if self.config.store_tool_results {
            let content = format!(
                "[Tool: {}] Result: {}",
                tool_call.function.name,
                serde_json::to_string_pretty(&result.result).unwrap_or_default()
            );
            if let Err(e) = self.memory.remember(content).await {
                tracing::error!(
                    tool = %tool_call.function.name,
                    error = %e,
                    "Failed to store tool result in memory"
                );
            }
        }

        if self.config.verbose {
            tracing::debug!(
                tool = %tool_call.function.name,
                success = result.success,
                "Tool execution completed"
            );
        }
    }

    async fn on_tool_error(&self, tool_call: &ToolCall, err: Value, _ctx: &Context) {
        tracing::error!(
            tool = %tool_call.function.name,
            error = %err,
            "Tool execution failed"
        );

        // Record failed tool call
        {
            let mut state = self.state.write().await;
            state.tool_calls.push(ToolCallSummary {
                tool_name: tool_call.function.name.clone(),
                success: false,
                duration_ms: None,
            });
        }

        // Emit tool call completed event with error
        if let Some(emitter) = &self.event_emitter {
            let error_msg = err
                .get("error")
                .and_then(|e| e.as_str())
                .map(String::from)
                .or_else(|| Some(err.to_string()));
            emitter
                .tool_call_completed(&tool_call.function.name, false, 0, error_msg)
                .await;
        }
    }

    async fn on_agent_shutdown(&self) {
        tracing::info!(
            agent = %self.name,
            "ThymosAgentCore shutting down"
        );
    }
}

// ============================================================================
// AgentExecutor Implementation
// ============================================================================

#[async_trait]
impl AgentExecutor for ThymosAgentCore {
    type Output = ThymosAgentOutput;
    type Error = ThymosAgentError;

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
        use autoagents_llm::chat::{ChatMessage, ChatRole, Tool as LLMTool, FunctionTool};

        // Build system message
        let system_message = ChatMessage {
            role: ChatRole::System,
            message_type: autoagents_llm::chat::MessageType::Text,
            content: format!(
                "You are {}. {}\n\nYou have access to the following tools.",
                self.name, self.description
            ),
        };

        // Build user message from task
        let user_message = ChatMessage {
            role: ChatRole::User,
            message_type: autoagents_llm::chat::MessageType::Text,
            content: task.prompt.clone(),
        };

        let mut messages = vec![system_message, user_message];

        // Convert tools for LLM
        let llm_tools: Vec<LLMTool> = self.tools.iter().map(|t| {
            LLMTool {
                tool_type: "function".to_string(),
                function: FunctionTool {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    parameters: t.schema().parameters,
                },
            }
        }).collect();

        let llm = context.llm();
        
        // Execute turns
        for turn in 0..self.config.max_turns {
            self.on_turn_start(turn, &context).await;

            // Call LLM
            let tools_ref: Option<&[LLMTool]> = if llm_tools.is_empty() {
                None
            } else {
                Some(&llm_tools)
            };

            let response = llm
                .chat(&messages, tools_ref, None)
                .await
                .map_err(|e| ThymosAgentError::execution(format!("LLM error: {}", e)))?;

            // Check for tool calls
            if let Some(tool_calls) = response.tool_calls() {
                for tool_call in tool_calls {
                    // Check policy via hook
                    if self.on_tool_call(&tool_call, &context).await == HookOutcome::Abort {
                        continue;
                    }

                    self.on_tool_start(&tool_call, &context).await;

                    // Execute tool
                    let args: Value = serde_json::from_str(&tool_call.function.arguments)
                        .unwrap_or(Value::Null);
                    
                    let tool_result = if let Some(adapter) = self.tool_adapters.iter()
                        .find(|t| t.name() == tool_call.function.name)
                    {
                        match adapter.execute(args.clone()).await {
                            Ok(result) => ToolCallResult {
                                tool_name: tool_call.function.name.clone(),
                                success: true,
                                arguments: args,
                                result,
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

                    if tool_result.success {
                        self.on_tool_result(&tool_call, &tool_result, &context).await;
                    } else {
                        self.on_tool_error(&tool_call, tool_result.result.clone(), &context).await;
                    }

                    // Add tool result to messages
                    messages.push(ChatMessage {
                        role: ChatRole::Tool,
                        message_type: autoagents_llm::chat::MessageType::Text,
                        content: serde_json::to_string(&tool_result.result).unwrap_or_default(),
                    });
                }
            } else if let Some(text) = response.text() {
                // No tool calls, got final response
                self.on_turn_complete(turn, &context).await;

                let state = self.state.read().await;
                return Ok(ThymosAgentOutput {
                    response: text,
                    tool_calls: state.tool_calls.clone(),
                    metadata: serde_json::json!({
                        "turns": turn + 1,
                        "agent": self.name,
                    }),
                });
            }

            self.on_turn_complete(turn, &context).await;
        }

        // Max turns reached
        let state = self.state.read().await;
        Err(ThymosAgentError::execution(format!(
            "Max turns ({}) reached without completion. Tool calls: {}",
            self.config.max_turns,
            state.tool_calls.len()
        )))
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for ThymosAgentCore
pub struct ThymosAgentCoreBuilder {
    name: Option<&'static str>,
    description: Option<&'static str>,
    memory: Option<Arc<MemorySystem>>,
    tools: Vec<Arc<dyn Tool>>,
    policy: CapabilityPolicy,
    config: ThymosAgentConfig,
    enable_replay: bool,
    event_sender: Option<super::events::EventSender>,
}

impl ThymosAgentCoreBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            memory: None,
            tools: Vec::new(),
            policy: CapabilityPolicy::deny_all(),
            config: ThymosAgentConfig::default(),
            enable_replay: false,
            event_sender: None,
        }
    }

    /// Set the agent name
    pub fn name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the agent description
    pub fn description(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the memory system
    pub fn memory(mut self, memory: Arc<MemorySystem>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Add a tool
    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tools
    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Set the capability policy
    pub fn policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set the configuration
    pub fn config(mut self, config: ThymosAgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable replay capture
    pub fn with_replay(mut self, enable: bool) -> Self {
        self.enable_replay = enable;
        self
    }

    /// Set an event sender for real-time observability.
    ///
    /// When configured, the agent will emit events during execution that can
    /// be used for monitoring, debugging, and integration with external systems.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use thymos_core::integration::{event_channel, ThymosAgentCore};
    ///
    /// let (tx, rx) = event_channel(100);
    ///
    /// let agent = ThymosAgentCore::builder()
    ///     .name("my_agent")
    ///     .event_sender(tx)
    ///     .build()?;
    /// ```
    pub fn event_sender(mut self, sender: super::events::EventSender) -> Self {
        self.event_sender = Some(sender);
        self
    }

    /// Build the agent
    pub fn build(self) -> Result<ThymosAgentCore, ThymosAgentError> {
        let name = self
            .name
            .ok_or_else(|| ThymosAgentError::config("Agent name is required"))?;
        let description = self
            .description
            .ok_or_else(|| ThymosAgentError::config("Agent description is required"))?;
        let memory = self
            .memory
            .ok_or_else(|| ThymosAgentError::config("Memory system is required"))?;

        let tool_adapters = thymos_tools_to_autoagents(&self.tools);

        let replay_capture = if self.enable_replay || self.config.enable_replay_capture {
            Some(Arc::new(ReplayCapture::new(format!("{}_session", name))))
        } else {
            None
        };

        let event_emitter = self
            .event_sender
            .map(|sender| EventEmitter::new(sender, name));

        Ok(ThymosAgentCore {
            name,
            description,
            memory,
            tools: self.tools,
            tool_adapters,
            policy: self.policy,
            config: self.config,
            replay_capture,
            event_emitter,
            state: RwLock::new(AgentExecutionState::default()),
        })
    }
}

impl Default for ThymosAgentCoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, MemoryMode};
    use crate::tools::{ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema};
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
    async fn test_builder_requires_name() {
        let (memory, _temp_dir) = create_test_memory().await;
        let result = ThymosAgentCore::builder()
            .description("Test description")
            .memory(memory)
            .build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_builder_requires_description() {
        let (memory, _temp_dir) = create_test_memory().await;
        let result = ThymosAgentCore::builder()
            .name("test_agent")
            .memory(memory)
            .build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_builder_requires_memory() {
        let result = ThymosAgentCore::builder()
            .name("test_agent")
            .description("Test description")
            .build();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_successful_build() {
        let (memory, _temp_dir) = create_test_memory().await;
        let agent = ThymosAgentCore::builder()
            .name("test_agent")
            .description("A test agent")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        assert_eq!(agent.name(), "test_agent");
        assert_eq!(agent.description(), "A test agent");
    }

    #[tokio::test]
    async fn test_agent_with_tools() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let agent = ThymosAgentCore::builder()
            .name("tool_agent")
            .description("An agent with tools")
            .memory(memory)
            .tool(tool)
            .policy(CapabilityPolicy::allow_all())
            .build()
            .expect("Failed to build agent");

        assert!(agent.find_tool("echo").is_some());
        assert!(agent.find_tool("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_tool_execution_with_policy() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let agent = ThymosAgentCore::builder()
            .name("policy_agent")
            .description("An agent with policy")
            .memory(memory)
            .tool(tool)
            .policy(CapabilityPolicy::allow_all())
            .build()
            .expect("Failed to build agent");

        let result = agent
            .execute_tool("echo", serde_json::json!({"message": "hello"}))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_derive_t_impl() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let agent = ThymosAgentCore::builder()
            .name("derive_agent")
            .description("Testing AgentDeriveT")
            .memory(memory)
            .tool(tool)
            .build()
            .expect("Failed to build agent");

        // Test AgentDeriveT implementation
        assert_eq!(AgentDeriveT::name(&agent), "derive_agent");
        assert_eq!(AgentDeriveT::description(&agent), "Testing AgentDeriveT");
        
        let tools = AgentDeriveT::tools(&agent);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "echo");

        assert!(agent.output_schema().is_some());
    }

    #[tokio::test]
    async fn test_remember_and_search() {
        let (memory, _temp_dir) = create_test_memory().await;
        
        let agent = ThymosAgentCore::builder()
            .name("memory_agent")
            .description("Testing memory")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        agent.remember("Important fact").await.expect("Failed to remember");
        
        // Note: Search results depend on indexing timing
        let results = agent.search_memories("important", Some(10)).await;
        assert!(results.is_ok());
    }

    #[tokio::test]
    async fn test_executor_config() {
        let (memory, _temp_dir) = create_test_memory().await;

        let config = ThymosAgentConfig::new().with_max_turns(20);

        let agent = ThymosAgentCore::builder()
            .name("config_agent")
            .description("Testing config")
            .memory(memory)
            .config(config)
            .build()
            .expect("Failed to build agent");

        assert_eq!(agent.config().max_turns, 20);
        assert_eq!(AgentExecutor::config(&agent).max_turns, 20);
    }

    #[tokio::test]
    async fn test_agent_with_event_emitter() {
        use crate::integration::event_channel;

        let (memory, _temp_dir) = create_test_memory().await;
        let (tx, _rx) = event_channel(10);

        let agent = ThymosAgentCore::builder()
            .name("event_agent")
            .description("An agent with event emitter")
            .memory(memory)
            .event_sender(tx)
            .build()
            .expect("Failed to build agent");

        assert!(agent.event_emitter().is_some());
    }

    #[tokio::test]
    async fn test_agent_without_event_emitter() {
        let (memory, _temp_dir) = create_test_memory().await;

        let agent = ThymosAgentCore::builder()
            .name("no_event_agent")
            .description("An agent without event emitter")
            .memory(memory)
            .build()
            .expect("Failed to build agent");

        assert!(agent.event_emitter().is_none());
    }
}


