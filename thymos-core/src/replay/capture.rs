//! Capture hooks for recording agent execution
//!
//! Provides a tracer that can be attached to agent execution to capture
//! events for replay. Supports different recording modes and redaction policies.

use super::record::{
    LlmCallEvent, MemoryRetrievalEvent, MemoryVersioningEvent, ReplayEvent, ReplayRecord,
    SessionEvent, SessionMetadata, ToolCallEvent, VersioningOperation,
};
use crate::tools::ToolResultEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Recording mode for capture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    /// Record everything (use with caution - may include secrets)
    Full,

    /// Record with automatic secret redaction (default)
    #[default]
    Redacted,

    /// Record only hashes and metadata (no actual data)
    HashesOnly,

    /// Disabled - no recording
    Disabled,
}

/// Configuration for capture behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Recording mode
    pub mode: RecordingMode,

    /// Patterns for fields that should be redacted (regex patterns)
    pub redact_patterns: Vec<String>,

    /// Maximum size of data to record (larger values are truncated)
    pub max_data_size: usize,

    /// Whether to record LLM prompts (can be large)
    pub record_prompts: bool,

    /// Whether to record tool results (can be large)
    pub record_tool_results: bool,

    /// Whether to record memory content
    pub record_memory_content: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            mode: RecordingMode::Redacted,
            redact_patterns: vec![
                r"(?i)password".to_string(),
                r"(?i)secret".to_string(),
                r"(?i)api_key".to_string(),
                r"(?i)token".to_string(),
                r"(?i)auth".to_string(),
            ],
            max_data_size: 10_000,
            record_prompts: false, // Prompts can be very large
            record_tool_results: true,
            record_memory_content: false,
        }
    }
}

/// Trait for objects that can receive replay events
pub trait ReplayCaptureHooks: Send + Sync {
    /// Called when a tool is invoked
    fn on_tool_call(&self, event: ToolCallEvent);

    /// Called when an LLM is called
    fn on_llm_call(&self, event: LlmCallEvent);

    /// Called when memory is retrieved
    fn on_memory_retrieval(&self, event: MemoryRetrievalEvent);

    /// Called when a versioning operation occurs
    fn on_versioning(&self, event: MemoryVersioningEvent);

    /// Called for custom events
    fn on_custom(&self, name: String, data: Value);
}

/// Active capture session that records events
pub struct ReplayCapture {
    record: Arc<RwLock<ReplayRecord>>,
    config: CaptureConfig,
    enabled: bool,
}

impl ReplayCapture {
    /// Create a new capture session
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            record: Arc::new(RwLock::new(ReplayRecord::new(session_id))),
            config: CaptureConfig::default(),
            enabled: true,
        }
    }

    /// Create a capture session with custom config
    pub fn with_config(session_id: impl Into<String>, config: CaptureConfig) -> Self {
        let enabled = config.mode != RecordingMode::Disabled;
        Self {
            record: Arc::new(RwLock::new(ReplayRecord::new(session_id))),
            config,
            enabled,
        }
    }

    /// Set the agent ID
    pub async fn set_agent_id(&self, agent_id: impl Into<String>) {
        let mut record = self.record.write().await;
        record.agent_id = Some(agent_id.into());
    }

    /// Set session metadata
    pub async fn set_metadata(&self, metadata: SessionMetadata) {
        let mut record = self.record.write().await;
        record.metadata = metadata;
    }

    /// Check if capture is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable capture
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.config.mode != RecordingMode::Disabled;
    }

    /// Record session start
    pub async fn start_session(&self) {
        if !self.enabled {
            return;
        }

        let record = self.record.read().await;
        let event = ReplayEvent::SessionStart(SessionEvent {
            session_id: record.session_id.clone(),
            agent_id: record.agent_id.clone(),
            context: Value::Null,
        });
        drop(record);

        self.emit(event).await;
    }

    /// Record session end and return the completed record
    pub async fn end_session(&self) -> ReplayRecord {
        if self.enabled {
            let record = self.record.read().await;
            let event = ReplayEvent::SessionEnd(SessionEvent {
                session_id: record.session_id.clone(),
                agent_id: record.agent_id.clone(),
                context: Value::Null,
            });
            drop(record);

            self.emit(event).await;
        }

        let mut record = self.record.write().await;
        record.finish();
        record.clone()
    }

    /// Emit a raw event
    pub async fn emit(&self, event: ReplayEvent) {
        if !self.enabled {
            return;
        }

        let mut record = self.record.write().await;
        record.push_event(event);
    }

    /// Record a tool call from an envelope
    pub async fn record_tool_call(
        &self,
        tool_name: impl Into<String>,
        args: Value,
        envelope: &ToolResultEnvelope,
    ) {
        if !self.enabled {
            return;
        }

        let mut event = ToolCallEvent::from_envelope(tool_name, args.clone(), envelope);

        // Apply redaction if needed
        match self.config.mode {
            RecordingMode::Full => {
                // Keep everything
            }
            RecordingMode::Redacted => {
                event.args = self.redact_value(&args);
                if let Some(ref result) = event.result {
                    event.result = Some(self.redact_value(result));
                }
            }
            RecordingMode::HashesOnly => {
                event.args = Value::Null;
                event.result = None;
            }
            RecordingMode::Disabled => return,
        }

        // Apply size limits
        if !self.config.record_tool_results {
            event.result = None;
        }

        self.emit(ReplayEvent::ToolCall(event)).await;
    }

    /// Record an LLM call
    pub async fn record_llm_call(&self, event: LlmCallEvent) {
        if !self.enabled {
            return;
        }

        self.emit(ReplayEvent::LlmCall(event)).await;
    }

    /// Record a memory retrieval
    pub async fn record_memory_retrieval(
        &self,
        query: impl Into<String>,
        result_ids: Vec<String>,
        latency_ms: u64,
        trace_id: Option<String>,
    ) {
        if !self.enabled {
            return;
        }

        let query = query.into();
        let query_hash = Self::hash_string(&query);

        let event = MemoryRetrievalEvent {
            query: if self.config.mode == RecordingMode::Full {
                query
            } else {
                "[redacted]".to_string()
            },
            query_hash,
            result_count: result_ids.len(),
            result_ids,
            scope: None,
            latency_ms,
            trace_id,
        };

        self.emit(ReplayEvent::MemoryRetrieval(event)).await;
    }

    /// Record a versioning operation
    #[allow(clippy::too_many_arguments)]
    pub async fn record_versioning(
        &self,
        operation: VersioningOperation,
        branch: Option<String>,
        commit_hash: Option<String>,
        worktree_id: Option<String>,
        success: bool,
        error: Option<String>,
        trace_id: Option<String>,
    ) {
        if !self.enabled {
            return;
        }

        let event = MemoryVersioningEvent {
            operation,
            branch,
            commit_hash,
            worktree_id,
            merge_source: None,
            success,
            error,
            trace_id,
        };

        self.emit(ReplayEvent::MemoryVersioning(event)).await;
    }

    /// Record a custom event
    pub async fn record_custom(&self, name: impl Into<String>, data: Value) {
        if !self.enabled {
            return;
        }

        let data = match self.config.mode {
            RecordingMode::Full => data,
            RecordingMode::Redacted => self.redact_value(&data),
            RecordingMode::HashesOnly => Value::Null,
            RecordingMode::Disabled => return,
        };

        self.emit(ReplayEvent::Custom {
            name: name.into(),
            data,
        })
        .await;
    }

    /// Get a clone of the current record (for inspection)
    pub async fn current_record(&self) -> ReplayRecord {
        self.record.read().await.clone()
    }

    /// Get a reference to the record for read operations
    pub fn record(&self) -> Arc<RwLock<ReplayRecord>> {
        self.record.clone()
    }

    /// Redact sensitive values from a JSON value
    fn redact_value(&self, value: &Value) -> Value {
        // Simple redaction: replace string values matching patterns with [REDACTED]
        match value {
            Value::String(s) => {
                for pattern in &self.config.redact_patterns {
                    if regex::Regex::new(pattern).is_ok_and(|re| re.is_match(s)) {
                        return Value::String("[REDACTED]".to_string());
                    }
                }
                // Truncate if too long
                if s.len() > self.config.max_data_size {
                    Value::String(format!(
                        "{}... [truncated, {} bytes total]",
                        &s[..self.config.max_data_size],
                        s.len()
                    ))
                } else {
                    value.clone()
                }
            }
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    // Check if key matches redaction patterns
                    let should_redact = self.config.redact_patterns.iter().any(|pattern| {
                        regex::Regex::new(pattern)
                            .map(|re| re.is_match(k))
                            .unwrap_or(false)
                    });

                    if should_redact {
                        new_map.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                    } else {
                        new_map.insert(k.clone(), self.redact_value(v));
                    }
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.redact_value(v)).collect())
            }
            _ => value.clone(),
        }
    }

    /// Compute hash of a string
    fn hash_string(s: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(s.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}

impl ReplayCaptureHooks for ReplayCapture {
    fn on_tool_call(&self, event: ToolCallEvent) {
        let capture = self.record.clone();
        let enabled = self.enabled;
        tokio::spawn(async move {
            if enabled {
                let mut record = capture.write().await;
                record.push_event(ReplayEvent::ToolCall(event));
            }
        });
    }

    fn on_llm_call(&self, event: LlmCallEvent) {
        let capture = self.record.clone();
        let enabled = self.enabled;
        tokio::spawn(async move {
            if enabled {
                let mut record = capture.write().await;
                record.push_event(ReplayEvent::LlmCall(event));
            }
        });
    }

    fn on_memory_retrieval(&self, event: MemoryRetrievalEvent) {
        let capture = self.record.clone();
        let enabled = self.enabled;
        tokio::spawn(async move {
            if enabled {
                let mut record = capture.write().await;
                record.push_event(ReplayEvent::MemoryRetrieval(event));
            }
        });
    }

    fn on_versioning(&self, event: MemoryVersioningEvent) {
        let capture = self.record.clone();
        let enabled = self.enabled;
        tokio::spawn(async move {
            if enabled {
                let mut record = capture.write().await;
                record.push_event(ReplayEvent::MemoryVersioning(event));
            }
        });
    }

    fn on_custom(&self, name: String, data: Value) {
        let capture = self.record.clone();
        let enabled = self.enabled;
        tokio::spawn(async move {
            if enabled {
                let mut record = capture.write().await;
                record.push_event(ReplayEvent::Custom { name, data });
            }
        });
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use crate::tools::{ToolProvenance, ToolResult, ToolResultEnvelope};

    #[tokio::test]
    async fn test_capture_session() {
        let capture = ReplayCapture::new("test_session");
        capture.set_agent_id("agent_1").await;

        capture.start_session().await;

        // Record a tool call
        let provenance = ToolProvenance::new("test_tool", "hash123");
        let envelope = ToolResultEnvelope::success(serde_json::json!({"result": 42}), provenance);

        capture
            .record_tool_call("test_tool", serde_json::json!({"input": "test"}), &envelope)
            .await;

        let record = capture.end_session().await;

        assert_eq!(record.session_id, "test_session");
        assert_eq!(record.agent_id, Some("agent_1".to_string()));
        // SessionStart, ToolCall, SessionEnd = 3 events
        assert_eq!(record.events.len(), 3);
    }

    #[tokio::test]
    async fn test_redaction() {
        let config = CaptureConfig {
            mode: RecordingMode::Redacted,
            ..Default::default()
        };

        let capture = ReplayCapture::with_config("redaction_test", config);

        let sensitive_data = serde_json::json!({
            "username": "user123",
            "password": "secret123",
            "api_key": "sk-12345",
            "normal_field": "visible"
        });

        let redacted = capture.redact_value(&sensitive_data);

        assert_eq!(redacted["username"], "user123");
        assert_eq!(redacted["password"], "[REDACTED]");
        assert_eq!(redacted["api_key"], "[REDACTED]");
        assert_eq!(redacted["normal_field"], "visible");
    }

    #[tokio::test]
    async fn test_disabled_mode() {
        let config = CaptureConfig {
            mode: RecordingMode::Disabled,
            ..Default::default()
        };

        let capture = ReplayCapture::with_config("disabled_test", config);

        assert!(!capture.is_enabled());

        capture.start_session().await;

        let record = capture.end_session().await;
        assert_eq!(record.events.len(), 0);
    }

    #[tokio::test]
    async fn test_hashes_only_mode() {
        let config = CaptureConfig {
            mode: RecordingMode::HashesOnly,
            ..Default::default()
        };

        let capture = ReplayCapture::with_config("hashes_test", config);

        capture.start_session().await;

        let provenance = ToolProvenance::new("tool", "h1");
        let envelope = ToolResultEnvelope::success(serde_json::json!({"data": "sensitive"}), provenance);

        capture
            .record_tool_call(
                "tool",
                serde_json::json!({"secret": "value"}),
                &envelope,
            )
            .await;

        let record = capture.end_session().await;

        // Find the tool call event
        let tool_call = record.tool_calls().next().unwrap();

        // Args and result should be null in hashes-only mode
        assert_eq!(tool_call.args, Value::Null);
        assert!(tool_call.result.is_none());

        // But hash should still be present
        assert!(!tool_call.args_hash.is_empty());
    }

    #[tokio::test]
    async fn test_memory_retrieval_recording() {
        let capture = ReplayCapture::new("memory_test");

        capture.start_session().await;

        capture
            .record_memory_retrieval(
                "find important things",
                vec!["mem_1".to_string(), "mem_2".to_string()],
                50,
                Some("trace_1".to_string()),
            )
            .await;

        let record = capture.end_session().await;

        let retrievals: Vec<_> = record.memory_retrievals().collect();
        assert_eq!(retrievals.len(), 1);
        assert_eq!(retrievals[0].result_count, 2);
        assert_eq!(retrievals[0].latency_ms, 50);
    }

    #[tokio::test]
    async fn test_versioning_recording() {
        let capture = ReplayCapture::new("versioning_test");

        capture.start_session().await;

        capture
            .record_versioning(
                VersioningOperation::Commit,
                Some("main".to_string()),
                Some("abc123".to_string()),
                None,
                true,
                None,
                None,
            )
            .await;

        let record = capture.end_session().await;

        let versioning_events: Vec<_> = record
            .events_of_type(|e| matches!(e, ReplayEvent::MemoryVersioning(_)))
            .collect();

        assert_eq!(versioning_events.len(), 1);
    }
}



