//! Replay record types and persistence
//!
//! Defines the structure of replay records and individual events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;

use super::REPLAY_SCHEMA_VERSION;
use crate::tools::{ToolErrorKind, ToolResultEnvelope};

/// A complete replay record containing all events from a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// Schema version for forward compatibility
    pub schema_version: u32,

    /// Unique session identifier
    pub session_id: String,

    /// Agent ID that generated this record
    pub agent_id: Option<String>,

    /// When the session started
    pub started_at: DateTime<Utc>,

    /// When the session ended (if finished)
    pub ended_at: Option<DateTime<Utc>>,

    /// All events in order
    pub events: Vec<ReplayEventEnvelope>,

    /// Metadata about the session
    pub metadata: SessionMetadata,
}

impl ReplayRecord {
    /// Create a new empty replay record
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            schema_version: REPLAY_SCHEMA_VERSION,
            session_id: session_id.into(),
            agent_id: None,
            started_at: Utc::now(),
            ended_at: None,
            events: Vec::new(),
            metadata: SessionMetadata::default(),
        }
    }

    /// Set the agent ID
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Add an event to the record
    pub fn push_event(&mut self, event: ReplayEvent) {
        self.events.push(ReplayEventEnvelope {
            sequence: self.events.len() as u64,
            timestamp: Utc::now(),
            event,
        });
    }

    /// Mark the session as ended
    pub fn finish(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Get the total duration of the session
    pub fn duration(&self) -> Option<Duration> {
        self.ended_at.map(|end| {
            (end - self.started_at)
                .to_std()
                .unwrap_or(Duration::ZERO)
        })
    }

    /// Save the record to a JSON-lines file
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);

        // Write header line with metadata
        let header = serde_json::json!({
            "type": "header",
            "schema_version": self.schema_version,
            "session_id": self.session_id,
            "agent_id": self.agent_id,
            "started_at": self.started_at,
            "ended_at": self.ended_at,
            "metadata": self.metadata,
        });
        writeln!(writer, "{}", serde_json::to_string(&header)?)?;

        // Write each event as a line
        for envelope in &self.events {
            writeln!(writer, "{}", serde_json::to_string(envelope)?)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Load a record from a JSON-lines file
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read header
        let header_line = lines
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Empty file"))??;

        let header: Value = serde_json::from_str(&header_line)?;

        let mut record = ReplayRecord {
            schema_version: header["schema_version"].as_u64().unwrap_or(1) as u32,
            session_id: header["session_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            agent_id: header["agent_id"].as_str().map(|s| s.to_string()),
            started_at: header["started_at"]
                .as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            ended_at: header["ended_at"]
                .as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            events: Vec::new(),
            metadata: serde_json::from_value(header["metadata"].clone()).unwrap_or_default(),
        };

        // Read events
        for line in lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let envelope: ReplayEventEnvelope = serde_json::from_str(&line)?;
            record.events.push(envelope);
        }

        Ok(record)
    }

    /// Get events of a specific type
    pub fn events_of_type<F>(&self, filter: F) -> impl Iterator<Item = &ReplayEventEnvelope>
    where
        F: Fn(&ReplayEvent) -> bool,
    {
        self.events.iter().filter(move |e| filter(&e.event))
    }

    /// Get all tool call events
    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCallEvent> {
        self.events.iter().filter_map(|e| match &e.event {
            ReplayEvent::ToolCall(tc) => Some(tc),
            _ => None,
        })
    }

    /// Get all LLM call events
    pub fn llm_calls(&self) -> impl Iterator<Item = &LlmCallEvent> {
        self.events.iter().filter_map(|e| match &e.event {
            ReplayEvent::LlmCall(lc) => Some(lc),
            _ => None,
        })
    }

    /// Get all memory retrieval events
    pub fn memory_retrievals(&self) -> impl Iterator<Item = &MemoryRetrievalEvent> {
        self.events.iter().filter_map(|e| match &e.event {
            ReplayEvent::MemoryRetrieval(mr) => Some(mr),
            _ => None,
        })
    }
}

/// Metadata about the recording session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Description of the session
    pub description: Option<String>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Whether secrets were redacted
    pub secrets_redacted: bool,

    /// Model configuration (if any)
    pub model_config: Option<ModelConfig>,

    /// Custom metadata
    pub extra: Value,
}

/// Model configuration snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Provider name
    pub provider: String,

    /// Model name
    pub model: String,

    /// Temperature setting
    pub temperature: Option<f32>,

    /// Max tokens setting
    pub max_tokens: Option<u32>,

    /// Other parameters
    pub parameters: Value,
}

/// Envelope wrapping each event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEventEnvelope {
    /// Sequence number within the session
    pub sequence: u64,

    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// The event data
    pub event: ReplayEvent,
}

/// Types of events that can be recorded
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplayEvent {
    /// Session started
    SessionStart(SessionEvent),

    /// Session ended
    SessionEnd(SessionEvent),

    /// Tool was invoked
    ToolCall(ToolCallEvent),

    /// LLM was called
    LlmCall(LlmCallEvent),

    /// Memory was retrieved
    MemoryRetrieval(MemoryRetrievalEvent),

    /// Memory versioning operation
    MemoryVersioning(MemoryVersioningEvent),

    /// Custom event
    Custom {
        /// Event name
        name: String,
        /// Event data
        data: Value,
    },
}

/// Session start/end event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Session ID
    pub session_id: String,

    /// Agent ID
    pub agent_id: Option<String>,

    /// Additional context
    pub context: Value,
}

/// Tool call event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEvent {
    /// Tool name
    pub tool_name: String,

    /// Tool version (if available)
    pub tool_version: Option<String>,

    /// Input arguments (may be redacted)
    pub args: Value,

    /// Hash of input arguments (for replay matching)
    pub args_hash: String,

    /// Result status
    pub status: ToolCallStatus,

    /// Result value (if successful, may be redacted)
    pub result: Option<Value>,

    /// Error details (if failed)
    pub error: Option<ToolCallError>,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Trace ID for correlation
    pub trace_id: Option<String>,
}

impl ToolCallEvent {
    /// Create from a tool result envelope
    pub fn from_envelope(
        tool_name: impl Into<String>,
        args: Value,
        envelope: &ToolResultEnvelope,
    ) -> Self {
        let status = if envelope.is_success() {
            ToolCallStatus::Success
        } else if envelope.is_cancelled() {
            ToolCallStatus::Cancelled
        } else {
            ToolCallStatus::Error
        };

        let error = envelope.get_error().map(|e| ToolCallError {
            kind: e.kind,
            message: e.message.clone(),
            code: e.code.clone(),
        });

        Self {
            tool_name: tool_name.into(),
            tool_version: envelope.provenance.tool_version.clone(),
            args,
            args_hash: envelope.provenance.args_hash.clone(),
            status,
            result: envelope.value().cloned(),
            error,
            duration_ms: envelope.provenance.duration.as_millis() as u64,
            trace_id: envelope.provenance.trace_id.clone(),
        }
    }
}

/// Tool call status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Success,
    Error,
    Cancelled,
}

/// Tool call error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallError {
    /// Error kind
    pub kind: ToolErrorKind,

    /// Error message
    pub message: String,

    /// Error code
    pub code: Option<String>,
}

/// LLM call event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallEvent {
    /// Provider name
    pub provider: String,

    /// Model name
    pub model: String,

    /// Input tokens (if available)
    pub input_tokens: Option<u32>,

    /// Output tokens (if available)
    pub output_tokens: Option<u32>,

    /// Prompt hash (for replay matching, not the actual prompt)
    pub prompt_hash: String,

    /// Whether this was a streaming call
    pub streaming: bool,

    /// Response latency in milliseconds
    pub latency_ms: u64,

    /// Estimated cost in USD (if available)
    pub cost_usd: Option<f64>,

    /// Trace ID for correlation
    pub trace_id: Option<String>,

    /// Temperature used
    pub temperature: Option<f32>,

    /// Stop reason
    pub stop_reason: Option<String>,
}

/// Memory retrieval event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRetrievalEvent {
    /// Query used for retrieval
    pub query: String,

    /// Query hash (for replay matching)
    pub query_hash: String,

    /// Number of results returned
    pub result_count: usize,

    /// IDs/hashes of returned memories (for replay matching)
    pub result_ids: Vec<String>,

    /// Search scope (if applicable)
    pub scope: Option<String>,

    /// Retrieval latency in milliseconds
    pub latency_ms: u64,

    /// Trace ID for correlation
    pub trace_id: Option<String>,
}

/// Memory versioning event (branch, commit, checkout, merge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersioningEvent {
    /// Operation type
    pub operation: VersioningOperation,

    /// Branch name (if applicable)
    pub branch: Option<String>,

    /// Commit hash (if applicable)
    pub commit_hash: Option<String>,

    /// Worktree ID (if applicable)
    pub worktree_id: Option<String>,

    /// Merge source (if merge operation)
    pub merge_source: Option<String>,

    /// Whether operation succeeded
    pub success: bool,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Trace ID for correlation
    pub trace_id: Option<String>,
}

/// Versioning operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersioningOperation {
    BranchCreate,
    BranchDelete,
    Commit,
    Checkout,
    Merge,
    WorktreeCreate,
    WorktreeRemove,
}

#[cfg(test)]
mod record_tests {
    use super::*;

    #[test]
    fn test_replay_record_creation() {
        let mut record = ReplayRecord::new("test_session")
            .with_agent_id("agent_1");

        record.push_event(ReplayEvent::SessionStart(SessionEvent {
            session_id: "test_session".to_string(),
            agent_id: Some("agent_1".to_string()),
            context: Value::Null,
        }));

        assert_eq!(record.session_id, "test_session");
        assert_eq!(record.agent_id, Some("agent_1".to_string()));
        assert_eq!(record.events.len(), 1);
    }

    #[test]
    fn test_event_serialization() {
        let event = ReplayEvent::ToolCall(ToolCallEvent {
            tool_name: "test_tool".to_string(),
            tool_version: Some("1.0.0".to_string()),
            args: serde_json::json!({"key": "value"}),
            args_hash: "abc123".to_string(),
            status: ToolCallStatus::Success,
            result: Some(serde_json::json!({"result": 42})),
            error: None,
            duration_ms: 150,
            trace_id: Some("trace_1".to_string()),
        });

        let envelope = ReplayEventEnvelope {
            sequence: 0,
            timestamp: Utc::now(),
            event,
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ReplayEventEnvelope = serde_json::from_str(&json).unwrap();

        match parsed.event {
            ReplayEvent::ToolCall(tc) => {
                assert_eq!(tc.tool_name, "test_tool");
                assert_eq!(tc.status, ToolCallStatus::Success);
            }
            _ => panic!("Expected ToolCall event"),
        }
    }

    #[test]
    fn test_record_save_load() {
        let mut record = ReplayRecord::new("test_save_load")
            .with_agent_id("agent_1");

        record.metadata.description = Some("Test session".to_string());
        record.metadata.tags = vec!["test".to_string()];

        record.push_event(ReplayEvent::SessionStart(SessionEvent {
            session_id: "test_save_load".to_string(),
            agent_id: Some("agent_1".to_string()),
            context: Value::Null,
        }));

        record.push_event(ReplayEvent::ToolCall(ToolCallEvent {
            tool_name: "echo".to_string(),
            tool_version: None,
            args: serde_json::json!({"message": "hello"}),
            args_hash: "hash123".to_string(),
            status: ToolCallStatus::Success,
            result: Some(serde_json::json!("hello")),
            error: None,
            duration_ms: 10,
            trace_id: None,
        }));

        record.finish();

        // Save to temp file
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("test_record.jsonl");

        record.save(&path).unwrap();

        // Load back
        let loaded = ReplayRecord::load(&path).unwrap();

        assert_eq!(loaded.session_id, "test_save_load");
        assert_eq!(loaded.agent_id, Some("agent_1".to_string()));
        assert_eq!(loaded.events.len(), 2);
        assert!(loaded.ended_at.is_some());
    }

    #[test]
    fn test_event_filtering() {
        let mut record = ReplayRecord::new("filter_test");

        record.push_event(ReplayEvent::ToolCall(ToolCallEvent {
            tool_name: "tool1".to_string(),
            tool_version: None,
            args: Value::Null,
            args_hash: "h1".to_string(),
            status: ToolCallStatus::Success,
            result: None,
            error: None,
            duration_ms: 10,
            trace_id: None,
        }));

        record.push_event(ReplayEvent::LlmCall(LlmCallEvent {
            provider: "test".to_string(),
            model: "gpt-4".to_string(),
            input_tokens: Some(100),
            output_tokens: Some(50),
            prompt_hash: "ph1".to_string(),
            streaming: false,
            latency_ms: 500,
            cost_usd: Some(0.01),
            trace_id: None,
            temperature: Some(0.7),
            stop_reason: Some("stop".to_string()),
        }));

        record.push_event(ReplayEvent::ToolCall(ToolCallEvent {
            tool_name: "tool2".to_string(),
            tool_version: None,
            args: Value::Null,
            args_hash: "h2".to_string(),
            status: ToolCallStatus::Error,
            result: None,
            error: Some(ToolCallError {
                kind: ToolErrorKind::Timeout,
                message: "Timed out".to_string(),
                code: None,
            }),
            duration_ms: 30000,
            trace_id: None,
        }));

        let tool_calls: Vec<_> = record.tool_calls().collect();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].tool_name, "tool1");
        assert_eq!(tool_calls[1].tool_name, "tool2");

        let llm_calls: Vec<_> = record.llm_calls().collect();
        assert_eq!(llm_calls.len(), 1);
        assert_eq!(llm_calls[0].model, "gpt-4");
    }
}



