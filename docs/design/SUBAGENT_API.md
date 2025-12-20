# Subagent API Design

**Status**: Implemented  
**Priority**: Medium  
**Affects**: `thymos-core/src/agent.rs`, `thymos-core/src/memory/versioning/`

## Overview

An ergonomic API for spawning isolated subagents with worktree-backed memory, simplifying the orchestrator-workers pattern.

## Problem Statement

The `MemoryWorktreeManager` provides powerful isolation capabilities but requires manual orchestration:

```rust
// Current - verbose
let worktree_id = worktree_manager.create_worktree("main", Some("worker"), "agent-1", config).await?;
let worker_agent = worktree_manager.get_worktree_agent(&worktree_id).await?;
// ... do work ...
worktree_manager.commit_worktree_changes(&worktree_id, "results").await?;
worktree_manager.remove_worktree(&worktree_id, true).await?;
```

Agents need an ergonomic way to spawn workers with isolated memory.

## Proposed Design

### Configuration

```rust
/// Subagent configuration
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Subagent name (used for logging and identification)
    pub name: String,

    /// Purpose description
    pub purpose: String,

    /// System prompt override for subagent
    pub system_prompt: Option<String>,

    /// Whether to copy parent memory at spawn
    pub inherit_memory: bool,

    /// Memory scope for new memories created by subagent
    pub memory_scope: Option<String>,

    /// Tools available to subagent (subset of parent tools)
    pub tools: Option<Vec<Arc<dyn Tool>>>,

    /// Use reduced capability policy
    pub reduced_permissions: bool,
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
            reduced_permissions: true,
        }
    }
}

impl SubagentConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = purpose.into();
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn inherit_memory(mut self, inherit: bool) -> Self {
        self.inherit_memory = inherit;
        self
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.memory_scope = Some(scope.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = Some(tools);
        self
    }
}
```

### Subagent Handle

```rust
/// Handle to a running subagent
pub struct Subagent {
    config: SubagentConfig,
    worktree_id: String,
    agent: Arc<Agent>,
    worktree_manager: Arc<MemoryWorktreeManager>,
    parent_id: String,
}

impl Subagent {
    /// Get the underlying agent
    pub fn agent(&self) -> &Arc<Agent> {
        &self.agent
    }

    /// Get subagent ID
    pub fn id(&self) -> &str {
        &self.worktree_id
    }

    /// Get subagent name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Execute a task and return result
    pub async fn execute(
        &self,
        task: &str,
        llm: Arc<dyn LLMProvider>,
    ) -> Result<SubagentResult>;

    /// Commit subagent's changes to worktree
    pub async fn commit(&self, message: &str) -> Result<String>;

    /// Get memories created by this subagent
    pub async fn get_discoveries(&self) -> Result<Vec<Memory>>;

    /// Merge specific memories back to parent
    pub async fn merge_discoveries(
        self,
        parent: &Agent,
        memory_ids: Vec<String>,
    ) -> Result<MergeResult>;

    /// Merge all changes to parent and cleanup
    pub async fn merge_all(self, parent: &Agent) -> Result<MergeResult>;

    /// Discard subagent without affecting parent
    pub async fn discard(self) -> Result<()>;
}
```

### Result Types

```rust
#[derive(Debug)]
pub struct SubagentResult {
    /// Task output
    pub output: String,
    /// IDs of memories created during execution
    pub memories_created: Vec<String>,
    /// Tools that were called
    pub tools_called: Vec<String>,
    /// Execution duration
    pub duration: std::time::Duration,
}

#[derive(Debug)]
pub struct MergeResult {
    /// Number of memories merged
    pub memories_merged: usize,
    /// Commit hash if committed
    pub commit_hash: Option<String>,
}
```

### Agent Extension

```rust
impl Agent {
    /// Spawn a subagent with isolated memory
    pub async fn spawn_subagent(
        &self,
        config: SubagentConfig,
    ) -> Result<Subagent> {
        // Get or create worktree manager
        let manager = self.get_or_create_worktree_manager().await?;
        
        // Create worktree from current state
        let worktree_id = manager.create_worktree(
            "main",
            Some(&config.name),
            &format!("{}-{}", self.id, config.name),
            self.memory_config()?,
        ).await?;
        
        // Get subagent's isolated agent
        let agent = manager.get_worktree_agent(&worktree_id).await?;
        
        // Apply tool filtering if specified
        // Apply permission reduction if specified
        
        Ok(Subagent {
            config,
            worktree_id,
            agent,
            worktree_manager: manager,
            parent_id: self.id.clone(),
        })
    }

    /// Spawn multiple subagents for parallel work
    pub async fn spawn_subagents(
        &self,
        configs: Vec<SubagentConfig>,
    ) -> Result<Vec<Subagent>> {
        let mut subagents = Vec::with_capacity(configs.len());
        for config in configs {
            subagents.push(self.spawn_subagent(config).await?);
        }
        Ok(subagents)
    }
}
```

## Example Usage

### Basic Subagent

```rust
let agent = Agent::builder()
    .id("orchestrator")
    .build()
    .await?;

// Spawn a research worker
let researcher = agent.spawn_subagent(
    SubagentConfig::new("researcher")
        .with_purpose("Research a topic and report findings")
        .with_scope("research")
).await?;

// Execute task in isolation
let result = researcher.execute(
    "Research recent developments in Rust async",
    llm.clone(),
).await?;

println!("Found {} memories", result.memories_created.len());

// Merge discoveries back to parent
let merge_result = researcher.merge_all(&agent).await?;
println!("Merged {} memories", merge_result.memories_merged);
```

### Parallel Workers

```rust
let configs = vec![
    SubagentConfig::new("slack-observer")
        .with_purpose("Monitor Slack for relevant signals")
        .with_scope("observations"),
    SubagentConfig::new("github-observer")
        .with_purpose("Monitor GitHub for PR activity")
        .with_scope("observations"),
];

let workers = agent.spawn_subagents(configs).await?;

// Execute in parallel
let handles: Vec<_> = workers.into_iter().map(|worker| {
    let llm = llm.clone();
    tokio::spawn(async move {
        let result = worker.execute("Observe and report", llm).await?;
        Ok::<_, ThymosError>((worker, result))
    })
}).collect();

// Collect and merge results
for handle in handles {
    let (worker, result) = handle.await??;
    if !result.memories_created.is_empty() {
        worker.merge_all(&agent).await?;
    } else {
        worker.discard().await?;
    }
}
```

### Selective Merge

```rust
let worker = agent.spawn_subagent(
    SubagentConfig::new("analyzer")
).await?;

let result = worker.execute("Analyze codebase", llm).await?;

// Only merge the most relevant discoveries
let discoveries = worker.get_discoveries().await?;
let important: Vec<_> = discoveries.iter()
    .filter(|m| m.importance > 0.7)
    .map(|m| m.id.clone())
    .collect();

worker.merge_discoveries(&agent, important).await?;
```

## Implementation Notes

### Memory Inheritance

When `inherit_memory: true`:
1. Create worktree from current branch (snapshot copy)
2. Subagent starts with all parent memories

When `inherit_memory: false`:
1. Create worktree from empty state
2. Subagent starts fresh

### Tool Filtering

If `tools` is specified:
- Only those tools are available to subagent
- Parent tools not in list are inaccessible

If `tools` is None:
- Subagent inherits all parent tools
- `reduced_permissions` may limit capability policy

### Merge Strategies

**merge_all**: 
1. Get all memories created since spawn
2. Copy to parent's memory system
3. Optionally commit worktree changes
4. Remove worktree

**merge_discoveries** (selective):
1. Copy only specified memory IDs to parent
2. Optionally commit worktree changes
3. Remove worktree

**discard**:
1. Remove worktree with `force=true`
2. No changes to parent

## Testing Strategy

1. Spawn/discard lifecycle test
2. Memory isolation verification
3. Merge correctness test
4. Parallel workers test
5. Tool filtering test
6. Error handling (spawn failure, merge conflicts)
