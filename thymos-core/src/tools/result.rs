//! Structured tool result envelope and error model
//!
//! Provides a stable, versionable result format for tool execution that includes:
//! - Success/error/cancelled status
//! - Warnings that don't fail execution
//! - Provenance metadata for tracing and replay
//! - Unified error taxonomy with retryable vs fatal distinction

use crate::tools::Capability;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Current schema version for result envelopes
pub const RESULT_SCHEMA_VERSION: u32 = 1;

/// Structured result envelope for tool execution
///
/// This envelope wraps all tool outputs with provenance metadata,
/// enabling tracing, replay, and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEnvelope {
    /// Schema version for forward compatibility
    pub schema_version: u32,

    /// Result status and payload
    pub result: ToolResult,

    /// Warnings that occurred but didn't fail execution
    pub warnings: Vec<ToolWarning>,

    /// Provenance metadata for tracing/replay
    pub provenance: ToolProvenance,
}

impl ToolResultEnvelope {
    /// Create a successful result envelope
    pub fn success(value: Value, provenance: ToolProvenance) -> Self {
        Self {
            schema_version: RESULT_SCHEMA_VERSION,
            result: ToolResult::Success { value },
            warnings: Vec::new(),
            provenance,
        }
    }

    /// Create an error result envelope
    pub fn error(error: ToolError, provenance: ToolProvenance) -> Self {
        Self {
            schema_version: RESULT_SCHEMA_VERSION,
            result: ToolResult::Error { error },
            warnings: Vec::new(),
            provenance,
        }
    }

    /// Create a cancelled result envelope
    pub fn cancelled(reason: String, provenance: ToolProvenance) -> Self {
        Self {
            schema_version: RESULT_SCHEMA_VERSION,
            result: ToolResult::Cancelled { reason },
            warnings: Vec::new(),
            provenance,
        }
    }

    /// Add a warning to the envelope
    pub fn with_warning(mut self, warning: ToolWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add multiple warnings
    pub fn with_warnings(mut self, warnings: impl IntoIterator<Item = ToolWarning>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    /// Check if this result represents success
    pub fn is_success(&self) -> bool {
        matches!(self.result, ToolResult::Success { .. })
    }

    /// Check if this result represents an error
    pub fn is_error(&self) -> bool {
        matches!(self.result, ToolResult::Error { .. })
    }

    /// Check if this result represents cancellation
    pub fn is_cancelled(&self) -> bool {
        matches!(self.result, ToolResult::Cancelled { .. })
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match &self.result {
            ToolResult::Error { error } => error.kind.is_retryable(),
            _ => false,
        }
    }

    /// Get the value if successful
    pub fn value(&self) -> Option<&Value> {
        match &self.result {
            ToolResult::Success { value } => Some(value),
            _ => None,
        }
    }

    /// Get the error if failed
    pub fn get_error(&self) -> Option<&ToolError> {
        match &self.result {
            ToolResult::Error { error } => Some(error),
            _ => None,
        }
    }
}

/// Tool execution result (success, error, or cancelled)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolResult {
    /// Tool executed successfully
    Success {
        /// The result value
        value: Value,
    },

    /// Tool execution failed
    Error {
        /// Structured error information
        error: ToolError,
    },

    /// Tool execution was cancelled
    Cancelled {
        /// Reason for cancellation
        reason: String,
    },
}

/// Structured tool error with taxonomy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    /// Error kind (determines retryability)
    pub kind: ToolErrorKind,

    /// Human-readable error message
    pub message: String,

    /// Underlying error code (if applicable)
    pub code: Option<String>,

    /// Additional context
    pub context: Option<Value>,

    /// Suggested retry delay (for retryable errors)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_duration_millis"
    )]
    pub retry_after: Option<Duration>,
}

impl ToolError {
    /// Create a new tool error
    pub fn new(kind: ToolErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            code: None,
            context: None,
            retry_after: None,
        }
    }

    /// Add an error code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Add context
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Add retry delay
    pub fn with_retry_after(mut self, delay: Duration) -> Self {
        self.retry_after = Some(delay);
        self
    }

    /// Create a validation error
    pub fn validation(errors: Vec<ValidationError>) -> Self {
        Self {
            kind: ToolErrorKind::Validation,
            message: format!(
                "Validation failed: {}",
                errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            code: Some("VALIDATION_FAILED".to_string()),
            context: Some(serde_json::to_value(&errors).unwrap_or_default()),
            retry_after: None,
        }
    }

    /// Create a capability denied error
    pub fn capability_denied(denied: &[Capability]) -> Self {
        Self {
            kind: ToolErrorKind::CapabilityDenied,
            message: format!(
                "Required capabilities denied: {:?}",
                denied
            ),
            code: Some("CAPABILITY_DENIED".to_string()),
            context: Some(serde_json::to_value(denied).unwrap_or_default()),
            retry_after: None,
        }
    }

    /// Create a timeout error
    pub fn timeout(duration: Duration) -> Self {
        Self {
            kind: ToolErrorKind::Timeout,
            message: format!("Tool execution timed out after {:?}", duration),
            code: Some("TIMEOUT".to_string()),
            context: None,
            retry_after: Some(Duration::from_secs(1)),
        }
    }

    /// Create a rate limit error
    pub fn rate_limited(retry_after: Duration) -> Self {
        Self {
            kind: ToolErrorKind::RateLimited,
            message: "Rate limit exceeded".to_string(),
            code: Some("RATE_LIMITED".to_string()),
            context: None,
            retry_after: Some(retry_after),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ToolErrorKind::Internal, message)
            .with_code("INTERNAL_ERROR")
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

impl std::error::Error for ToolError {}

/// Error kind taxonomy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorKind {
    /// Input validation failed (not retryable - fix args)
    Validation,

    /// Required capability was denied (not retryable - policy)
    CapabilityDenied,

    /// Execution timed out (retryable)
    Timeout,

    /// Rate limit exceeded (retryable after delay)
    RateLimited,

    /// Transient network/service error (retryable)
    Transient,

    /// Resource not found (not retryable)
    NotFound,

    /// Permission denied by external system (not retryable)
    PermissionDenied,

    /// Invalid response from external system (may be retryable)
    InvalidResponse,

    /// Internal tool error (not retryable - bug)
    Internal,

    /// Cancelled by user/system (not retryable)
    Cancelled,
}

impl ToolErrorKind {
    /// Check if this error kind is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ToolErrorKind::Timeout
                | ToolErrorKind::RateLimited
                | ToolErrorKind::Transient
                | ToolErrorKind::InvalidResponse
        )
    }

    /// Check if this error kind is fatal (never retry)
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ToolErrorKind::Validation
                | ToolErrorKind::CapabilityDenied
                | ToolErrorKind::NotFound
                | ToolErrorKind::PermissionDenied
                | ToolErrorKind::Internal
                | ToolErrorKind::Cancelled
        )
    }
}

/// Validation error for a specific field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Field path (e.g., "args.url")
    pub field: String,

    /// Error message
    pub message: String,

    /// Error code
    pub code: Option<String>,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: None,
        }
    }

    /// Add an error code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref code) = self.code {
            write!(f, "[{}] {}: {}", code, self.field, self.message)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}

/// Warning that occurred during execution but didn't cause failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolWarning {
    /// Warning code
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Additional context
    pub context: Option<Value>,
}

impl ToolWarning {
    /// Create a new warning
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: None,
        }
    }

    /// Add context
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }
}

/// Provenance metadata for tracing and replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProvenance {
    /// Tool name
    pub tool_name: String,

    /// Tool version (if available)
    pub tool_version: Option<String>,

    /// Hash of input arguments (for replay matching)
    pub args_hash: String,

    /// Timestamp when execution started
    pub started_at: DateTime<Utc>,

    /// Execution duration
    #[serde(with = "duration_millis")]
    pub duration: Duration,

    /// Agent ID that invoked the tool
    pub agent_id: Option<String>,

    /// Request/trace ID for correlation
    pub trace_id: Option<String>,

    /// Policy decisions that were applied
    pub policy_decisions: Vec<PolicyDecision>,
}

impl ToolProvenance {
    /// Create new provenance
    pub fn new(tool_name: impl Into<String>, args_hash: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_version: None,
            args_hash: args_hash.into(),
            started_at: Utc::now(),
            duration: Duration::ZERO,
            agent_id: None,
            trace_id: None,
            policy_decisions: Vec::new(),
        }
    }

    /// Set duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Set agent ID
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set trace ID
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Add a policy decision
    pub fn with_policy_decision(mut self, decision: PolicyDecision) -> Self {
        self.policy_decisions.push(decision);
        self
    }
}

/// Record of a policy decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    /// What was checked
    pub check: String,

    /// Whether it was allowed
    pub allowed: bool,

    /// Reason for decision
    pub reason: Option<String>,
}

impl PolicyDecision {
    /// Create a new policy decision
    pub fn new(check: impl Into<String>, allowed: bool) -> Self {
        Self {
            check: check.into(),
            allowed,
            reason: None,
        }
    }

    /// Add a reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

// Serde helpers for Duration serialization as milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

mod option_duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => serializer.serialize_some(&(d.as_millis() as u64)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<u64> = Option::deserialize(deserializer)?;
        Ok(opt.map(Duration::from_millis))
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;

    #[test]
    fn test_success_envelope() {
        let provenance = ToolProvenance::new("test_tool", "abc123");
        let envelope = ToolResultEnvelope::success(serde_json::json!({"key": "value"}), provenance);

        assert!(envelope.is_success());
        assert!(!envelope.is_error());
        assert!(!envelope.is_cancelled());
        assert!(envelope.value().is_some());
    }

    #[test]
    fn test_error_envelope() {
        let provenance = ToolProvenance::new("test_tool", "abc123");
        let error = ToolError::new(ToolErrorKind::Validation, "Invalid input");
        let envelope = ToolResultEnvelope::error(error, provenance);

        assert!(!envelope.is_success());
        assert!(envelope.is_error());
        assert!(!envelope.is_retryable());
    }

    #[test]
    fn test_retryable_error() {
        let provenance = ToolProvenance::new("test_tool", "abc123");
        let error = ToolError::timeout(Duration::from_secs(30));
        let envelope = ToolResultEnvelope::error(error, provenance);

        assert!(envelope.is_error());
        assert!(envelope.is_retryable());
    }

    #[test]
    fn test_error_kind_retryability() {
        assert!(ToolErrorKind::Timeout.is_retryable());
        assert!(ToolErrorKind::RateLimited.is_retryable());
        assert!(ToolErrorKind::Transient.is_retryable());

        assert!(ToolErrorKind::Validation.is_fatal());
        assert!(ToolErrorKind::CapabilityDenied.is_fatal());
        assert!(ToolErrorKind::Internal.is_fatal());
    }

    #[test]
    fn test_envelope_serialization() {
        let provenance = ToolProvenance::new("test_tool", "abc123")
            .with_duration(Duration::from_millis(150))
            .with_agent_id("agent_1");

        let envelope = ToolResultEnvelope::success(serde_json::json!({"result": 42}), provenance)
            .with_warning(ToolWarning::new("DEPRECATED", "This tool is deprecated"));

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ToolResultEnvelope = serde_json::from_str(&json).unwrap();

        assert!(parsed.is_success());
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.provenance.tool_name, "test_tool");
    }

    #[test]
    fn test_validation_error() {
        let errors = vec![
            ValidationError::new("url", "URL is required"),
            ValidationError::new("timeout", "Must be positive"),
        ];
        let error = ToolError::validation(errors);

        assert_eq!(error.kind, ToolErrorKind::Validation);
        assert!(error.message.contains("Validation failed"));
        assert!(error.context.is_some());
    }
}

