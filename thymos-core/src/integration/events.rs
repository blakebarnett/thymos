//! Agent Event Protocol for Real-Time Observability
//!
//! This module provides a structured event system for monitoring agent execution
//! in real-time. Events are emitted at task, turn, and tool boundaries, enabling:
//!
//! - Real-time dashboards and monitoring
//! - Streaming progress updates to clients
//! - Event-driven integrations and webhooks
//! - Debugging and observability
//!
//! # Event Types
//!
//! - [`AgentEvent::TaskStarted`] - Emitted when a task begins execution
//! - [`AgentEvent::TaskCompleted`] - Emitted when a task finishes (success or failure)
//! - [`AgentEvent::TurnStarted`] - Emitted at the start of each execution turn
//! - [`AgentEvent::TurnCompleted`] - Emitted when a turn finishes
//! - [`AgentEvent::ToolCallStarted`] - Emitted before tool execution
//! - [`AgentEvent::ToolCallCompleted`] - Emitted after tool execution
//! - [`AgentEvent::StreamChunk`] - Emitted for streaming LLM responses
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::integration::{EventChannel, AgentEvent};
//!
//! let (tx, mut rx) = EventChannel::new(100);
//!
//! // Subscribe to events
//! tokio::spawn(async move {
//!     while let Some(event) = rx.recv().await {
//!         match event {
//!             AgentEvent::TaskStarted { task_id, .. } => {
//!                 println!("Task {} started", task_id);
//!             }
//!             AgentEvent::ToolCallCompleted { tool_name, success, .. } => {
//!                 println!("Tool {} completed: {}", tool_name, success);
//!             }
//!             _ => {}
//!         }
//!     }
//! });
//!
//! // Events are emitted automatically by ThymosAgentCore when configured
//! let agent = ThymosAgentCore::builder()
//!     .name("my_agent")
//!     .event_sender(tx)
//!     .build()?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Events emitted during agent execution for real-time observability.
///
/// These events provide structured information about the agent's execution
/// lifecycle, enabling monitoring, debugging, and integration with external systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Emitted when a task begins execution
    TaskStarted {
        /// Unique identifier for this task execution
        task_id: String,
        /// The agent processing this task
        agent_name: String,
        /// The task prompt/description
        prompt: String,
        /// When the task started
        timestamp: DateTime<Utc>,
    },

    /// Emitted when a task completes (successfully or with error)
    TaskCompleted {
        /// Unique identifier for this task execution
        task_id: String,
        /// The agent that processed this task
        agent_name: String,
        /// Whether the task completed successfully
        success: bool,
        /// The final response (if successful)
        response: Option<String>,
        /// Error message (if failed)
        error: Option<String>,
        /// Total number of turns taken
        turns: usize,
        /// Total number of tool calls made
        tool_calls: usize,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// When the task completed
        timestamp: DateTime<Utc>,
    },

    /// Emitted at the start of each execution turn
    TurnStarted {
        /// Task this turn belongs to
        task_id: String,
        /// The agent executing
        agent_name: String,
        /// Turn index (0-based)
        turn_index: usize,
        /// When the turn started
        timestamp: DateTime<Utc>,
    },

    /// Emitted when a turn completes
    TurnCompleted {
        /// Task this turn belongs to
        task_id: String,
        /// The agent executing
        agent_name: String,
        /// Turn index (0-based)
        turn_index: usize,
        /// Whether the turn resulted in tool calls
        had_tool_calls: bool,
        /// Whether this was the final turn
        is_final: bool,
        /// When the turn completed
        timestamp: DateTime<Utc>,
    },

    /// Emitted before a tool is executed
    ToolCallStarted {
        /// Task this tool call belongs to
        task_id: String,
        /// The agent making the call
        agent_name: String,
        /// Name of the tool being called
        tool_name: String,
        /// Arguments passed to the tool (may be redacted for security)
        arguments: Value,
        /// When the tool call started
        timestamp: DateTime<Utc>,
    },

    /// Emitted after a tool completes execution
    ToolCallCompleted {
        /// Task this tool call belongs to
        task_id: String,
        /// The agent that made the call
        agent_name: String,
        /// Name of the tool that was called
        tool_name: String,
        /// Whether the tool call succeeded
        success: bool,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// Error message if failed (result not included for security)
        error: Option<String>,
        /// When the tool call completed
        timestamp: DateTime<Utc>,
    },

    /// Emitted for streaming LLM response chunks
    StreamChunk {
        /// Task this chunk belongs to
        task_id: String,
        /// The agent generating the response
        agent_name: String,
        /// The text chunk
        chunk: String,
        /// Whether this is the final chunk
        is_final: bool,
        /// When the chunk was received
        timestamp: DateTime<Utc>,
    },

    /// Emitted when a policy blocks a tool call
    PolicyBlocked {
        /// Task where the block occurred
        task_id: String,
        /// The agent that was blocked
        agent_name: String,
        /// The tool that was blocked
        tool_name: String,
        /// The capabilities that were denied
        denied_capabilities: Vec<String>,
        /// When the block occurred
        timestamp: DateTime<Utc>,
    },

    /// Custom event for extension
    Custom {
        /// Task context (if applicable)
        task_id: Option<String>,
        /// Agent context (if applicable)
        agent_name: Option<String>,
        /// Event name
        name: String,
        /// Event data
        data: Value,
        /// When the event occurred
        timestamp: DateTime<Utc>,
    },
}

impl AgentEvent {
    /// Get the event type as a string
    pub fn event_type(&self) -> &'static str {
        match self {
            AgentEvent::TaskStarted { .. } => "task_started",
            AgentEvent::TaskCompleted { .. } => "task_completed",
            AgentEvent::TurnStarted { .. } => "turn_started",
            AgentEvent::TurnCompleted { .. } => "turn_completed",
            AgentEvent::ToolCallStarted { .. } => "tool_call_started",
            AgentEvent::ToolCallCompleted { .. } => "tool_call_completed",
            AgentEvent::StreamChunk { .. } => "stream_chunk",
            AgentEvent::PolicyBlocked { .. } => "policy_blocked",
            AgentEvent::Custom { .. } => "custom",
        }
    }

    /// Get the timestamp of the event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            AgentEvent::TaskStarted { timestamp, .. }
            | AgentEvent::TaskCompleted { timestamp, .. }
            | AgentEvent::TurnStarted { timestamp, .. }
            | AgentEvent::TurnCompleted { timestamp, .. }
            | AgentEvent::ToolCallStarted { timestamp, .. }
            | AgentEvent::ToolCallCompleted { timestamp, .. }
            | AgentEvent::StreamChunk { timestamp, .. }
            | AgentEvent::PolicyBlocked { timestamp, .. }
            | AgentEvent::Custom { timestamp, .. } => *timestamp,
        }
    }

    /// Get the task_id if present
    pub fn task_id(&self) -> Option<&str> {
        match self {
            AgentEvent::TaskStarted { task_id, .. }
            | AgentEvent::TaskCompleted { task_id, .. }
            | AgentEvent::TurnStarted { task_id, .. }
            | AgentEvent::TurnCompleted { task_id, .. }
            | AgentEvent::ToolCallStarted { task_id, .. }
            | AgentEvent::ToolCallCompleted { task_id, .. }
            | AgentEvent::StreamChunk { task_id, .. }
            | AgentEvent::PolicyBlocked { task_id, .. } => Some(task_id),
            AgentEvent::Custom { task_id, .. } => task_id.as_deref(),
        }
    }

    /// Get the agent_name if present
    pub fn agent_name(&self) -> Option<&str> {
        match self {
            AgentEvent::TaskStarted { agent_name, .. }
            | AgentEvent::TaskCompleted { agent_name, .. }
            | AgentEvent::TurnStarted { agent_name, .. }
            | AgentEvent::TurnCompleted { agent_name, .. }
            | AgentEvent::ToolCallStarted { agent_name, .. }
            | AgentEvent::ToolCallCompleted { agent_name, .. }
            | AgentEvent::StreamChunk { agent_name, .. }
            | AgentEvent::PolicyBlocked { agent_name, .. } => Some(agent_name),
            AgentEvent::Custom { agent_name, .. } => agent_name.as_deref(),
        }
    }
}

/// Sender half of an event channel
pub type EventSender = mpsc::Sender<AgentEvent>;

/// Receiver half of an event channel
pub type EventReceiver = mpsc::Receiver<AgentEvent>;

/// Creates a new event channel with the specified buffer capacity.
///
/// Returns a tuple of (sender, receiver) that can be used to emit and
/// receive agent events.
///
/// # Arguments
///
/// * `buffer_size` - The capacity of the channel buffer. Use a larger
///   buffer for high-throughput scenarios.
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::integration::event_channel;
///
/// let (tx, mut rx) = event_channel(100);
///
/// // Send events
/// tx.send(AgentEvent::TaskStarted { ... }).await?;
///
/// // Receive events
/// while let Some(event) = rx.recv().await {
///     println!("Got event: {:?}", event);
/// }
/// ```
pub fn event_channel(buffer_size: usize) -> (EventSender, EventReceiver) {
    mpsc::channel(buffer_size)
}

/// A clonable event emitter that wraps an EventSender.
///
/// This type can be shared across threads and cloned to allow multiple
/// components to emit events to the same channel.
#[derive(Clone)]
pub struct EventEmitter {
    sender: EventSender,
    agent_name: String,
    current_task_id: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl EventEmitter {
    /// Create a new event emitter
    pub fn new(sender: EventSender, agent_name: impl Into<String>) -> Self {
        Self {
            sender,
            agent_name: agent_name.into(),
            current_task_id: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Set the current task ID for subsequent events
    pub async fn set_task_id(&self, task_id: Option<String>) {
        let mut current = self.current_task_id.write().await;
        *current = task_id;
    }

    /// Get the current task ID
    pub async fn task_id(&self) -> Option<String> {
        self.current_task_id.read().await.clone()
    }

    /// Emit a task started event
    pub async fn task_started(&self, task_id: &str, prompt: &str) {
        self.set_task_id(Some(task_id.to_string())).await;
        let _ = self
            .sender
            .send(AgentEvent::TaskStarted {
                task_id: task_id.to_string(),
                agent_name: self.agent_name.clone(),
                prompt: prompt.to_string(),
                timestamp: Utc::now(),
            })
            .await;
    }

    /// Emit a task completed event
    #[allow(clippy::too_many_arguments)]
    pub async fn task_completed(
        &self,
        task_id: &str,
        success: bool,
        response: Option<String>,
        error: Option<String>,
        turns: usize,
        tool_calls: usize,
        duration_ms: u64,
    ) {
        let _ = self
            .sender
            .send(AgentEvent::TaskCompleted {
                task_id: task_id.to_string(),
                agent_name: self.agent_name.clone(),
                success,
                response,
                error,
                turns,
                tool_calls,
                duration_ms,
                timestamp: Utc::now(),
            })
            .await;
        self.set_task_id(None).await;
    }

    /// Emit a turn started event
    pub async fn turn_started(&self, turn_index: usize) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::TurnStarted {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    turn_index,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a turn completed event
    pub async fn turn_completed(&self, turn_index: usize, had_tool_calls: bool, is_final: bool) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::TurnCompleted {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    turn_index,
                    had_tool_calls,
                    is_final,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a tool call started event
    pub async fn tool_call_started(&self, tool_name: &str, arguments: Value) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::ToolCallStarted {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    tool_name: tool_name.to_string(),
                    arguments,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a tool call completed event
    pub async fn tool_call_completed(
        &self,
        tool_name: &str,
        success: bool,
        duration_ms: u64,
        error: Option<String>,
    ) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::ToolCallCompleted {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    tool_name: tool_name.to_string(),
                    success,
                    duration_ms,
                    error,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a stream chunk event
    pub async fn stream_chunk(&self, chunk: &str, is_final: bool) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::StreamChunk {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    chunk: chunk.to_string(),
                    is_final,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a policy blocked event
    pub async fn policy_blocked(&self, tool_name: &str, denied_capabilities: Vec<String>) {
        if let Some(task_id) = self.task_id().await {
            let _ = self
                .sender
                .send(AgentEvent::PolicyBlocked {
                    task_id,
                    agent_name: self.agent_name.clone(),
                    tool_name: tool_name.to_string(),
                    denied_capabilities,
                    timestamp: Utc::now(),
                })
                .await;
        }
    }

    /// Emit a custom event
    pub async fn custom(&self, name: &str, data: Value) {
        let _ = self
            .sender
            .send(AgentEvent::Custom {
                task_id: self.task_id().await,
                agent_name: Some(self.agent_name.clone()),
                name: name.to_string(),
                data,
                timestamp: Utc::now(),
            })
            .await;
    }

    /// Emit a raw event
    pub async fn emit(&self, event: AgentEvent) {
        let _ = self.sender.send(event).await;
    }
}

impl std::fmt::Debug for EventEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventEmitter")
            .field("agent_name", &self.agent_name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let event = AgentEvent::TaskStarted {
            task_id: "task-1".to_string(),
            agent_name: "agent-1".to_string(),
            prompt: "Hello".to_string(),
            timestamp: Utc::now(),
        };
        assert_eq!(event.event_type(), "task_started");

        let event = AgentEvent::ToolCallCompleted {
            task_id: "task-1".to_string(),
            agent_name: "agent-1".to_string(),
            tool_name: "search".to_string(),
            success: true,
            duration_ms: 100,
            error: None,
            timestamp: Utc::now(),
        };
        assert_eq!(event.event_type(), "tool_call_completed");
    }

    #[test]
    fn test_event_task_id() {
        let event = AgentEvent::TurnStarted {
            task_id: "task-123".to_string(),
            agent_name: "agent-1".to_string(),
            turn_index: 0,
            timestamp: Utc::now(),
        };
        assert_eq!(event.task_id(), Some("task-123"));

        let event = AgentEvent::Custom {
            task_id: None,
            agent_name: Some("agent-1".to_string()),
            name: "custom".to_string(),
            data: Value::Null,
            timestamp: Utc::now(),
        };
        assert_eq!(event.task_id(), None);
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentEvent::TaskCompleted {
            task_id: "task-1".to_string(),
            agent_name: "agent-1".to_string(),
            success: true,
            response: Some("Done".to_string()),
            error: None,
            turns: 3,
            tool_calls: 2,
            duration_ms: 5000,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"task_completed\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"turns\":3"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "task_completed");
    }

    #[tokio::test]
    async fn test_event_channel() {
        let (tx, mut rx) = event_channel(10);

        tx.send(AgentEvent::TaskStarted {
            task_id: "task-1".to_string(),
            agent_name: "test".to_string(),
            prompt: "Hello".to_string(),
            timestamp: Utc::now(),
        })
        .await
        .unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type(), "task_started");
        assert_eq!(event.task_id(), Some("task-1"));
    }

    #[tokio::test]
    async fn test_event_emitter() {
        let (tx, mut rx) = event_channel(10);
        let emitter = EventEmitter::new(tx, "test_agent");

        emitter.task_started("task-1", "Do something").await;
        emitter.turn_started(0).await;
        emitter
            .tool_call_started("search", serde_json::json!({"query": "test"}))
            .await;
        emitter.tool_call_completed("search", true, 100, None).await;
        emitter.turn_completed(0, true, false).await;
        emitter
            .task_completed("task-1", true, Some("Done".to_string()), None, 1, 1, 500)
            .await;

        // Collect all events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert_eq!(events.len(), 6);
        assert_eq!(events[0].event_type(), "task_started");
        assert_eq!(events[1].event_type(), "turn_started");
        assert_eq!(events[2].event_type(), "tool_call_started");
        assert_eq!(events[3].event_type(), "tool_call_completed");
        assert_eq!(events[4].event_type(), "turn_completed");
        assert_eq!(events[5].event_type(), "task_completed");
    }

    #[tokio::test]
    async fn test_event_emitter_task_id_tracking() {
        let (tx, _rx) = event_channel(10);
        let emitter = EventEmitter::new(tx, "test_agent");

        // No task ID initially
        assert!(emitter.task_id().await.is_none());

        // Set task ID
        emitter.set_task_id(Some("task-1".to_string())).await;
        assert_eq!(emitter.task_id().await, Some("task-1".to_string()));

        // Clear task ID
        emitter.set_task_id(None).await;
        assert!(emitter.task_id().await.is_none());
    }

    #[test]
    fn test_policy_blocked_event() {
        let event = AgentEvent::PolicyBlocked {
            task_id: "task-1".to_string(),
            agent_name: "agent-1".to_string(),
            tool_name: "dangerous_tool".to_string(),
            denied_capabilities: vec!["network".to_string(), "filesystem".to_string()],
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "policy_blocked");

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("dangerous_tool"));
        assert!(json.contains("network"));
    }

    #[test]
    fn test_stream_chunk_event() {
        let event = AgentEvent::StreamChunk {
            task_id: "task-1".to_string(),
            agent_name: "agent-1".to_string(),
            chunk: "Hello, ".to_string(),
            is_final: false,
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "stream_chunk");
        assert_eq!(event.agent_name(), Some("agent-1"));
    }
}

