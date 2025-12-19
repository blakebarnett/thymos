//! Bridges Thymos Tool trait to AutoAgents ToolT trait
//!
//! This adapter allows Thymos tools with their rich capabilities (security,
//! provenance, validation) to be used within AutoAgents execution contexts.

use crate::tools::{Tool, ToolExecutionContext, ToolResult, ToolResultEnvelope};
use async_trait::async_trait;
use autoagents_core::tool::{ToolCallError, ToolRuntime, ToolT};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

/// Global cache for static strings used by tool adapters.
/// This avoids memory leaks from Box::leak while providing 'static lifetimes.
static STRING_CACHE: OnceLock<StringCache> = OnceLock::new();

/// Thread-safe cache for storing tool metadata strings with 'static lifetime.
struct StringCache {
    names: std::sync::RwLock<HashMap<String, &'static str>>,
    descriptions: std::sync::RwLock<HashMap<String, &'static str>>,
}

impl StringCache {
    fn new() -> Self {
        Self {
            names: std::sync::RwLock::new(HashMap::new()),
            descriptions: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn get_or_insert_name(&self, key: &str, value: &str) -> &'static str {
        {
            let names = self.names.read().unwrap();
            if let Some(&cached) = names.get(key) {
                return cached;
            }
        }

        let mut names = self.names.write().unwrap();
        if let Some(&cached) = names.get(key) {
            return cached;
        }

        let leaked: &'static str = Box::leak(value.to_string().into_boxed_str());
        names.insert(key.to_string(), leaked);
        leaked
    }

    fn get_or_insert_description(&self, key: &str, value: &str) -> &'static str {
        {
            let descriptions = self.descriptions.read().unwrap();
            if let Some(&cached) = descriptions.get(key) {
                return cached;
            }
        }

        let mut descriptions = self.descriptions.write().unwrap();
        if let Some(&cached) = descriptions.get(key) {
            return cached;
        }

        let leaked: &'static str = Box::leak(value.to_string().into_boxed_str());
        descriptions.insert(key.to_string(), leaked);
        leaked
    }
}

fn string_cache() -> &'static StringCache {
    STRING_CACHE.get_or_init(StringCache::new)
}

/// Adapter that bridges a Thymos Tool to an AutoAgents ToolT.
///
/// This adapter:
/// - Preserves Thymos's capability-based security (via the inner tool)
/// - Converts ToolResultEnvelope to simple Value for AutoAgents
/// - Logs provenance information for observability
/// - Handles validation through the inner tool's validate() method
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::integration::ThymosToolAdapter;
/// use thymos_core::tools::Tool;
///
/// let thymos_tool: Arc<dyn Tool> = Arc::new(MyTool::new());
/// let adapter = ThymosToolAdapter::new(thymos_tool);
///
/// // Use adapter as AutoAgents ToolT
/// let result = adapter.execute(args).await?;
/// ```
pub struct ThymosToolAdapter {
    inner: Arc<dyn Tool>,
    /// Cached name for 'static lifetime requirement
    cached_name: &'static str,
    /// Cached description for 'static lifetime requirement
    cached_description: &'static str,
}

impl ThymosToolAdapter {
    /// Create a new adapter wrapping a Thymos Tool
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        let cache = string_cache();
        let name = tool.name();
        let description = tool.description();

        Self {
            cached_name: cache.get_or_insert_name(name, name),
            cached_description: cache.get_or_insert_description(name, description),
            inner: tool,
        }
    }

    /// Get the inner Thymos tool
    pub fn inner(&self) -> &Arc<dyn Tool> {
        &self.inner
    }

    /// Execute with full Thymos context and return the envelope
    pub async fn execute_with_context(
        &self,
        args: Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, crate::tools::ToolError> {
        self.inner.execute(args, ctx).await
    }
}

impl Debug for ThymosToolAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThymosToolAdapter")
            .field("name", &self.cached_name)
            .field("description", &self.cached_description)
            .finish()
    }
}

impl ToolT for ThymosToolAdapter {
    fn name(&self) -> &'static str {
        self.cached_name
    }

    fn description(&self) -> &'static str {
        self.cached_description
    }

    fn args_schema(&self) -> Value {
        self.inner.schema().parameters
    }
}

#[async_trait]
impl ToolRuntime for ThymosToolAdapter {
    async fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let ctx = ToolExecutionContext::default();

        let result = self
            .inner
            .execute(args, &ctx)
            .await
            .map_err(|e| ToolCallError::RuntimeError(Box::new(e)))?;

        let provenance = &result.provenance;
        tracing::debug!(
            tool = %provenance.tool_name,
            trace_id = ?provenance.trace_id,
            duration_ms = ?provenance.duration.as_millis(),
            "Tool execution completed via ThymosToolAdapter"
        );

        match &result.result {
            ToolResult::Success { value } => Ok(value.clone()),
            ToolResult::Error { error } => {
                Err(ToolCallError::RuntimeError(error.message.clone().into()))
            }
            ToolResult::Cancelled { reason } => {
                Err(ToolCallError::RuntimeError(format!("Tool execution cancelled: {}", reason).into()))
            }
        }
    }
}

/// Convert a collection of Thymos tools to AutoAgents ToolT boxes
pub fn thymos_tools_to_autoagents(tools: &[Arc<dyn Tool>]) -> Vec<Box<dyn ToolT>> {
    tools
        .iter()
        .map(|t| Box::new(ThymosToolAdapter::new(Arc::clone(t))) as Box<dyn ToolT>)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema};

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

    #[test]
    fn test_adapter_creation() {
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let adapter = ThymosToolAdapter::new(tool);

        assert_eq!(adapter.name(), "echo");
        assert_eq!(adapter.description(), "Echoes input back");
    }

    #[test]
    fn test_adapter_schema() {
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let adapter = ThymosToolAdapter::new(tool);

        let schema = adapter.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["message"].is_object());
    }

    #[tokio::test]
    async fn test_adapter_execution() {
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let adapter = ThymosToolAdapter::new(tool);

        let args = serde_json::json!({"message": "hello world"});
        let result = adapter.execute(args).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("hello world"));
    }

    #[test]
    fn test_debug_impl() {
        let tool = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let adapter = ThymosToolAdapter::new(tool);

        let debug_str = format!("{:?}", adapter);
        assert!(debug_str.contains("ThymosToolAdapter"));
        assert!(debug_str.contains("echo"));
    }

    #[test]
    fn test_tools_conversion() {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(EchoTool::new()),
            Arc::new(EchoTool::new()),
        ];

        let autoagent_tools = thymos_tools_to_autoagents(&tools);
        assert_eq!(autoagent_tools.len(), 2);

        for tool in &autoagent_tools {
            assert_eq!(tool.name(), "echo");
        }
    }

    #[test]
    fn test_string_caching() {
        let tool1 = Arc::new(EchoTool::new()) as Arc<dyn Tool>;
        let tool2 = Arc::new(EchoTool::new()) as Arc<dyn Tool>;

        let adapter1 = ThymosToolAdapter::new(tool1);
        let adapter2 = ThymosToolAdapter::new(tool2);

        // Both should point to the same cached string
        assert_eq!(adapter1.cached_name as *const str, adapter2.cached_name as *const str);
    }
}


