//! Skill definition and builder
//!
//! Skills bundle tools with prompts, memory scope, and policy.

use crate::tools::{CapabilityPolicy, Tool, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Error type for skill operations
#[derive(Debug, Clone)]
pub enum SkillError {
    /// Skill already registered
    AlreadyRegistered(String),
    /// Skill not found
    NotFound(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Tool error
    ToolError(String),
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillError::AlreadyRegistered(name) => {
                write!(f, "Skill '{}' is already registered", name)
            }
            SkillError::NotFound(name) => {
                write!(f, "Skill '{}' not found", name)
            }
            SkillError::InvalidConfig(msg) => {
                write!(f, "Invalid skill configuration: {}", msg)
            }
            SkillError::ToolError(msg) => {
                write!(f, "Tool error: {}", msg)
            }
        }
    }
}

impl std::error::Error for SkillError {}

/// A prompt template with variable substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Template name
    pub name: String,
    /// Template content with {{variable}} placeholders
    pub template: String,
    /// Description of the template
    pub description: Option<String>,
}

impl PromptTemplate {
    /// Create a new prompt template
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: template.into(),
            description: None,
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Render the template with variable substitution
    pub fn render(&self, variables: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

/// A skill bundles tools, prompts, memory scope, and policy
#[derive(Clone)]
pub struct Skill {
    /// Skill name (unique identifier, hyphen-case for Anthropic compatibility)
    name: String,
    /// Human-readable description
    description: String,
    /// Tools included in this skill
    tools: Vec<Arc<dyn Tool>>,
    /// Prompt templates
    prompts: HashMap<String, PromptTemplate>,
    /// Memory scope for this skill (optional)
    memory_scope: Option<String>,
    /// Capability policy override (optional)
    policy: Option<CapabilityPolicy>,
    /// Tags for categorization
    tags: Vec<String>,
    /// License (for Anthropic Skills compatibility)
    license: Option<String>,
    /// Arbitrary metadata key-value pairs (for Anthropic Skills compatibility)
    metadata: HashMap<String, String>,
    /// Instructions markdown body (for Anthropic Skills compatibility)
    instructions: Option<String>,
    /// Allowed tool names (for Anthropic Skills compatibility)
    allowed_tools: Option<Vec<String>>,
}

impl std::fmt::Debug for Skill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Skill")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("tool_count", &self.tools.len())
            .field("prompt_count", &self.prompts.len())
            .field("memory_scope", &self.memory_scope)
            .field("has_policy", &self.policy.is_some())
            .field("tags", &self.tags)
            .finish()
    }
}

impl Skill {
    /// Create a new skill builder
    pub fn builder(name: impl Into<String>, description: impl Into<String>) -> SkillBuilder {
        SkillBuilder::new(name, description)
    }

    /// Get the skill name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the skill description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get all tools in this skill
    pub fn tools(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    /// Get tool names
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name)
    }

    /// Get all prompts
    pub fn prompts(&self) -> &HashMap<String, PromptTemplate> {
        &self.prompts
    }

    /// Get a prompt by name
    pub fn get_prompt(&self, name: &str) -> Option<&PromptTemplate> {
        self.prompts.get(name)
    }

    /// Get the memory scope
    pub fn memory_scope(&self) -> Option<&str> {
        self.memory_scope.as_deref()
    }

    /// Get the capability policy (if set)
    pub fn policy(&self) -> Option<&CapabilityPolicy> {
        self.policy.as_ref()
    }

    /// Get tags
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Check if the skill has a tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Get the license
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }

    /// Get metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Get a metadata value by key
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Get instructions markdown body
    pub fn instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Get allowed tools list (Anthropic format)
    pub fn allowed_tools_list(&self) -> Option<&[String]> {
        self.allowed_tools.as_deref()
    }

    /// Create a ToolRegistry from this skill's tools
    pub fn to_registry(&self) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for tool in &self.tools {
            let _ = registry.register(Arc::clone(tool));
        }
        registry
    }

    /// Check if a tool is allowed by this skill's policy
    pub fn tool_allowed(&self, tool: &dyn Tool) -> bool {
        match &self.policy {
            Some(policy) => {
                let required = tool.required_capabilities();
                policy.check_all(&required).is_ok()
            }
            None => true,
        }
    }

    /// Get tools filtered by this skill's policy
    pub fn allowed_tools(&self) -> Vec<&Arc<dyn Tool>> {
        match &self.policy {
            Some(policy) => self
                .tools
                .iter()
                .filter(|t| {
                    let required = t.required_capabilities();
                    policy.check_all(&required).is_ok()
                })
                .collect(),
            None => self.tools.iter().collect(),
        }
    }
}

/// Builder for creating Skills
pub struct SkillBuilder {
    name: String,
    description: String,
    tools: Vec<Arc<dyn Tool>>,
    prompts: HashMap<String, PromptTemplate>,
    memory_scope: Option<String>,
    policy: Option<CapabilityPolicy>,
    tags: Vec<String>,
    license: Option<String>,
    metadata: HashMap<String, String>,
    instructions: Option<String>,
    allowed_tools: Option<Vec<String>>,
}

impl SkillBuilder {
    /// Create a new skill builder
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            tools: Vec::new(),
            prompts: HashMap::new(),
            memory_scope: None,
            policy: None,
            tags: Vec::new(),
            license: None,
            metadata: HashMap::new(),
            instructions: None,
            allowed_tools: None,
        }
    }

    /// Add a tool to the skill
    pub fn add_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add multiple tools
    pub fn add_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Add a prompt template
    pub fn add_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompts.insert(prompt.name.clone(), prompt);
        self
    }

    /// Add a simple prompt template by name and content
    pub fn with_prompt_template(
        mut self,
        name: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        let prompt = PromptTemplate::new(name, template);
        self.prompts.insert(prompt.name.clone(), prompt);
        self
    }

    /// Set the memory scope
    pub fn with_memory_scope(mut self, scope: impl Into<String>) -> Self {
        self.memory_scope = Some(scope.into());
        self
    }

    /// Set the capability policy
    pub fn with_policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Set the license (for Anthropic Skills compatibility)
    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    /// Add a metadata key-value pair
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set all metadata
    pub fn with_all_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set the instructions markdown body
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// Set allowed tools list (Anthropic format)
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// Build the skill
    pub fn build(self) -> Skill {
        Skill {
            name: self.name,
            description: self.description,
            tools: self.tools,
            prompts: self.prompts,
            memory_scope: self.memory_scope,
            policy: self.policy,
            tags: self.tags,
            license: self.license,
            metadata: self.metadata,
            instructions: self.instructions,
            allowed_tools: self.allowed_tools,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{
        Capability, CapabilitySet, ToolExecutionContext, ToolMetadata, ToolProvenance,
        ToolResultEnvelope, ToolSchema,
    };
    use async_trait::async_trait;

    struct MockTool {
        metadata: ToolMetadata,
        capabilities: CapabilitySet,
    }

    impl MockTool {
        fn new(name: &str, caps: &[Capability]) -> Self {
            Self {
                metadata: ToolMetadata::new(name, format!("{} tool", name)),
                capabilities: CapabilitySet::from_capabilities(caps.iter().copied()),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::empty()
        }

        fn required_capabilities(&self) -> CapabilitySet {
            self.capabilities.clone()
        }

        async fn execute(
            &self,
            _args: serde_json::Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, crate::tools::ToolError> {
            let provenance = ToolProvenance::new(&self.metadata.name, "test");
            Ok(ToolResultEnvelope::success(
                serde_json::json!({"status": "ok"}),
                provenance,
            ))
        }
    }

    #[test]
    fn test_skill_builder() {
        let skill = Skill::builder("research", "Perform research tasks")
            .add_tool(Arc::new(MockTool::new("search", &[Capability::Network])))
            .with_prompt_template("system", "You are a research assistant")
            .with_memory_scope("research")
            .with_tag("web")
            .build();

        assert_eq!(skill.name(), "research");
        assert_eq!(skill.description(), "Perform research tasks");
        assert_eq!(skill.tools().len(), 1);
        assert_eq!(skill.memory_scope(), Some("research"));
        assert!(skill.has_tag("web"));
    }

    #[test]
    fn test_prompt_template_render() {
        let template = PromptTemplate::new(
            "greeting",
            "Hello {{name}}, welcome to {{place}}!",
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("place".to_string(), "Thymos".to_string());

        let rendered = template.render(&vars);
        assert_eq!(rendered, "Hello Alice, welcome to Thymos!");
    }

    #[test]
    fn test_skill_policy_filtering() {
        let network_tool = Arc::new(MockTool::new("network_tool", &[Capability::Network]));
        let safe_tool = Arc::new(MockTool::new("safe_tool", &[]));

        let skill = Skill::builder("mixed", "Mixed tools")
            .add_tool(network_tool as Arc<dyn Tool>)
            .add_tool(safe_tool as Arc<dyn Tool>)
            .with_policy(CapabilityPolicy::deny_all())
            .build();

        let allowed = skill.allowed_tools();
        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].name(), "safe_tool");
    }

    #[test]
    fn test_skill_to_registry() {
        let tool1 = Arc::new(MockTool::new("tool1", &[]));
        let tool2 = Arc::new(MockTool::new("tool2", &[]));

        let skill = Skill::builder("test", "Test skill")
            .add_tool(tool1 as Arc<dyn Tool>)
            .add_tool(tool2 as Arc<dyn Tool>)
            .build();

        let registry = skill.to_registry();
        assert_eq!(registry.len(), 2);
        assert!(registry.contains("tool1"));
        assert!(registry.contains("tool2"));
    }

    #[test]
    fn test_get_tool() {
        let tool = Arc::new(MockTool::new("mytool", &[]));

        let skill = Skill::builder("test", "Test")
            .add_tool(tool as Arc<dyn Tool>)
            .build();

        assert!(skill.get_tool("mytool").is_some());
        assert!(skill.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_get_prompt() {
        let skill = Skill::builder("test", "Test")
            .with_prompt_template("greeting", "Hello!")
            .with_prompt_template("farewell", "Goodbye!")
            .build();

        assert!(skill.get_prompt("greeting").is_some());
        assert!(skill.get_prompt("farewell").is_some());
        assert!(skill.get_prompt("unknown").is_none());
    }

    #[test]
    fn test_skill_without_policy() {
        let network_tool = Arc::new(MockTool::new("network_tool", &[Capability::Network]));

        let skill = Skill::builder("no_policy", "No policy")
            .add_tool(network_tool as Arc<dyn Tool>)
            .build();

        assert!(skill.policy().is_none());
        assert_eq!(skill.allowed_tools().len(), 1);
    }
}

