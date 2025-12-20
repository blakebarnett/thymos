//! Subagent API - Ergonomic interface for spawning isolated worker agents
//!
//! Provides a high-level API for creating and managing subagents with isolated
//! memory worktrees, simplifying the orchestrator-workers pattern.
//!
//! # Example
//!
//! ```rust,ignore
//! let researcher = agent.spawn_subagent(
//!     SubagentConfig::new("researcher")
//!         .with_purpose("Research a topic and report findings")
//!         .with_scope("research")
//! ).await?;
//!
//! let result = researcher.execute("Research Rust async", llm.clone()).await?;
//! let merge_result = researcher.merge_all(&agent).await?;
//! ```

use crate::agent::Agent;
use crate::error::Result;
use crate::tools::{CapabilityPolicy, Tool};
use autoagents_llm::LLMProvider as AutoAgentsLLMProvider;
use locai::prelude::Memory;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::worktree::MemoryWorktreeManager;

/// Configuration for spawning a subagent
#[derive(Clone)]
pub struct SubagentConfig {
    /// Subagent name (used for logging and identification)
    pub name: String,

    /// Purpose description (used in system prompt if not overridden)
    pub purpose: String,

    /// System prompt override for subagent
    pub system_prompt: Option<String>,

    /// Whether to copy parent memory at spawn
    pub inherit_memory: bool,

    /// Memory scope for new memories created by subagent
    pub memory_scope: Option<String>,

    /// Tools available to subagent (subset of parent tools)
    pub tools: Option<Vec<Arc<dyn Tool>>>,

    /// Capability policy override
    pub policy: Option<CapabilityPolicy>,

    /// Use reduced capability policy
    pub reduced_permissions: bool,
}

impl std::fmt::Debug for SubagentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubagentConfig")
            .field("name", &self.name)
            .field("purpose", &self.purpose)
            .field("system_prompt", &self.system_prompt)
            .field("inherit_memory", &self.inherit_memory)
            .field("memory_scope", &self.memory_scope)
            .field("tools", &self.tools.as_ref().map(|t| format!("[{} tools]", t.len())))
            .field("policy", &self.policy)
            .field("reduced_permissions", &self.reduced_permissions)
            .finish()
    }
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            name: "subagent".to_string(),
            purpose: "Worker subagent".to_string(),
            system_prompt: None,
            inherit_memory: true,
            memory_scope: None,
            tools: None,
            policy: None,
            reduced_permissions: true,
        }
    }
}

impl SubagentConfig {
    /// Create a new subagent config with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the purpose description
    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = purpose.into();
        self
    }

    /// Set a custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set whether to inherit parent memory
    pub fn inherit_memory(mut self, inherit: bool) -> Self {
        self.inherit_memory = inherit;
        self
    }

    /// Set the memory scope for new memories
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.memory_scope = Some(scope.into());
        self
    }

    /// Set the tools available to the subagent
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set a custom capability policy
    pub fn with_policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Set whether to use reduced permissions
    pub fn with_reduced_permissions(mut self, reduced: bool) -> Self {
        self.reduced_permissions = reduced;
        self
    }
}

/// Result from subagent task execution
#[derive(Debug)]
pub struct SubagentResult {
    /// Task output/response
    pub output: String,

    /// IDs of memories created during execution
    pub memories_created: Vec<String>,

    /// Names of tools that were called
    pub tools_called: Vec<String>,

    /// Execution duration
    pub duration: Duration,
}

/// Result from merging subagent changes
#[derive(Debug)]
pub struct MergeResult {
    /// Number of memories merged to parent
    pub memories_merged: usize,

    /// Commit hash if changes were committed
    pub commit_hash: Option<String>,
}

/// Handle to a running subagent with isolated memory
pub struct Subagent {
    /// Configuration used to create this subagent
    config: SubagentConfig,

    /// Worktree ID for this subagent
    worktree_id: String,

    /// The isolated agent instance
    agent: Arc<Agent>,

    /// Worktree manager for lifecycle operations
    worktree_manager: Arc<MemoryWorktreeManager>,

    /// Parent agent ID for merge operations
    parent_id: String,

    /// Track memories created since spawn
    memories_at_spawn: Vec<String>,
}

impl Subagent {
    /// Create a new subagent (called by Agent::spawn_subagent)
    pub(crate) async fn new(
        config: SubagentConfig,
        worktree_id: String,
        agent: Arc<Agent>,
        worktree_manager: Arc<MemoryWorktreeManager>,
        parent_id: String,
    ) -> Result<Self> {
        // Capture existing memory IDs to track new ones
        let existing_memories = agent
            .search_memories("")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect();

        Ok(Self {
            config,
            worktree_id,
            agent,
            worktree_manager,
            parent_id,
            memories_at_spawn: existing_memories,
        })
    }

    /// Get the underlying agent
    pub fn agent(&self) -> &Arc<Agent> {
        &self.agent
    }

    /// Get the subagent's worktree ID
    pub fn id(&self) -> &str {
        &self.worktree_id
    }

    /// Get the subagent's name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the subagent's purpose
    pub fn purpose(&self) -> &str {
        &self.config.purpose
    }

    /// Get the parent agent's ID
    pub fn parent_id(&self) -> &str {
        &self.parent_id
    }

    /// Execute a task and return the result
    ///
    /// # Arguments
    ///
    /// * `task` - The task description to execute
    /// * `llm` - LLM provider to use for execution
    ///
    /// # Returns
    ///
    /// Result containing the output and execution metadata
    pub async fn execute(
        &self,
        task: &str,
        llm: Arc<dyn AutoAgentsLLMProvider>,
    ) -> Result<SubagentResult> {
        let start = Instant::now();

        // Build prompt with purpose context
        let prompt = if let Some(ref system_prompt) = self.config.system_prompt {
            format!("{}\n\nTask: {}", system_prompt, task)
        } else {
            format!("Purpose: {}\n\nTask: {}", self.config.purpose, task)
        };

        // Execute via the agent
        let output = self.agent.execute(&prompt, llm).await?;

        // Get memories created during execution
        let current_memories: Vec<String> = self
            .agent
            .search_memories("")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect();

        let memories_created: Vec<String> = current_memories
            .into_iter()
            .filter(|id| !self.memories_at_spawn.contains(id))
            .collect();

        // Extract tool calls from output
        let tools_called = output
            .tool_calls
            .iter()
            .map(|tc| tc.tool_name.clone())
            .collect();

        Ok(SubagentResult {
            output: output.response,
            memories_created,
            tools_called,
            duration: start.elapsed(),
        })
    }

    /// Commit the subagent's changes to its worktree
    ///
    /// # Arguments
    ///
    /// * `message` - Commit message describing the changes
    ///
    /// # Returns
    ///
    /// The commit hash
    pub async fn commit(&self, message: &str) -> Result<String> {
        self.worktree_manager
            .commit_worktree_changes(&self.worktree_id, message)
            .await
    }

    /// Get memories created by this subagent since spawn
    pub async fn get_discoveries(&self) -> Result<Vec<Memory>> {
        let all_memories = self.agent.search_memories("").await?;

        let discoveries: Vec<Memory> = all_memories
            .into_iter()
            .filter(|m| !self.memories_at_spawn.contains(&m.id))
            .collect();

        Ok(discoveries)
    }

    /// Merge specific memories back to the parent agent
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent agent to merge into
    /// * `memory_ids` - IDs of memories to merge
    ///
    /// # Returns
    ///
    /// Result with merge statistics
    pub async fn merge_discoveries(
        self,
        parent: &Agent,
        memory_ids: Vec<String>,
    ) -> Result<MergeResult> {
        let mut merged_count = 0;

        // Get the discoveries
        let discoveries = self.get_discoveries().await?;

        // Filter to only requested IDs and copy to parent
        for memory in discoveries {
            if memory_ids.contains(&memory.id) {
                // Store in parent's memory with scope tag if configured
                let content = if let Some(ref scope) = self.config.memory_scope {
                    format!("[{}] {}", scope, memory.content)
                } else {
                    memory.content.clone()
                };

                parent.remember(content).await?;
                merged_count += 1;
            }
        }

        // Commit changes if any were merged
        let commit_hash = if merged_count > 0 {
            let hash = self
                .commit(&format!(
                    "Merged {} discoveries from subagent {}",
                    merged_count, self.config.name
                ))
                .await
                .ok();
            hash
        } else {
            None
        };

        // Clean up worktree
        self.worktree_manager
            .remove_worktree(&self.worktree_id, true)
            .await?;

        Ok(MergeResult {
            memories_merged: merged_count,
            commit_hash,
        })
    }

    /// Merge all changes to parent and cleanup
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent agent to merge into
    ///
    /// # Returns
    ///
    /// Result with merge statistics
    pub async fn merge_all(self, parent: &Agent) -> Result<MergeResult> {
        let discoveries = self.get_discoveries().await?;
        let memory_ids: Vec<String> = discoveries.iter().map(|m| m.id.clone()).collect();

        self.merge_discoveries(parent, memory_ids).await
    }

    /// Discard the subagent without affecting the parent
    ///
    /// All changes made by this subagent are lost.
    pub async fn discard(self) -> Result<()> {
        self.worktree_manager
            .remove_worktree(&self.worktree_id, true)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_config_defaults() {
        let config = SubagentConfig::default();
        assert_eq!(config.name, "subagent");
        assert_eq!(config.purpose, "Worker subagent");
        assert!(config.inherit_memory);
        assert!(config.reduced_permissions);
    }

    #[test]
    fn test_subagent_config_builder() {
        let config = SubagentConfig::new("researcher")
            .with_purpose("Research topics")
            .with_scope("research")
            .with_reduced_permissions(false);

        assert_eq!(config.name, "researcher");
        assert_eq!(config.purpose, "Research topics");
        assert_eq!(config.memory_scope, Some("research".to_string()));
        assert!(!config.reduced_permissions);
    }

    #[test]
    fn test_subagent_config_with_system_prompt() {
        let config = SubagentConfig::new("analyzer")
            .with_system_prompt("You are an expert code analyzer.");

        assert_eq!(
            config.system_prompt,
            Some("You are an expert code analyzer.".to_string())
        );
    }
}
