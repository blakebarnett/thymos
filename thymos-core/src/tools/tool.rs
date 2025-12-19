//! Tool trait and metadata definitions
//!
//! Tools are the primary way agents interact with the world.
//! Each tool declares its capabilities, parameters, and execution logic.

use super::capability::{CapabilitySet};
use super::result::{ToolError, ToolProvenance, ToolResultEnvelope};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Tool metadata for LLM-friendly discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Tool name (unique identifier)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// When should the LLM use this tool?
    pub usage_hints: Vec<String>,

    /// What the tool returns
    pub returns: String,

    /// What to do if the tool fails
    pub error_guidance: Option<String>,

    /// Tool version
    pub version: Option<String>,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ToolMetadata {
    /// Create new metadata with required fields
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            usage_hints: Vec::new(),
            returns: "Tool-specific result".to_string(),
            error_guidance: None,
            version: None,
            tags: Vec::new(),
        }
    }

    /// Add a usage hint
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.usage_hints.push(hint.into());
        self
    }

    /// Set return description
    pub fn with_returns(mut self, returns: impl Into<String>) -> Self {
        self.returns = returns.into();
        self
    }

    /// Set error guidance
    pub fn with_error_guidance(mut self, guidance: impl Into<String>) -> Self {
        self.error_guidance = Some(guidance.into());
        self
    }

    /// Set version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// JSON Schema for tool parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// JSON Schema for input parameters
    pub parameters: Value,

    /// Whether strict validation is required
    pub strict: bool,
}

impl ToolSchema {
    /// Create a schema from a JSON Schema value
    pub fn new(parameters: Value) -> Self {
        Self {
            parameters,
            strict: true,
        }
    }

    /// Create an empty schema (tool takes no parameters)
    pub fn empty() -> Self {
        Self {
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            strict: true,
        }
    }

    /// Set strict mode
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}

/// Example tool invocation for few-shot learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    /// Example name/description
    pub name: String,

    /// Input arguments
    pub input: Value,

    /// Expected output (or description)
    pub output: Value,
}

impl ToolExample {
    /// Create a new example
    pub fn new(name: impl Into<String>, input: Value, output: Value) -> Self {
        Self {
            name: name.into(),
            input,
            output,
        }
    }
}

/// Context provided to tool execution
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    /// Agent ID making the call
    pub agent_id: Option<String>,

    /// Trace ID for correlation
    pub trace_id: Option<String>,

    /// Whether to redact secrets from output
    pub redact_secrets: bool,

    /// Additional context values
    pub extra: Value,
}

impl Default for ToolExecutionContext {
    fn default() -> Self {
        Self {
            agent_id: None,
            trace_id: None,
            redact_secrets: true,
            extra: Value::Null,
        }
    }
}

impl ToolExecutionContext {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
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

    /// Set redaction policy
    pub fn with_redact_secrets(mut self, redact: bool) -> Self {
        self.redact_secrets = redact;
        self
    }

    /// Add extra context
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }
}

/// Core tool trait
///
/// Implement this trait to create a tool that agents can use.
/// The runtime will enforce capability policies before calling `execute`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool metadata
    fn metadata(&self) -> &ToolMetadata;

    /// Get tool name (convenience method)
    fn name(&self) -> &str {
        &self.metadata().name
    }

    /// Get tool description (convenience method)
    fn description(&self) -> &str {
        &self.metadata().description
    }

    /// Get the JSON schema for this tool's parameters
    fn schema(&self) -> ToolSchema;

    /// Get examples for few-shot learning
    fn examples(&self) -> Vec<ToolExample> {
        Vec::new()
    }

    /// Get required capabilities
    ///
    /// The runtime will check these against the policy before execution.
    /// Override this if your tool needs privileged capabilities.
    fn required_capabilities(&self) -> CapabilitySet {
        CapabilitySet::new()
    }

    /// Validate input arguments before execution
    ///
    /// Called before `execute`. Return validation errors if args are invalid.
    /// Default implementation performs no validation.
    fn validate(&self, _args: &Value) -> Result<(), Vec<crate::tools::ValidationError>> {
        Ok(())
    }

    /// Execute the tool with given arguments
    ///
    /// This is called only after:
    /// 1. Capability policy check passes
    /// 2. Validation passes
    ///
    /// Return a ToolResultEnvelope with success, error, or cancelled status.
    async fn execute(
        &self,
        args: Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError>;
}

/// Handler trait for simpler tool implementations
///
/// Use this when you don't need full control over the result envelope.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Execute and return a simple result
    async fn handle(&self, args: Value, ctx: &ToolExecutionContext) -> Result<Value, ToolError>;
}

/// Wrapper to convert a ToolHandler into a full Tool
#[allow(dead_code)]
pub struct HandlerTool<H: ToolHandler> {
    metadata: ToolMetadata,
    schema: ToolSchema,
    capabilities: CapabilitySet,
    examples: Vec<ToolExample>,
    handler: H,
}

#[allow(dead_code)]
impl<H: ToolHandler> HandlerTool<H> {
    /// Create a new handler tool
    pub fn new(metadata: ToolMetadata, schema: ToolSchema, handler: H) -> Self {
        Self {
            metadata,
            schema,
            capabilities: CapabilitySet::new(),
            examples: Vec::new(),
            handler,
        }
    }

    /// Set required capabilities
    pub fn with_capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = caps;
        self
    }

    /// Add examples
    pub fn with_examples(mut self, examples: Vec<ToolExample>) -> Self {
        self.examples = examples;
        self
    }
}

#[async_trait]
impl<H: ToolHandler + 'static> Tool for HandlerTool<H> {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    fn examples(&self) -> Vec<ToolExample> {
        self.examples.clone()
    }

    fn required_capabilities(&self) -> CapabilitySet {
        self.capabilities.clone()
    }

    async fn execute(
        &self,
        args: Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        use sha2::{Digest, Sha256};

        let started_at = chrono::Utc::now();

        // Compute args hash
        let args_json = serde_json::to_string(&args).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(args_json.as_bytes());
        let args_hash = format!("{:x}", hasher.finalize());

        let result = self.handler.handle(args, ctx).await;

        let duration = (chrono::Utc::now() - started_at)
            .to_std()
            .unwrap_or_default();

        let mut provenance = ToolProvenance::new(&self.metadata.name, &args_hash[..16])
            .with_duration(duration);

        if let Some(ref agent_id) = ctx.agent_id {
            provenance = provenance.with_agent_id(agent_id);
        }
        if let Some(ref trace_id) = ctx.trace_id {
            provenance = provenance.with_trace_id(trace_id);
        }

        match result {
            Ok(value) => Ok(ToolResultEnvelope::success(value, provenance)),
            Err(error) => Ok(ToolResultEnvelope::error(error, provenance)),
        }
    }
}

/// Type alias for boxed tools
pub type BoxedTool = Arc<dyn Tool>;

#[cfg(test)]
mod tool_tests {
    use super::*;

    struct EchoTool {
        metadata: ToolMetadata,
    }

    impl EchoTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("echo", "Echoes input back")
                    .with_hint("Use when you need to test tool execution")
                    .with_returns("The same value passed as input"),
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
            ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, ToolError> {
            let provenance = crate::tools::ToolProvenance::new("echo", "test")
                .with_agent_id(ctx.agent_id.clone().unwrap_or_default());

            let message = args.get("message").cloned().unwrap_or(Value::Null);
            Ok(ToolResultEnvelope::success(message, provenance))
        }
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let tool = EchoTool::new();
        let ctx = ToolExecutionContext::new().with_agent_id("test_agent");
        let args = serde_json::json!({ "message": "hello" });

        let result = tool.execute(args, &ctx).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.value().unwrap(), &serde_json::json!("hello"));
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = ToolMetadata::new("my_tool", "Does something useful")
            .with_hint("Use for X")
            .with_hint("Don't use for Y")
            .with_returns("A JSON object")
            .with_error_guidance("If it fails, try again with different params")
            .with_version("1.0.0")
            .with_tag("utility");

        assert_eq!(metadata.name, "my_tool");
        assert_eq!(metadata.usage_hints.len(), 2);
        assert!(metadata.version.is_some());
    }
}

