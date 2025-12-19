//! Integration tests for the tools module

use super::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

/// A simple echo tool for testing
struct EchoTool {
    metadata: ToolMetadata,
}

impl EchoTool {
    fn new() -> Self {
        Self {
            metadata: ToolMetadata::new("echo", "Echoes input back")
                .with_hint("Use to test tool execution")
                .with_returns("The input message"),
        }
    }
}

#[async_trait]
impl Tool for EchoTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        }))
    }

    fn validate(&self, args: &Value) -> Result<(), Vec<ValidationError>> {
        if args.get("message").is_none() {
            return Err(vec![ValidationError::new("message", "message is required")]);
        }
        Ok(())
    }

    async fn execute(
        &self,
        args: Value,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        let provenance = ToolProvenance::new("echo", "test");
        let message = args.get("message").cloned().unwrap_or(Value::Null);
        Ok(ToolResultEnvelope::success(message, provenance))
    }
}

/// A tool that requires filesystem access
struct FileTool {
    metadata: ToolMetadata,
}

impl FileTool {
    fn new() -> Self {
        Self {
            metadata: ToolMetadata::new("file_read", "Reads a file"),
        }
    }
}

#[async_trait]
impl Tool for FileTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }))
    }

    fn required_capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_capabilities([Capability::FilesystemRead])
    }

    async fn execute(
        &self,
        args: Value,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        let provenance = ToolProvenance::new("file_read", "test");
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Ok(ToolResultEnvelope::success(
            json!({"path": path, "content": "file contents"}),
            provenance,
        ))
    }
}

#[tokio::test]
async fn test_echo_tool_success() {
    let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
    let tool = EchoTool::new();
    let ctx = ToolContext::new().with_agent_id("test");

    let result = runtime
        .execute(&tool, json!({"message": "hello"}), &ctx)
        .await;

    assert!(result.is_success());
    assert_eq!(result.value().unwrap(), &json!("hello"));
}

#[tokio::test]
async fn test_validation_error() {
    let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
    let tool = EchoTool::new();
    let ctx = ToolContext::new();

    let result = runtime.execute(&tool, json!({}), &ctx).await;

    assert!(result.is_error());
    let error = result.get_error().unwrap();
    assert_eq!(error.kind, ToolErrorKind::Validation);
}

#[tokio::test]
async fn test_capability_denied() {
    // Deny all by default
    let runtime = ToolRuntime::new(CapabilityPolicy::deny_all());
    let tool = FileTool::new();
    let ctx = ToolContext::new();

    let result = runtime
        .execute(&tool, json!({"path": "/etc/passwd"}), &ctx)
        .await;

    assert!(result.is_error());
    let error = result.get_error().unwrap();
    assert_eq!(error.kind, ToolErrorKind::CapabilityDenied);
}

#[tokio::test]
async fn test_capability_allowed() {
    // Allow filesystem read
    let policy = CapabilityPolicy::deny_all().allow(Capability::FilesystemRead);
    let runtime = ToolRuntime::new(policy);
    let tool = FileTool::new();
    let ctx = ToolContext::new();

    let result = runtime
        .execute(&tool, json!({"path": "/etc/passwd"}), &ctx)
        .await;

    assert!(result.is_success());
}

#[tokio::test]
async fn test_safe_only_policy() {
    let runtime = ToolRuntime::new(CapabilityPolicy::safe_only());

    // Echo tool (no special capabilities) should work
    let echo = EchoTool::new();
    let ctx = ToolContext::new();
    let result = runtime
        .execute(&echo, json!({"message": "hi"}), &ctx)
        .await;
    assert!(result.is_success());

    // File tool requires filesystem read (safe capability) - should work
    let file = FileTool::new();
    let result = runtime
        .execute(&file, json!({"path": "/test"}), &ctx)
        .await;
    assert!(result.is_success());
}

#[tokio::test]
async fn test_result_envelope_serialization() {
    let provenance = ToolProvenance::new("test", "abc123")
        .with_duration(Duration::from_millis(42))
        .with_agent_id("agent_1")
        .with_policy_decision(PolicyDecision::new("cap_check", true));

    let envelope = ToolResultEnvelope::success(json!({"result": 42}), provenance)
        .with_warning(ToolWarning::new("DEPRECATION", "This tool is deprecated"));

    let json_str = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed: ToolResultEnvelope = serde_json::from_str(&json_str).unwrap();

    assert!(parsed.is_success());
    assert_eq!(parsed.warnings.len(), 1);
    assert_eq!(parsed.provenance.tool_name, "test");
    assert_eq!(parsed.provenance.duration.as_millis(), 42);
}

#[tokio::test]
async fn test_error_taxonomy() {
    // Test that different error kinds have correct retryability
    let retryable_kinds = [
        ToolErrorKind::Timeout,
        ToolErrorKind::RateLimited,
        ToolErrorKind::Transient,
    ];

    let fatal_kinds = [
        ToolErrorKind::Validation,
        ToolErrorKind::CapabilityDenied,
        ToolErrorKind::NotFound,
        ToolErrorKind::Internal,
    ];

    for kind in retryable_kinds {
        assert!(kind.is_retryable(), "{:?} should be retryable", kind);
        assert!(!kind.is_fatal(), "{:?} should not be fatal", kind);
    }

    for kind in fatal_kinds {
        assert!(kind.is_fatal(), "{:?} should be fatal", kind);
        assert!(!kind.is_retryable(), "{:?} should not be retryable", kind);
    }
}

#[tokio::test]
async fn test_provenance_in_result() {
    let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
    let tool = EchoTool::new();
    let ctx = ToolContext::new()
        .with_agent_id("my_agent")
        .with_trace_id("trace_abc");

    let result = runtime
        .execute(&tool, json!({"message": "test"}), &ctx)
        .await;

    assert!(result.is_success());
    assert_eq!(result.provenance.agent_id, Some("my_agent".to_string()));
    assert_eq!(result.provenance.trace_id, Some("trace_abc".to_string()));
    assert!(!result.provenance.args_hash.is_empty());
    assert!(result.provenance.duration >= Duration::ZERO);
}

#[tokio::test]
async fn test_handler_tool_wrapper() {
    use crate::tools::tool::{HandlerTool, ToolHandler};

    struct SimpleHandler;

    #[async_trait]
    impl ToolHandler for SimpleHandler {
        async fn handle(
            &self,
            args: Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<Value, ToolError> {
            Ok(json!({
                "received": args,
                "processed": true
            }))
        }
    }

    let tool = HandlerTool::new(
        ToolMetadata::new("simple", "A simple handler tool"),
        ToolSchema::empty(),
        SimpleHandler,
    );

    let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
    let ctx = ToolContext::new();

    let result = runtime.execute(&tool, json!({"input": "data"}), &ctx).await;

    assert!(result.is_success());
    let value = result.value().unwrap();
    assert_eq!(value.get("processed"), Some(&json!(true)));
}

