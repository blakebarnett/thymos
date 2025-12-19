//! Tool stubbing for deterministic evaluation
//!
//! Provides stub implementations of tools that return predetermined responses,
//! enabling offline, deterministic testing of workflows.

use crate::tools::{
    CapabilitySet, Tool, ToolError, ToolErrorKind, ToolExecutionContext, ToolMetadata,
    ToolProvenance, ToolResultEnvelope, ToolSchema,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::fixture::{StubResponseDef, StubResponseValue};

/// Predetermined response for a stub tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubResponse {
    /// Value to return on success
    pub value: Option<Value>,

    /// Error to return on failure
    pub error: Option<StubError>,

    /// Simulated delay in milliseconds
    pub delay_ms: u64,

    /// Warnings to include
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl StubResponse {
    /// Create a successful response
    pub fn success(value: Value) -> Self {
        Self {
            value: Some(value),
            error: None,
            delay_ms: 0,
            warnings: Vec::new(),
        }
    }

    /// Create an error response
    pub fn error(kind: ToolErrorKind, message: impl Into<String>) -> Self {
        Self {
            value: None,
            error: Some(StubError {
                kind,
                message: message.into(),
            }),
            delay_ms: 0,
            warnings: Vec::new(),
        }
    }

    /// Add a simulated delay
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Stub error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubError {
    pub kind: ToolErrorKind,
    pub message: String,
}

/// A stub tool that returns predetermined responses
pub struct StubTool {
    metadata: ToolMetadata,
    schema: ToolSchema,
    responses: Arc<RwLock<Vec<StubResponse>>>,
    call_count: AtomicUsize,
    call_history: Arc<RwLock<Vec<Value>>>,
}

impl StubTool {
    /// Create a new stub tool with a single response
    pub fn new(name: impl Into<String>, response: StubResponse) -> Self {
        let name = name.into();
        Self {
            metadata: ToolMetadata::new(&name, format!("Stub tool: {}", name)),
            schema: ToolSchema::empty(),
            responses: Arc::new(RwLock::new(vec![response])),
            call_count: AtomicUsize::new(0),
            call_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a stub tool with multiple responses (returned in order)
    pub fn with_responses(name: impl Into<String>, responses: Vec<StubResponse>) -> Self {
        let name = name.into();
        Self {
            metadata: ToolMetadata::new(&name, format!("Stub tool: {}", name)),
            schema: ToolSchema::empty(),
            responses: Arc::new(RwLock::new(responses)),
            call_count: AtomicUsize::new(0),
            call_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create from fixture stub response definitions
    pub fn from_fixture_stubs(
        name: impl Into<String>,
        stub_defs: &[StubResponseDef],
    ) -> Self {
        let responses: Vec<StubResponse> = stub_defs
            .iter()
            .map(|def| {
                let mut response = match &def.response {
                    StubResponseValue::Success(value) => StubResponse::success(value.clone()),
                    StubResponseValue::Error { error_kind, message } => {
                        let kind = match error_kind.as_str() {
                            "timeout" => ToolErrorKind::Timeout,
                            "validation" => ToolErrorKind::Validation,
                            "not_found" => ToolErrorKind::NotFound,
                            "transient" => ToolErrorKind::Transient,
                            "rate_limited" => ToolErrorKind::RateLimited,
                            "cancelled" => ToolErrorKind::Cancelled,
                            _ => ToolErrorKind::Internal,
                        };
                        StubResponse::error(kind, message)
                    }
                };
                response.delay_ms = def.delay_ms;
                response
            })
            .collect();

        Self::with_responses(name, responses)
    }

    /// Get the number of times this tool has been called
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Get the call history (args passed to each call)
    pub async fn call_history(&self) -> Vec<Value> {
        self.call_history.read().await.clone()
    }

    /// Reset call count and history
    pub async fn reset(&self) {
        self.call_count.store(0, Ordering::SeqCst);
        self.call_history.write().await.clear();
    }

    /// Set a custom schema
    pub fn with_schema(mut self, schema: ToolSchema) -> Self {
        self.schema = schema;
        self
    }

    /// Set custom metadata
    pub fn with_metadata(mut self, metadata: ToolMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[async_trait]
impl Tool for StubTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    fn required_capabilities(&self) -> CapabilitySet {
        // Stubs don't require any real capabilities
        CapabilitySet::new()
    }

    async fn execute(
        &self,
        args: Value,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        // Record the call
        let call_num = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.call_history.write().await.push(args.clone());

        // Get the response for this call
        let responses = self.responses.read().await;
        let response = if call_num < responses.len() {
            responses[call_num].clone()
        } else if !responses.is_empty() {
            // Repeat last response if we've run out
            responses.last().unwrap().clone()
        } else {
            // No responses configured - return empty success
            StubResponse::success(Value::Null)
        };
        drop(responses);

        // Simulate delay if configured
        if response.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(response.delay_ms)).await;
        }

        let provenance = ToolProvenance::new(&self.metadata.name, format!("stub_call_{}", call_num))
            .with_duration(std::time::Duration::from_millis(response.delay_ms));

        if let Some(ref error) = response.error {
            return Ok(ToolResultEnvelope::error(
                ToolError::new(error.kind, &error.message),
                provenance,
            ));
        }

        let mut envelope = ToolResultEnvelope::success(
            response.value.unwrap_or(Value::Null),
            provenance,
        );

        for warning in &response.warnings {
            envelope = envelope.with_warning(crate::tools::ToolWarning::new("STUB_WARNING", warning));
        }

        Ok(envelope)
    }
}

/// Registry of stub tools for evaluation
pub struct StubRegistry {
    stubs: HashMap<String, Arc<StubTool>>,
}

impl StubRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            stubs: HashMap::new(),
        }
    }

    /// Register a stub tool
    pub fn register(&mut self, stub: StubTool) {
        let name = stub.metadata().name.clone();
        self.stubs.insert(name, Arc::new(stub));
    }

    /// Get a stub tool by name
    pub fn get(&self, name: &str) -> Option<Arc<StubTool>> {
        self.stubs.get(name).cloned()
    }

    /// Get a tool reference by name (for use with ToolRuntime)
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.stubs.get(name).map(|s| s.clone() as Arc<dyn Tool>)
    }

    /// Check if a tool is registered
    pub fn contains(&self, name: &str) -> bool {
        self.stubs.contains_key(name)
    }

    /// Get all registered tool names
    pub fn tool_names(&self) -> Vec<&String> {
        self.stubs.keys().collect()
    }

    /// Reset all stubs (clear call counts and histories)
    pub async fn reset_all(&self) {
        for stub in self.stubs.values() {
            stub.reset().await;
        }
    }

    /// Create from fixture tool stubs
    pub fn from_fixture_stubs(
        stubs: &HashMap<String, Vec<StubResponseDef>>,
    ) -> Self {
        let mut registry = Self::new();
        for (name, responses) in stubs {
            registry.register(StubTool::from_fixture_stubs(name, responses));
        }
        registry
    }
}

impl Default for StubRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod stub_tests {
    use super::*;

    #[tokio::test]
    async fn test_stub_tool_success() {
        let stub = StubTool::new("test", StubResponse::success(serde_json::json!({"result": 42})));
        let ctx = ToolExecutionContext::default();

        let result = stub.execute(serde_json::json!({"input": "test"}), &ctx).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.value().unwrap(), &serde_json::json!({"result": 42}));
        assert_eq!(stub.call_count(), 1);
    }

    #[tokio::test]
    async fn test_stub_tool_error() {
        let stub = StubTool::new(
            "failing_tool",
            StubResponse::error(ToolErrorKind::Timeout, "Simulated timeout"),
        );
        let ctx = ToolExecutionContext::default();

        let result = stub.execute(Value::Null, &ctx).await.unwrap();

        assert!(result.is_error());
        let error = result.get_error().unwrap();
        assert_eq!(error.kind, ToolErrorKind::Timeout);
    }

    #[tokio::test]
    async fn test_stub_tool_sequence() {
        let stub = StubTool::with_responses(
            "sequence_tool",
            vec![
                StubResponse::success(serde_json::json!(1)),
                StubResponse::success(serde_json::json!(2)),
                StubResponse::success(serde_json::json!(3)),
            ],
        );
        let ctx = ToolExecutionContext::default();

        let r1 = stub.execute(Value::Null, &ctx).await.unwrap();
        let r2 = stub.execute(Value::Null, &ctx).await.unwrap();
        let r3 = stub.execute(Value::Null, &ctx).await.unwrap();
        let r4 = stub.execute(Value::Null, &ctx).await.unwrap(); // Should repeat last

        assert_eq!(r1.value().unwrap(), &serde_json::json!(1));
        assert_eq!(r2.value().unwrap(), &serde_json::json!(2));
        assert_eq!(r3.value().unwrap(), &serde_json::json!(3));
        assert_eq!(r4.value().unwrap(), &serde_json::json!(3)); // Repeats
        assert_eq!(stub.call_count(), 4);
    }

    #[tokio::test]
    async fn test_stub_tool_call_history() {
        let stub = StubTool::new("history_tool", StubResponse::success(Value::Null));
        let ctx = ToolExecutionContext::default();

        stub.execute(serde_json::json!({"call": 1}), &ctx).await.unwrap();
        stub.execute(serde_json::json!({"call": 2}), &ctx).await.unwrap();

        let history = stub.call_history().await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], serde_json::json!({"call": 1}));
        assert_eq!(history[1], serde_json::json!({"call": 2}));
    }

    #[tokio::test]
    async fn test_stub_registry() {
        let mut registry = StubRegistry::new();

        registry.register(StubTool::new("tool1", StubResponse::success(serde_json::json!("result1"))));
        registry.register(StubTool::new("tool2", StubResponse::success(serde_json::json!("result2"))));

        assert!(registry.contains("tool1"));
        assert!(registry.contains("tool2"));
        assert!(!registry.contains("tool3"));

        let tool1 = registry.get("tool1").unwrap();
        let ctx = ToolExecutionContext::default();
        let result = tool1.execute(Value::Null, &ctx).await.unwrap();
        assert_eq!(result.value().unwrap(), &serde_json::json!("result1"));
    }

    #[tokio::test]
    async fn test_stub_with_delay() {
        let stub = StubTool::new(
            "slow_tool",
            StubResponse::success(Value::Null).with_delay(50),
        );
        let ctx = ToolExecutionContext::default();

        let start = std::time::Instant::now();
        stub.execute(Value::Null, &ctx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() >= 50);
    }
}



