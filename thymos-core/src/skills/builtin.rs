//! Built-in memory tools with provenance tracking
//!
//! Provides `memory_search` and `memory_store` tools that integrate with
//! the Thymos memory system and emit proper provenance metadata.

use crate::memory::MemorySystem;
use crate::tools::{
    Capability, CapabilityPolicy, CapabilitySet, Tool, ToolExecutionContext, ToolError,
    ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use super::{Skill, SkillBuilder};

/// Memory search result with provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// Matching memories
    pub memories: Vec<MemoryEntry>,
    /// Query used
    pub query: String,
    /// Number of results
    pub count: usize,
}

/// A memory entry in search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Memory ID
    pub id: String,
    /// Memory content
    pub content: String,
    /// Relevance score (if available)
    pub score: Option<f32>,
    /// Memory agent ID
    pub agent_id: Option<String>,
}

/// Tool for searching memories
///
/// This tool searches the memory system and returns matching memories
/// with provenance metadata suitable for tracing and replay.
pub struct MemorySearchTool {
    metadata: ToolMetadata,
    memory: Arc<MemorySystem>,
}

impl MemorySearchTool {
    /// Create a new memory search tool
    pub fn new(memory: Arc<MemorySystem>) -> Self {
        Self {
            metadata: ToolMetadata::new("memory_search", "Search stored memories for relevant information")
                .with_hint("Use to recall previously stored information")
                .with_hint("Provide a semantic query to find related memories")
                .with_returns("A list of matching memories with content and relevance scores")
                .with_tag("memory")
                .with_tag("builtin"),
            memory,
        }
    }
}

impl std::fmt::Debug for MemorySearchTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemorySearchTool")
            .field("name", &self.metadata.name)
            .finish()
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant memories"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 10)",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["query"]
        }))
    }

    fn required_capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_capabilities([Capability::MemoryRead])
    }

    async fn execute(
        &self,
        args: Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        let started_at = std::time::Instant::now();

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::new(
                    crate::tools::ToolErrorKind::Validation,
                    "query parameter is required",
                )
            })?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(10);

        // Perform the search
        let search_result = self
            .memory
            .search(query, Some(limit))
            .await
            .map_err(|e| {
                ToolError::new(
                    crate::tools::ToolErrorKind::Transient,
                    format!("Memory search failed: {}", e),
                )
            })?;

        // Convert to our result format
        let entries: Vec<MemoryEntry> = search_result
            .into_iter()
            .map(|m| MemoryEntry {
                id: m.id.clone(),
                content: m.content,
                score: None, // Locai doesn't expose score directly
                agent_id: if m.source.is_empty() { None } else { Some(m.source) },
            })
            .collect();

        let result = MemorySearchResult {
            query: query.to_string(),
            count: entries.len(),
            memories: entries,
        };

        let duration = started_at.elapsed();

        // Build provenance
        let mut provenance =
            ToolProvenance::new("memory_search", format!("query:{}", query)).with_duration(duration);

        if let Some(ref agent_id) = ctx.agent_id {
            provenance = provenance.with_agent_id(agent_id);
        }
        if let Some(ref trace_id) = ctx.trace_id {
            provenance = provenance.with_trace_id(trace_id);
        }

        Ok(ToolResultEnvelope::success(
            serde_json::to_value(result).unwrap_or(Value::Null),
            provenance,
        ))
    }
}

/// Tool for storing memories
///
/// This tool stores content in the memory system and returns
/// confirmation with provenance metadata.
pub struct MemoryStoreTool {
    metadata: ToolMetadata,
    memory: Arc<MemorySystem>,
}

impl MemoryStoreTool {
    /// Create a new memory store tool
    pub fn new(memory: Arc<MemorySystem>) -> Self {
        Self {
            metadata: ToolMetadata::new("memory_store", "Store information in memory for later retrieval")
                .with_hint("Use to remember important facts, decisions, or context")
                .with_hint("Stored memories can be retrieved later with memory_search")
                .with_returns("Confirmation with the memory ID")
                .with_tag("memory")
                .with_tag("builtin"),
            memory,
        }
    }
}

impl std::fmt::Debug for MemoryStoreTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryStoreTool")
            .field("name", &self.metadata.name)
            .finish()
    }
}

/// Memory store result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStoreResult {
    /// Whether the store was successful
    pub success: bool,
    /// The stored memory ID
    pub memory_id: String,
    /// Content stored (may be truncated in response)
    pub content_preview: String,
}

#[async_trait]
impl Tool for MemoryStoreTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to store in memory"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for categorization"
                }
            },
            "required": ["content"]
        }))
    }

    fn required_capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_capabilities([Capability::MemoryWrite])
    }

    async fn execute(
        &self,
        args: Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResultEnvelope, ToolError> {
        let started_at = std::time::Instant::now();

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::new(
                    crate::tools::ToolErrorKind::Validation,
                    "content parameter is required",
                )
            })?;

        // Store the memory
        let memory_id = self
            .memory
            .remember(content.to_string())
            .await
            .map_err(|e| {
                ToolError::new(
                    crate::tools::ToolErrorKind::Transient,
                    format!("Memory store failed: {}", e),
                )
            })?;

        // Create preview (truncate long content)
        let preview = if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.to_string()
        };

        let result = MemoryStoreResult {
            success: true,
            memory_id: memory_id.clone(),
            content_preview: preview,
        };

        let duration = started_at.elapsed();

        // Build provenance
        let mut provenance = ToolProvenance::new("memory_store", &memory_id)
            .with_duration(duration);

        if let Some(ref agent_id) = ctx.agent_id {
            provenance = provenance.with_agent_id(agent_id);
        }
        if let Some(ref trace_id) = ctx.trace_id {
            provenance = provenance.with_trace_id(trace_id);
        }

        Ok(ToolResultEnvelope::success(
            serde_json::to_value(result).unwrap_or(Value::Null),
            provenance,
        ))
    }
}

/// Create the built-in memory skill with search and store tools
///
/// This creates a skill bundle containing:
/// - `memory_search`: Search memories by semantic query
/// - `memory_store`: Store new memories
///
/// The skill has a `memory` scope and allows memory read/write capabilities.
pub fn create_memory_skill(memory: Arc<MemorySystem>) -> Skill {
    let search_tool = Arc::new(MemorySearchTool::new(Arc::clone(&memory)));
    let store_tool = Arc::new(MemoryStoreTool::new(memory));

    SkillBuilder::new("memory", "Built-in memory management tools")
        .add_tool(search_tool as Arc<dyn Tool>)
        .add_tool(store_tool as Arc<dyn Tool>)
        .with_prompt_template(
            "system",
            "You have access to memory tools. Use memory_search to recall information and memory_store to save important facts.",
        )
        .with_memory_scope("agent")
        .with_policy(CapabilityPolicy::memory_only())
        .with_tag("builtin")
        .with_tag("memory")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, MemoryMode};
    use tempfile::TempDir;

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
    async fn test_memory_store_tool() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = MemoryStoreTool::new(memory);

        assert_eq!(tool.name(), "memory_store");
        assert!(tool.description().contains("Store"));

        let caps = tool.required_capabilities();
        assert!(caps.contains(Capability::MemoryWrite));
    }

    #[tokio::test]
    async fn test_memory_search_tool() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = MemorySearchTool::new(memory);

        assert_eq!(tool.name(), "memory_search");
        assert!(tool.description().contains("Search"));

        let caps = tool.required_capabilities();
        assert!(caps.contains(Capability::MemoryRead));
    }

    #[tokio::test]
    async fn test_memory_store_execution() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = MemoryStoreTool::new(memory);

        let ctx = ToolExecutionContext::new()
            .with_agent_id("test_agent")
            .with_trace_id("trace_123");

        let args = serde_json::json!({
            "content": "This is a test memory"
        });

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert!(envelope.is_success());

        let value = envelope.value().unwrap();
        assert_eq!(value["success"], true);
        assert!(!value["memory_id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_memory_search_execution() {
        let (memory, _temp_dir) = create_test_memory().await;

        // Store something first
        memory.remember("Important fact about Rust".to_string()).await.unwrap();

        let tool = MemorySearchTool::new(memory);
        let ctx = ToolExecutionContext::new().with_agent_id("test_agent");

        let args = serde_json::json!({
            "query": "Rust",
            "limit": 5
        });

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert!(envelope.is_success());

        let value = envelope.value().unwrap();
        assert_eq!(value["query"], "Rust");
    }

    #[tokio::test]
    async fn test_create_memory_skill() {
        let (memory, _temp_dir) = create_test_memory().await;
        let skill = create_memory_skill(memory);

        assert_eq!(skill.name(), "memory");
        assert_eq!(skill.tools().len(), 2);
        assert!(skill.get_tool("memory_search").is_some());
        assert!(skill.get_tool("memory_store").is_some());
        assert!(skill.has_tag("builtin"));
        assert!(skill.has_tag("memory"));
        assert!(skill.get_prompt("system").is_some());
    }

    #[test]
    fn test_memory_search_result_serialization() {
        let result = MemorySearchResult {
            query: "test".to_string(),
            count: 1,
            memories: vec![MemoryEntry {
                id: "mem_123".to_string(),
                content: "Test content".to_string(),
                score: Some(0.9),
                agent_id: Some("agent_1".to_string()),
            }],
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["query"], "test");
        assert_eq!(json["count"], 1);
        assert_eq!(json["memories"][0]["id"], "mem_123");
    }

    #[test]
    fn test_memory_store_result_serialization() {
        let result = MemoryStoreResult {
            success: true,
            memory_id: "mem_456".to_string(),
            content_preview: "Preview...".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["memory_id"], "mem_456");
    }

    #[tokio::test]
    async fn test_store_requires_content() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = MemoryStoreTool::new(memory);
        let ctx = ToolExecutionContext::new();

        let args = serde_json::json!({});

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_requires_query() {
        let (memory, _temp_dir) = create_test_memory().await;
        let tool = MemorySearchTool::new(memory);
        let ctx = ToolExecutionContext::new();

        let args = serde_json::json!({});

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }
}

