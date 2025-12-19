//! Tool Registry for tool registration, lookup, and semantic discovery
//!
//! The `ToolRegistry` provides:
//! - Tool registration with duplicate detection
//! - Lookup by name with schema validation
//! - Semantic discovery over tool descriptions
//! - Filtering by capability requirements and policy
//! - Metadata exposure for MCP tool listing
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::tools::{ToolRegistry, Tool, CapabilityPolicy};
//!
//! let mut registry = ToolRegistry::new();
//! registry.register(Arc::new(SearchTool::new()))?;
//! registry.register(Arc::new(BrowseTool::new()))?;
//!
//! // Lookup by name
//! let tool = registry.get("search").unwrap();
//!
//! // Discover tools by description
//! let tools = registry.discover("find information online");
//!
//! // Filter by policy
//! let allowed = registry.filter_by_policy(&CapabilityPolicy::allow_all());
//! ```

use super::capability::{CapabilityPolicy, CapabilitySet};
use super::tool::Tool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Error type for registry operations
#[derive(Debug, Clone)]
pub enum RegistryError {
    /// Tool with this name already exists
    DuplicateTool(String),
    /// Tool not found
    NotFound(String),
    /// Validation error
    ValidationError(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DuplicateTool(name) => {
                write!(f, "Tool '{}' is already registered", name)
            }
            RegistryError::NotFound(name) => {
                write!(f, "Tool '{}' not found", name)
            }
            RegistryError::ValidationError(msg) => {
                write!(f, "Validation error: {}", msg)
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Summary of a tool for discovery and MCP listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Version if available
    pub version: Option<String>,
}

impl From<&dyn Tool> for ToolSummary {
    fn from(tool: &dyn Tool) -> Self {
        let metadata = tool.metadata();
        let caps = tool.required_capabilities();
        Self {
            name: metadata.name.clone(),
            description: metadata.description.clone(),
            tags: metadata.tags.clone(),
            required_capabilities: caps.iter().map(|c| c.to_string()).collect(),
            version: metadata.version.clone(),
        }
    }
}

/// Discovery strategy for finding tools
pub trait DiscoveryStrategy: Send + Sync {
    /// Score how well a tool matches the query (0.0 to 1.0)
    fn score(&self, query: &str, tool: &dyn Tool) -> f32;
}

/// Simple substring-based discovery strategy
///
/// Matches query against tool name, description, tags, and usage hints.
#[derive(Debug, Clone, Default)]
pub struct SubstringDiscovery;

impl DiscoveryStrategy for SubstringDiscovery {
    fn score(&self, query: &str, tool: &dyn Tool) -> f32 {
        let query_lower = query.to_lowercase();
        let metadata = tool.metadata();

        let mut score = 0.0f32;

        // Exact name match = highest score
        if metadata.name.to_lowercase() == query_lower {
            return 1.0;
        }

        // Name contains query
        if metadata.name.to_lowercase().contains(&query_lower) {
            score = score.max(0.9);
        }

        // Description contains query
        if metadata.description.to_lowercase().contains(&query_lower) {
            score = score.max(0.7);
        }

        // Tags match
        for tag in &metadata.tags {
            if tag.to_lowercase().contains(&query_lower) {
                score = score.max(0.6);
            }
        }

        // Usage hints match
        for hint in &metadata.usage_hints {
            if hint.to_lowercase().contains(&query_lower) {
                score = score.max(0.5);
            }
        }

        // Check for word overlap
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let desc_lower = metadata.description.to_lowercase();

        let mut word_matches = 0;
        for word in &query_words {
            if word.len() > 2 && desc_lower.contains(word) {
                word_matches += 1;
            }
        }

        if !query_words.is_empty() {
            let word_score = (word_matches as f32 / query_words.len() as f32) * 0.6;
            score = score.max(word_score);
        }

        score
    }
}

/// Result of a tool discovery search
#[derive(Clone)]
pub struct DiscoveryResult {
    /// The matching tool
    pub tool: Arc<dyn Tool>,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
}

impl std::fmt::Debug for DiscoveryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveryResult")
            .field("tool_name", &self.tool.name())
            .field("score", &self.score)
            .finish()
    }
}

/// Registry for managing and discovering tools
///
/// The registry provides:
/// - Registration with duplicate detection
/// - Lookup by name
/// - Semantic discovery over descriptions
/// - Policy-based filtering
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    discovery_strategy: Box<dyn DiscoveryStrategy>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tool_count", &self.tools.len())
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolRegistry {
    /// Create a new empty registry with default discovery strategy
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            discovery_strategy: Box::new(SubstringDiscovery),
        }
    }

    /// Create a registry with a custom discovery strategy
    pub fn with_discovery(strategy: Box<dyn DiscoveryStrategy>) -> Self {
        Self {
            tools: HashMap::new(),
            discovery_strategy: strategy,
        }
    }

    /// Register a tool
    ///
    /// Returns an error if a tool with the same name is already registered.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), RegistryError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(RegistryError::DuplicateTool(name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Register multiple tools at once
    ///
    /// Fails if any tool name is duplicated.
    pub fn register_all(&mut self, tools: Vec<Arc<dyn Tool>>) -> Result<(), RegistryError> {
        for tool in tools {
            self.register(tool)?;
        }
        Ok(())
    }

    /// Unregister a tool by name
    ///
    /// Returns the tool if found, None otherwise.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.remove(name)
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Check if a tool is registered
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all registered tools
    pub fn all(&self) -> Vec<&Arc<dyn Tool>> {
        self.tools.values().collect()
    }

    /// Get all tool names
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// List all tools with their summaries
    pub fn list(&self) -> Vec<ToolSummary> {
        self.tools
            .values()
            .map(|t| ToolSummary::from(t.as_ref()))
            .collect()
    }

    /// Discover tools matching a query
    ///
    /// Returns tools sorted by relevance score (highest first).
    /// Only returns tools with a score above the threshold (default 0.1).
    pub fn discover(&self, query: &str) -> Vec<DiscoveryResult> {
        self.discover_with_threshold(query, 0.1)
    }

    /// Discover tools with a custom score threshold
    pub fn discover_with_threshold(&self, query: &str, threshold: f32) -> Vec<DiscoveryResult> {
        let mut results: Vec<DiscoveryResult> = self
            .tools
            .values()
            .filter_map(|tool| {
                let score = self.discovery_strategy.score(query, tool.as_ref());
                if score >= threshold {
                    Some(DiscoveryResult {
                        tool: Arc::clone(tool),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Filter tools by capability policy
    ///
    /// Returns only tools whose required capabilities are allowed by the policy.
    pub fn filter_by_policy(&self, policy: &CapabilityPolicy) -> Vec<&Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| {
                let required = tool.required_capabilities();
                policy.check_all(&required).is_ok()
            })
            .collect()
    }

    /// Filter tools by required capabilities
    ///
    /// Returns tools that require any of the given capabilities.
    pub fn filter_by_capabilities(&self, capabilities: &CapabilitySet) -> Vec<&Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| {
                let required = tool.required_capabilities();
                required.iter().any(|cap| capabilities.contains(*cap))
            })
            .collect()
    }

    /// Filter tools by tag
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| tool.metadata().tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Get tools for MCP listing
    ///
    /// Returns tool metadata suitable for MCP tool listing.
    pub fn mcp_tools(&self) -> Vec<McpToolInfo> {
        self.tools
            .values()
            .map(|tool| McpToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.schema().parameters.clone(),
            })
            .collect()
    }

    /// Get tools for MCP listing, filtered by policy
    pub fn mcp_tools_filtered(&self, policy: &CapabilityPolicy) -> Vec<McpToolInfo> {
        self.filter_by_policy(policy)
            .into_iter()
            .map(|tool| McpToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.schema().parameters.clone(),
            })
            .collect()
    }

    /// Validate arguments against a tool's schema
    pub fn validate_args(&self, tool_name: &str, args: &serde_json::Value) -> Result<(), RegistryError> {
        let tool = self
            .get(tool_name)
            .ok_or_else(|| RegistryError::NotFound(tool_name.to_string()))?;

        if let Err(errors) = tool.validate(args) {
            let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(RegistryError::ValidationError(error_messages.join(", ")));
        }

        Ok(())
    }
}

/// Tool information for MCP listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Capability, ToolExecutionContext, ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema};
    use async_trait::async_trait;

    struct SearchTool {
        metadata: ToolMetadata,
    }

    impl SearchTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("search", "Search the web for information")
                    .with_tag("web")
                    .with_tag("research")
                    .with_hint("Use to find current information"),
            }
        }
    }

    #[async_trait]
    impl Tool for SearchTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::new(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }))
        }

        fn required_capabilities(&self) -> CapabilitySet {
            CapabilitySet::from_capabilities([Capability::Network])
        }

        async fn execute(
            &self,
            args: serde_json::Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, super::super::ToolError> {
            let provenance = ToolProvenance::new("search", "test");
            Ok(ToolResultEnvelope::success(args, provenance))
        }
    }

    struct CalculatorTool {
        metadata: ToolMetadata,
    }

    impl CalculatorTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("calculator", "Perform mathematical calculations")
                    .with_tag("math")
                    .with_hint("Use for arithmetic and math problems"),
            }
        }
    }

    #[async_trait]
    impl Tool for CalculatorTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::new(serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "string" }
                },
                "required": ["expression"]
            }))
        }

        async fn execute(
            &self,
            args: serde_json::Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, super::super::ToolError> {
            let provenance = ToolProvenance::new("calculator", "test");
            Ok(ToolResultEnvelope::success(args, provenance))
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(SearchTool::new());

        assert!(registry.register(tool.clone()).is_ok());
        assert!(registry.contains("search"));
        assert!(registry.get("search").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = ToolRegistry::new();
        let tool1 = Arc::new(SearchTool::new());
        let tool2 = Arc::new(SearchTool::new());

        assert!(registry.register(tool1).is_ok());
        assert!(matches!(
            registry.register(tool2),
            Err(RegistryError::DuplicateTool(_))
        ));
    }

    #[test]
    fn test_register_all() {
        let mut registry = ToolRegistry::new();
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(SearchTool::new()),
            Arc::new(CalculatorTool::new()),
        ];

        assert!(registry.register_all(tools).is_ok());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_unregister() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(SearchTool::new());

        registry.register(tool).unwrap();
        assert!(registry.contains("search"));

        let removed = registry.unregister("search");
        assert!(removed.is_some());
        assert!(!registry.contains("search"));
    }

    #[test]
    fn test_list() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let summaries = registry.list();
        assert_eq!(summaries.len(), 2);

        let names: Vec<&str> = summaries.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"search"));
        assert!(names.contains(&"calculator"));
    }

    #[test]
    fn test_discover_exact_name() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let results = registry.discover("search");
        assert!(!results.is_empty());
        assert_eq!(results[0].tool.name(), "search");
        assert_eq!(results[0].score, 1.0);
    }

    #[test]
    fn test_discover_by_description() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let results = registry.discover("web information");
        assert!(!results.is_empty());
        assert_eq!(results[0].tool.name(), "search");
    }

    #[test]
    fn test_discover_by_tag() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let results = registry.discover("math");
        assert!(!results.is_empty());
        assert_eq!(results[0].tool.name(), "calculator");
    }

    #[test]
    fn test_filter_by_policy_allow_all() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let policy = CapabilityPolicy::allow_all();
        let filtered = registry.filter_by_policy(&policy);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_policy_deny_network() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let policy = CapabilityPolicy::deny_all();
        let filtered = registry.filter_by_policy(&policy);

        // Only calculator should pass (no capabilities required)
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), "calculator");
    }

    #[test]
    fn test_filter_by_tag() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let web_tools = registry.filter_by_tag("web");
        assert_eq!(web_tools.len(), 1);
        assert_eq!(web_tools[0].name(), "search");

        let math_tools = registry.filter_by_tag("math");
        assert_eq!(math_tools.len(), 1);
        assert_eq!(math_tools[0].name(), "calculator");
    }

    #[test]
    fn test_mcp_tools() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let mcp_tools = registry.mcp_tools();
        assert_eq!(mcp_tools.len(), 1);
        assert_eq!(mcp_tools[0].name, "search");
        assert!(!mcp_tools[0].description.is_empty());
        assert!(mcp_tools[0].input_schema.is_object());
    }

    #[test]
    fn test_names() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();
        registry
            .register(Arc::new(CalculatorTool::new()) as Arc<dyn Tool>)
            .unwrap();

        let names = registry.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"search"));
        assert!(names.contains(&"calculator"));
    }

    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.all().is_empty());
        assert!(registry.discover("anything").is_empty());
    }

    #[test]
    fn test_discovery_threshold() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(SearchTool::new()) as Arc<dyn Tool>)
            .unwrap();

        // With high threshold, nothing matches generic queries
        let results = registry.discover_with_threshold("xyz", 0.9);
        assert!(results.is_empty());

        // With low threshold, even weak matches appear
        let results = registry.discover_with_threshold("information", 0.1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_tool_summary() {
        let tool = SearchTool::new();
        let summary = ToolSummary::from(&tool as &dyn Tool);

        assert_eq!(summary.name, "search");
        assert!(summary.description.contains("Search"));
        assert!(summary.tags.contains(&"web".to_string()));
        assert!(!summary.required_capabilities.is_empty());
    }
}

