# LLM-Native Agent Design for Thymos

**Date**: December 2024  
**Last Updated**: December 2025  
**Status**: Design Phase  
**Priority**: ⚡ **CRITICAL** - Strategic direction for framework evolution

## Executive Summary

This document synthesizes insights from Anthropic's "Building Effective Agents" guide and Google's Agent Development Kit (ADK) documentation to define Thymos's evolution into an LLM-native agent framework. The goal is to shift Thymos from a memory-orchestration layer to a framework that embraces the fuzzy, probabilistic nature of LLMs while providing structured patterns for building reliable agents.

## Current State Assessment

### What Thymos Does Well

| Feature | Status | Notes |
|---------|--------|-------|
| **Memory System** | ✅ Complete | Hybrid modes, semantic search, forgetting curves |
| **Memory Versioning** | ✅ Complete | Git-style branches, commits, worktrees, merge ⭐ **UNIQUE** |
| **Concept Extraction** | ✅ Complete | Regex + LLM extractors, alias resolution |
| **LLM Provider Abstraction** | ✅ Complete | Groq, Ollama providers, streaming |
| **Embedding Providers** | ✅ Complete | Local fastembed, provider factory |
| **Pub/Sub Coordination** | ✅ Complete | Local, distributed, hybrid modes |
| **Agent Lifecycle** | ✅ Complete | Relevance-based states, event hooks |

### Critical Gaps for LLM-Centric Design

| Feature | Status | Impact |
|---------|--------|--------|
| **MCP Server Interface** | ❌ Missing | Can't integrate with Claude ecosystem |
| **Tool/Skill System** | ❌ Missing | No structured agent capabilities |
| **Workflow Patterns** | ❌ Missing | No Chain/Route/Parallel/Orchestrate |
| **Structured Output Parsing** | ⚠️ Basic | No validation, retry, repair |
| **Context Management** | ⚠️ Partial | Versioning enables rollback; needs summarization layer |
| **Subagent Architecture** | ⚠️ Partial | Worktrees enable isolation; needs high-level API |
| **Execution Tracing** | ❌ Missing | No observability |

### Missing Foundations (Required for Production-Grade Agents)

These are enabling layers that make the above features safe, testable, and operable.

| Foundation | Status | Why It Matters |
|-----------|--------|----------------|
| **Tool Runtime Safety & Permissions** | ❌ Missing | Tools are the largest risk surface (secrets, filesystem, network) |
| **Unified Error Model & Recovery Semantics** | ❌ Missing | Reliable retries/cancellation across Chain/Parallel/Orchestrator |
| **Reproducibility & Replay** | ❌ Missing | Debug regressions; compare attempts; support evaluator-optimizer |
| **Evaluation Harness** | ❌ Missing | Prevent prompt/tool regressions; validate workflows without live APIs |
| **Concurrency Limits & Cancellation** | ❌ Missing | Prevent runaway parallelism; enforce timeouts and structured concurrency |
| **Observability Standards (OTel export)** | ❌ Missing | Integrate with existing tracing/metrics stacks; avoid bespoke tooling |

> **Note**: Context Management and Subagent Architecture have strong foundations through Memory Versioning (commits, worktrees). The gap is in high-level abstractions that expose these capabilities ergonomically.

---

## Anthropic Insights: Building Effective Agents

### Core Philosophy

> "Start with the simplest solution possible, and only add complexity when needed."

Anthropic emphasizes that many applications don't need complex agentic systems. When they do, the key is using **well-defined patterns** rather than ad-hoc implementations.

### Workflow Patterns

Anthropic identifies five foundational workflow patterns:

#### 1. Prompt Chaining

Sequential LLM calls where each output feeds the next input.

```
Input → LLM₁ → Output₁ → Gate → LLM₂ → Output₂ → ... → Final
```

**Use Cases**:
- Multi-step document processing
- Translation with quality check
- Content generation with editing

**Thymos Application**:
- Memory-informed generation chains
- Concept extraction → validation → storage

#### 2. Routing

Classify input and direct to specialized handlers.

```
Input → Classifier → Route A (specialized prompt)
                  → Route B (specialized prompt)
                  → Route C (specialized prompt)
```

**Use Cases**:
- Customer service (billing/technical/sales)
- Query type detection
- Intent-based handling

**Thymos Application**:
- Route to appropriate agent skills
- Memory scope selection
- Handler selection by semantic similarity

#### 3. Parallelization

Multiple LLM calls simultaneously, then aggregate results.

```
Input → LLM₁ ──┐
      → LLM₂ ──┼→ Aggregator → Output
      → LLM₃ ──┘
```

**Variants**:
- **Sectioning**: Split task into independent parts
- **Voting**: Multiple attempts, majority wins

**Thymos Application**:
- Parallel memory searches + generation
- Multi-perspective analysis
- Consensus-based extraction

#### 4. Orchestrator-Workers

Central LLM delegates to specialized workers.

```
                    ┌→ Worker₁ →┐
Input → Orchestrator├→ Worker₂ →┼→ Synthesizer → Output
                    └→ Worker₃ →┘
```

**Use Cases**:
- Complex research tasks
- Multi-file code generation
- Report generation

**Thymos Application**:
- Main agent + subagents
- Skill-based delegation
- Complex query decomposition

#### 5. Evaluator-Optimizer

Generate, evaluate, refine in a loop.

```
Input → Generator → Output → Evaluator → Feedback
              ↑                              │
              └──────────────────────────────┘
```

**Use Cases**:
- Code generation with testing
- Writing with revision
- Search with quality check

**Thymos Application**:
- Memory consolidation refinement
- Response quality improvement
- Concept extraction validation

### Tool Design Principles

Anthropic emphasizes that **tool design is as important as prompt engineering**:

1. **Clear Names**: Name tools from the model's perspective
2. **Detailed Descriptions**: Include when to use, what it returns
3. **Intuitive Schemas**: Self-explanatory parameter names
4. **Examples**: Few-shot examples in descriptions
5. **Error Guidance**: How to handle failures

### Context Management ("Context Rot")

Strategies to prevent agents losing track over long interactions:

1. **Summarization**: Periodically summarize past interactions
2. **Structured Notes**: Running notes in consistent format
3. **Context Compaction**: Compress when approaching limits
4. **Memory Grounding**: Anchor responses in retrieved memories

### Subagent Architecture

For complex tasks, spawn specialized subagents:

- Each subagent has its own context window (fresh slate)
- Specialized prompts and tools per subagent
- Main agent orchestrates and synthesizes
- Preserves main conversation focus

---

## Google Insights: Agent Development Kit & Multi-Agent Systems

### Core Philosophy

> "Build modular, composable agents with rich tool ecosystems and built-in observability."

Google emphasizes code-first development with pre-built components that can be combined flexibly.

### Agent Composition Patterns

#### Hierarchical Agents

Parent agents delegate to children based on capability.

```
       Root Agent
      /    |    \
  Child₁ Child₂ Child₃
```

#### Peer Agents

Agents collaborate as equals, each with specialized capabilities.

```
Agent₁ ←→ Agent₂ ←→ Agent₃
```

#### Routing Strategies

- **Intent-Based**: Route by classified intent
- **Capability-Based**: Route by matching capabilities
- **LLM-Decision**: Let LLM decide delegation
- **Round-Robin**: Load balancing

### Tool Ecosystem

- **Pre-built Tools**: Common capabilities out of the box
- **Custom Functions**: Easy tool definition
- **Third-party Integration**: Connect to external services
- **Discovery**: Semantic search over tool descriptions

### Observability

Built-in tracing and monitoring:

- **Execution Traces**: Full trace of agent actions
- **Metrics Collection**: Performance, cost, latency
- **Error Tracking**: Detailed error information
- **Replay**: Re-run past executions

### Session Management

- **Session State**: Track conversation sessions
- **State Persistence**: Save/restore state
- **Context Windowing**: Manage context limits
- **Turn Tracking**: Maintain history

---

## Protocol Standards

### Model Context Protocol (MCP) - Anthropic

MCP standardizes LLM ↔ External System connections:

| Component | Purpose |
|-----------|---------|
| **Resources** | Read-only data sources (files, DBs) |
| **Tools** | Functions the LLM can invoke |
| **Prompts** | Templated prompts with parameters |
| **Sampling** | LLM completion requests |

**Critical for Thymos**: MCP server lets Claude use Thymos agents as tools.

### Agent-to-Agent Protocol (A2A) - Google

A2A standardizes agent ↔ agent communication:

- Agent discovery by capability
- Task delegation
- Structured result exchange
- Error propagation

---

## Leveraging Memory Versioning: Thymos's Unique Advantage

Thymos's git-inspired memory versioning (branches, commits, worktrees) is **unique in the agent framework ecosystem** and maps directly to critical patterns that Anthropic and Google stress. This section shows how to leverage this existing capability.

### Memory Versioning → Anthropic/Google Pattern Mapping

| Pattern | Problem | Memory Versioning Solution |
|---------|---------|---------------------------|
| Context Anti-Rot | Context windows get polluted | **Commits as checkpoints** - rollback to clean state |
| Subagent Architecture | Subagents need isolated context | **Worktrees** - each subagent gets isolated memory |
| Evaluator-Optimizer | Need to compare attempts | **Branches per attempt** - evaluate and merge best |
| Parallel Execution | Concurrent agents conflict | **Worktrees** - isolated memory per parallel path |
| Safe Experimentation | Fear of breaking production | **Branches** - experiment without risk |
| Session Resume | Need to restore exact state | **Commits** - checkpoint and restore |

### Pattern 1: Context Anti-Rot via Commits

**Anthropic's Concern**: Context windows get polluted over long conversations, causing "context rot."

**Solution**: Use commits as checkpoints. Before risky operations, commit memory state. If context degrades, checkout to restore.

```rust
// Before a risky/long conversation
let checkpoint = repo.commit("Pre-exploration checkpoint", agent_id).await?;

// Run potentially context-polluting operations
agent.process_long_conversation(&conversation).await?;

// If context has degraded (LLM detects confusion, contradictions)
if context_quality_degraded(&agent).await? {
    // Rollback to clean state
    repo.checkout_commit(&checkpoint, &mut agent, None).await?;
    
    // Try again with different approach
    agent.process_with_summarization(&conversation).await?;
}
```

**Advanced: Branch Per Session**

```rust
// Each conversation gets its own branch
let session_branch = format!("session-{}", session_id);
repo.create_branch(&session_branch, Some("Conversation session"), None).await?;
repo.checkout(&session_branch, &mut agent).await?;

// Conversation runs in isolated branch
agent.converse(&messages).await?;

// Only merge valuable insights back to main, leaving noise behind
let insights = extract_valuable_insights(&agent).await?;
repo.checkout("main", &mut agent).await?;
for insight in insights {
    agent.remember(&insight).await?;
}
repo.commit("Merged insights from session", agent_id).await?;

// Discard session branch (with all the noise)
repo.delete_branch(&session_branch, true).await?;
```

### Pattern 2: Subagent Architecture via Worktrees

**Anthropic's Pattern**: Spawn subagents with fresh context windows, specialized tools, synthesize results back.

**Solution**: Worktrees provide exactly this - each subagent gets an isolated memory copy.

```rust
/// Subagent with isolated memory worktree
pub struct VersionedSubagent {
    pub definition: Subagent,
    pub worktree_id: String,
    pub worktree_manager: Arc<MemoryWorktreeManager>,
}

impl VersionedSubagent {
    /// Spawn subagent with isolated memory from current branch
    pub async fn spawn(
        manager: Arc<MemoryWorktreeManager>,
        definition: Subagent,
    ) -> Result<Self> {
        // Create worktree - subagent gets copy of current memory
        let worktree_id = manager.create_worktree(
            "main",  // or current branch
            Some(&format!("subagent-{}", definition.name)),
        ).await?;
        
        Ok(Self {
            definition,
            worktree_id,
            worktree_manager: manager,
        })
    }
    
    /// Execute subagent task in isolated memory
    pub async fn execute(&self, task: &str) -> Result<SubagentResult> {
        let agent = self.worktree_manager
            .get_worktree_agent(&self.worktree_id)
            .await?;
        
        // Subagent works in its own memory space
        // Can remember, search, modify without affecting main
        let result = agent.execute_task(task).await?;
        
        Ok(result)
    }
    
    /// Merge valuable discoveries back to main
    pub async fn merge_discoveries(
        &self,
        discoveries: Vec<String>,
        main_agent: &mut Agent,
    ) -> Result<()> {
        // Commit subagent's changes
        self.worktree_manager.commit_worktree_changes(
            &self.worktree_id,
            &format!("Subagent {} discoveries", self.definition.name),
        ).await?;
        
        // Merge only the good stuff
        // Option 1: Full merge
        // repo.merge(&subagent_branch, "main", main_agent, MergeStrategy::AutoMerge).await?;
        
        // Option 2: Cherry-pick specific memories (more selective)
        for discovery in discoveries {
            main_agent.remember(&discovery).await?;
        }
        
        Ok(())
    }
    
    /// Discard subagent without affecting main memory
    pub async fn discard(self) -> Result<()> {
        // All memory changes are lost - main is untouched
        self.worktree_manager.remove_worktree(&self.worktree_id, true).await?;
        Ok(())
    }
}

// Usage in Orchestrator-Workers pattern
async fn orchestrate_research(
    main_agent: &mut Agent,
    worktree_manager: Arc<MemoryWorktreeManager>,
    research_topics: Vec<&str>,
) -> Result<String> {
    let mut subagents = Vec::new();
    
    // Spawn worker subagent for each topic (parallel worktrees)
    for topic in &research_topics {
        let subagent = VersionedSubagent::spawn(
            worktree_manager.clone(),
            Subagent {
                name: format!("researcher-{}", topic),
                purpose: format!("Research {}", topic),
                system_prompt: format!("You are an expert on {}.", topic),
                tools: vec![/* research tools */],
                memory_scope: None,
            },
        ).await?;
        subagents.push((topic, subagent));
    }
    
    // Execute all subagents in parallel (each has isolated memory)
    let results = futures::future::join_all(
        subagents.iter().map(|(topic, sa)| {
            sa.execute(&format!("Research key facts about {}", topic))
        })
    ).await;
    
    // Merge discoveries from successful subagents
    for ((topic, subagent), result) in subagents.into_iter().zip(results) {
        match result {
            Ok(res) => {
                subagent.merge_discoveries(res.discoveries, main_agent).await?;
            }
            Err(_) => {
                // Failed subagent - discard without polluting main
                subagent.discard().await?;
            }
        }
    }
    
    // Synthesize final result from main agent (now has all discoveries)
    main_agent.generate_synthesis().await
}
```

### Pattern 3: Evaluator-Optimizer via Branches

**Anthropic's Pattern**: Generate → Evaluate → Refine loop.

**Solution**: Each generation attempt gets its own branch. Compare and merge the best.

```rust
/// Evaluator-Optimizer with branch-based versioning
pub struct BranchingEvaluatorOptimizer {
    repo: Arc<MemoryRepository>,
    worktree_manager: Arc<MemoryWorktreeManager>,
    evaluator: Arc<dyn Evaluator>,
    max_iterations: usize,
}

impl BranchingEvaluatorOptimizer {
    pub async fn optimize(
        &self,
        initial_agent: &Agent,
        goal: &str,
    ) -> Result<OptimizationResult> {
        let mut best_score = 0.0;
        let mut best_branch = "main".to_string();
        
        for i in 0..self.max_iterations {
            // Create branch for this attempt
            let attempt_branch = format!("optimize-attempt-{}", i);
            self.repo.create_branch(
                &attempt_branch,
                Some(&format!("Optimization attempt {}", i)),
                Some(&best_branch),  // Branch from current best
            ).await?;
            
            // Create worktree for parallel evaluation
            let worktree_id = self.worktree_manager
                .create_worktree(&attempt_branch, None)
                .await?;
            let attempt_agent = self.worktree_manager
                .get_worktree_agent(&worktree_id)
                .await?;
            
            // Generate attempt
            let output = attempt_agent.generate(goal).await?;
            
            // Evaluate
            let score = self.evaluator.evaluate(&output).await?;
            
            if score > best_score {
                // This attempt is better - keep it
                self.worktree_manager.commit_worktree_changes(
                    &worktree_id,
                    &format!("Attempt {} - score {}", i, score),
                ).await?;
                best_score = score;
                best_branch = attempt_branch;
            } else {
                // This attempt didn't improve - discard
                self.worktree_manager.remove_worktree(&worktree_id, true).await?;
                self.repo.delete_branch(&attempt_branch, true).await?;
            }
            
            // Early exit if good enough
            if score >= 0.95 {
                break;
            }
        }
        
        // Merge best branch back to main
        let mut main_agent = initial_agent.clone();
        self.repo.checkout("main", &mut main_agent).await?;
        self.repo.merge(
            &best_branch,
            "main",
            &mut main_agent,
            MergeStrategy::Theirs,  // Take all changes from best
        ).await?;
        
        Ok(OptimizationResult {
            best_score,
            iterations: self.max_iterations,
        })
    }
}
```

### Pattern 4: Parallel Execution via Worktrees

**Anthropic's Pattern**: Multiple LLM calls simultaneously, aggregate results.

**Solution**: Worktrees provide isolation so parallel agents don't conflict.

```rust
/// Parallel execution with memory isolation
pub async fn parallel_with_voting<T: Clone>(
    worktree_manager: Arc<MemoryWorktreeManager>,
    main_branch: &str,
    task: &str,
    num_attempts: usize,
    aggregator: impl Fn(Vec<T>) -> T,
) -> Result<T> {
    // Create worktree for each parallel attempt
    let worktree_ids: Vec<String> = futures::future::try_join_all(
        (0..num_attempts).map(|i| {
            let wm = worktree_manager.clone();
            let branch = main_branch.to_string();
            async move {
                wm.create_worktree(&branch, Some(&format!("parallel-{}", i))).await
            }
        })
    ).await?;
    
    // Execute in parallel (each has isolated memory)
    let results: Vec<T> = futures::future::try_join_all(
        worktree_ids.iter().map(|wt_id| {
            let wm = worktree_manager.clone();
            let task = task.to_string();
            let wt_id = wt_id.clone();
            async move {
                let agent = wm.get_worktree_agent(&wt_id).await?;
                agent.execute_and_parse::<T>(&task).await
            }
        })
    ).await?;
    
    // Aggregate results (e.g., voting, consensus)
    let final_result = aggregator(results);
    
    // Cleanup worktrees
    for wt_id in worktree_ids {
        worktree_manager.remove_worktree(&wt_id, true).await?;
    }
    
    Ok(final_result)
}

// Usage: Multi-perspective analysis
let perspectives = parallel_with_voting(
    worktree_manager,
    "main",
    "Analyze this situation from your perspective",
    3,
    |results| consensus_merge(results),
).await?;
```

### Pattern 5: Speculative Execution (Novel Pattern)

**Problem**: Want to try an approach but unsure if it will work.

**Solution**: Branch, execute speculatively, commit only if successful.

```rust
/// Speculative execution with automatic rollback
pub async fn speculate<F, T>(
    repo: &MemoryRepository,
    agent: &mut Agent,
    speculation_name: &str,
    action: F,
) -> Result<Option<T>>
where
    F: FnOnce(&mut Agent) -> Pin<Box<dyn Future<Output = Result<T>> + Send>>,
{
    // Create speculation branch
    let spec_branch = format!("speculative-{}", speculation_name);
    repo.create_branch(&spec_branch, Some("Speculative execution"), None).await?;
    repo.checkout(&spec_branch, agent).await?;
    
    // Execute speculatively
    match action(agent).await {
        Ok(result) => {
            // Success - commit and merge
            repo.commit(&format!("Successful: {}", speculation_name), &agent.id()).await?;
            repo.checkout("main", agent).await?;
            repo.merge(&spec_branch, "main", agent, MergeStrategy::Theirs).await?;
            repo.delete_branch(&spec_branch, true).await?;
            Ok(Some(result))
        }
        Err(_) => {
            // Failed - rollback, main is untouched
            repo.checkout("main", agent).await?;
            repo.delete_branch(&spec_branch, true).await?;
            Ok(None)
        }
    }
}

// Usage
let result = speculate(
    &repo,
    &mut agent,
    "risky-strategy",
    |agent| Box::pin(async move {
        agent.remember("Trying risky approach").await?;
        agent.execute_risky_plan().await
    }),
).await?;
```

### Pattern 6: Multi-Agent Consensus (Novel Pattern)

**Problem**: Multiple agents need to form a consensus.

**Solution**: Each agent forms conclusions in their own worktree, LLM-assisted merge finds consensus.

```rust
/// Multi-agent consensus via worktrees
pub async fn form_consensus(
    worktree_manager: Arc<MemoryWorktreeManager>,
    repo: Arc<MemoryRepository>,
    agents: Vec<AgentConfig>,
    topic: &str,
    llm: Arc<dyn LLMProvider>,
) -> Result<String> {
    let mut conclusions = Vec::new();
    
    // Each agent forms conclusions in their own worktree
    for agent_config in agents {
        let worktree_id = worktree_manager
            .create_worktree("main", Some(&agent_config.name))
            .await?;
        
        let agent = worktree_manager.get_worktree_agent(&worktree_id).await?;
        
        // Agent thinks independently
        agent.remember(&format!("My perspective on {}: ...", topic)).await?;
        let conclusion = agent.generate(&format!(
            "What is your conclusion about {}?",
            topic
        )).await?;
        
        conclusions.push((agent_config.name, conclusion));
        
        // Commit conclusions
        worktree_manager.commit_worktree_changes(
            &worktree_id,
            &format!("{}'s conclusions", agent_config.name),
        ).await?;
    }
    
    // LLM-assisted consensus merge
    let consensus = llm.generate(&LLMRequest {
        prompt: format!(
            "Synthesize these perspectives into a consensus:\n\n{}",
            conclusions.iter()
                .map(|(name, c)| format!("{}: {}", name, c))
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
        ..Default::default()
    }).await?;
    
    Ok(consensus.content)
}
```

### Pattern 7: Time-Travel Debugging (Novel Pattern)

**Problem**: Agent behavior changed unexpectedly. When did it break?

**Solution**: Binary search through commits to find where behavior changed.

```rust
/// Binary search through memory history to find regression
pub async fn bisect_regression(
    repo: &MemoryRepository,
    agent: &mut Agent,
    test: impl Fn(&Agent) -> bool,
    good_commit: &str,
    bad_commit: &str,
) -> Result<String> {
    let commits = repo.get_commits_between(good_commit, bad_commit).await?;
    let mut low = 0;
    let mut high = commits.len() - 1;
    
    while low < high {
        let mid = (low + high) / 2;
        let commit = &commits[mid];
        
        // Checkout this commit
        repo.checkout_commit(&commit.hash, agent, None).await?;
        
        // Test behavior
        if test(agent) {
            // Still good - problem is later
            low = mid + 1;
        } else {
            // Broken - problem is earlier or here
            high = mid;
        }
    }
    
    Ok(commits[low].hash.clone())
}

// Usage: Find when agent started giving wrong answers
let first_bad = bisect_regression(
    &repo,
    &mut agent,
    |agent| agent.answers_correctly("What is 2+2?"),
    "abc123",  // Known good commit
    "def456",  // Known bad commit
).await?;
println!("Regression introduced in commit: {}", first_bad);
```

### Integration with Context Manager

Memory versioning integrates naturally with context management:

```rust
pub struct VersionedContextManager {
    repo: Arc<MemoryRepository>,
    context_manager: ContextManager,
    checkpoint_interval: usize,
    turns_since_checkpoint: usize,
}

impl VersionedContextManager {
    /// Process turn with automatic checkpointing
    pub async fn process_turn(
        &mut self,
        agent: &mut Agent,
        input: &str,
    ) -> Result<String> {
        self.turns_since_checkpoint += 1;
        
        // Periodic checkpoint
        if self.turns_since_checkpoint >= self.checkpoint_interval {
            self.repo.commit(
                &format!("Checkpoint at turn {}", self.turns_since_checkpoint),
                &agent.id(),
            ).await?;
            self.turns_since_checkpoint = 0;
        }
        
        // Process with context management
        let response = self.context_manager.process(agent, input).await?;
        
        // Check for context degradation
        if self.context_manager.context_quality() < 0.7 {
            // Restore from last checkpoint
            let last_checkpoint = self.repo.get_last_commit().await?;
            self.repo.checkout_commit(&last_checkpoint.hash, agent, None).await?;
            
            // Compact context and retry
            self.context_manager.compact(agent.llm()).await?;
        }
        
        Ok(response)
    }
}
```

### Summary: Memory Versioning as Competitive Advantage

| Capability | What It Enables | No Other Framework Has This |
|------------|-----------------|----------------------------|
| **Commits** | Checkpoint/restore, time-travel, audit trail | ✅ Unique |
| **Branches** | Safe experimentation, A/B testing, speculation | ✅ Unique |
| **Worktrees** | Parallel execution isolation, subagent memory | ✅ Unique |
| **Merge** | LLM-assisted conflict resolution, consensus | ✅ Unique |
| **Checkout** | Context rollback, regression bisect | ✅ Unique |

This is Thymos's **key differentiator**. The recommended architecture should expose these capabilities through high-level patterns that integrate naturally with the workflow patterns Anthropic and Google recommend.

---

## Recommended Architecture

### 1. Enhanced Tool System

```rust
/// Tool with LLM-friendly metadata
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: JsonSchema,
    
    /// When should the LLM use this tool?
    pub usage_hints: Vec<String>,
    
    /// Example invocations for few-shot learning
    pub examples: Vec<ToolExample>,
    
    /// What to do if the tool fails
    pub error_guidance: Option<String>,
    
    /// Return value description
    pub returns: String,
    
    pub handler: Arc<dyn ToolHandler>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult>;
    fn validate(&self, args: &Value) -> Result<()>;
}
```

#### Tool Runtime Safety (Capability + Policy)

Tools need a minimal “runtime contract” so they can be safely invoked by agents and exposed via MCP.

**Key requirements**:
- Tool permissioning (capabilities: filesystem, network, subprocess, secrets)
- Allow/deny policy per agent/skill/subagent
- Resource limits (timeout, memory, max bytes read/written)
- Rate limiting and concurrency limits
- Secrets injection + redaction (no secrets in traces, logs, or LLM-visible text unless explicit)

### 2. Skill Bundles

```rust
/// Skill = Related tools + context + prompts
pub struct Skill {
    pub name: String,
    pub description: String,
    pub tools: Vec<Tool>,
    pub memory_scope: Option<String>,
    pub prompts: HashMap<String, PromptTemplate>,
}

impl Agent {
    pub fn register_skill(&mut self, skill: Skill) -> Result<()>;
    pub fn available_skills(&self) -> Vec<&Skill>;
}
```

### 3. Workflow Patterns

```rust
pub enum WorkflowPattern {
    /// Sequential steps
    Chain(Vec<WorkflowStep>),
    
    /// Route by classification
    Router {
        classifier: Arc<dyn Classifier>,
        routes: HashMap<String, Arc<dyn Handler>>,
    },
    
    /// Parallel execution
    Parallel {
        branches: Vec<WorkflowStep>,
        aggregator: Arc<dyn Aggregator>,
    },
    
    /// Orchestrator delegates
    Orchestrator {
        planner: Arc<dyn Planner>,
        workers: HashMap<String, Arc<Agent>>,
    },
    
    /// Iterative refinement
    EvaluatorOptimizer {
        generator: Arc<dyn Generator>,
        evaluator: Arc<dyn Evaluator>,
        max_iterations: usize,
    },
}
```

#### Workflow Execution Semantics (Failure, Cancellation, Partial Results)

Workflows need explicit semantics so behavior is predictable and testable:
- A shared error taxonomy (retryable vs fatal vs cancelled vs partial)
- Cancellation propagation (parent cancels children; timeouts end the branch)
- Aggregation rules for partial failures (Parallel/Voting)
- Idempotency guidance for tool calls used in retry loops
- Structured concurrency (bounded fan-out; backpressure)

### 4. Context Manager

```rust
pub struct ContextManager {
    max_tokens: usize,
    summary: Option<String>,
    notes: Vec<ContextNote>,
    memory: Arc<MemorySystem>,
}

impl ContextManager {
    /// Compact context when approaching limits
    pub async fn compact(&mut self, llm: &dyn LLMProvider) -> Result<()>;
    
    /// Ground response in memory
    pub async fn ground(&self, response: &str, query: &str) -> Result<String>;
    
    /// Add structured note
    pub fn note(&mut self, note: ContextNote);
}
```

#### Context Budgeting & Retrieval Policy

Context management needs an explicit token budget model:
- Prompt vs notes vs retrieved memories vs tool outputs are separately budgeted
- Retrieval policy is configurable (top-k, diversity, recency/importance weighting)
- Summarization cadence and rollback triggers are deterministic and testable

### 5. Subagent Support

```rust
pub struct Subagent {
    pub name: String,
    pub purpose: String,
    pub system_prompt: String,
    pub tools: Vec<Tool>,
    pub memory_scope: Option<String>,
}

impl Agent {
    pub async fn spawn_subagent(
        &self, 
        subagent: &Subagent, 
        task: &str
    ) -> Result<SubagentResult>;
}
```

#### Subagents as Capability-Scoped Worktrees

Subagents should default to reduced permissions compared to the main agent, even if they inherit memory via worktrees. This makes the orchestrator-workers pattern safer by default.

### 6. MCP Server

```rust
pub struct ThymosMcpServer {
    agents: HashMap<String, Arc<Agent>>,
}

impl McpServer for ThymosMcpServer {
    async fn list_tools(&self) -> Vec<McpTool>;
    async fn list_resources(&self) -> Vec<McpResource>;
    async fn call_tool(&self, name: &str, args: Value) -> McpResult;
    async fn read_resource(&self, uri: &str) -> McpResource;
}
```

#### MCP Design Requirements (Interoperability)

Minimum details to specify up-front:
- Transport support (at least stdio; optionally HTTP later)
- Authentication/authorization when not using stdio (API keys, tokens, mTLS)
- Session model mapping (stateless calls vs server-managed session handles)
- Tool/resource naming + versioning strategy
- Pagination and filtering for large memory/resource sets
- Safety: which tools are exposed over MCP by default (ideally none without explicit allowlist)

### 7. Execution Tracing

```rust
pub struct AgentTracer {
    traces: Vec<ExecutionTrace>,
    metrics: MetricsCollector,
}

#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub action: TraceAction,
    pub duration: Duration,
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub children: Vec<ExecutionTrace>,
}

#[derive(Debug, Clone)]
pub enum TraceAction {
    LLMCall { model: String, tokens_in: usize, tokens_out: usize },
    ToolCall { tool_name: String },
    MemorySearch { query: String, results: usize },
    SubagentSpawn { subagent_name: String },
}
```

#### Prefer OpenTelemetry-Compatible Tracing

Thymos tracing should map cleanly to OpenTelemetry concepts:
- Traces/spans for LLM calls, tool calls, memory searches, merges/checkouts, subagent runs
- Attributes for model name, token counts, branch/worktree IDs, commit hashes, tool capabilities
- An export path (OTLP) so existing tracing UIs can be used immediately

### 8. Reproducibility, Replay, and Evaluation Harness (New)

Production agent platforms need a way to reproduce and evaluate behavior over time.

**Replay record** should capture:
- Model config (provider, model, temperature, max tokens)
- Prompts/templates + resolved parameters
- Tool invocations (name, args, outputs, errors, durations)
- Memory retrieval provenance (query, result IDs/hashes, ranking inputs)
- Versioning events (branch/worktree IDs, commit hashes, merge results)

**Evaluation harness** should support:
- Offline runs (stubbed tools + canned memory) for determinism
- Golden tests for workflows (inputs → expected structured outputs)
- Regression detection (prompt/template changes, tool schema changes)

---

## Implementation Phases

### Phase 0: Safety + Replay/Eval Foundations (1-2 weeks)

**Goal**: Make tools/workflows safe and testable before expanding surface area.

- Tool runtime permissions and policy enforcement
- Unified error model with retry/cancel semantics
- Replay record format + basic replay runner (offline/stubbed tools)
- Minimal evaluation harness (golden tests for workflows and parsers)

### Phase 1: Tool & Skill Foundation (4-6 weeks)

**Goal**: First-class tool/skill abstractions

- Enhanced `Tool` struct with LLM-friendly metadata
- `ToolExample` for few-shot learning
- `ToolRegistry` with semantic discovery
- `Skill` bundles (tools + memory scope + prompts)
- Built-in tools (memory_search, memory_store)

### Phase 2: Workflow Patterns (4-6 weeks)

**Goal**: Structured workflow execution

- `WorkflowPattern` enum with all five patterns
- Chain execution with gates
- Router with semantic classification
- Parallel execution with aggregation
- Orchestrator-worker delegation

### Phase 3: MCP Integration (3-4 weeks)

**Goal**: Claude ecosystem integration

- MCP protocol implementation
- Expose tools as MCP tools
- Expose memories as MCP resources
- Expose prompts as MCP prompts
- Integration tests with Claude

### Phase 4: Context & Subagents (4-6 weeks)

**Goal**: Advanced context management

- `ContextManager` with anti-rot strategies
- Automatic summarization
- Memory grounding
- `Subagent` definition and execution
- Result synthesis

### Phase 5: Observability (3-4 weeks)

**Goal**: Production debugging

- `ExecutionTracer` with full traces
- Metrics collection (tokens, cost, latency)
- Error tracking with context
- Trace visualization/export

---

## Priority Matrix

| Feature | Priority | Effort | Impact |
|---------|----------|--------|--------|
| Tool System | 🔴 Critical | Medium | High |
| Skill Bundles | 🔴 Critical | Low | High |
| MCP Server | 🔴 Critical | Medium | High |
| Workflow Patterns | 🔴 Critical | High | High |
| Structured Output | 🟡 High | Low | Medium |
| Context Management | 🟡 High | Medium | High |
| Subagents | 🟡 High | Medium | High |
| Execution Tracing | 🟡 High | Medium | Medium |
| A2A Protocol | 🟢 Medium | Medium | Medium |
| Visual Editor | 🟢 Low | High | Low |

---

## Key Takeaways

### From Anthropic

1. **Start simple** - Add complexity only when needed
2. **Tools matter** - Design carefully from LLM perspective
3. **Context management** - Actively prevent context rot
4. **Patterns work** - Use established workflow patterns
5. **MCP is standard** - Integrate with Claude ecosystem

### From Google

1. **Modular design** - Composable agents are maintainable
2. **Rich tools** - Pre-built + custom ecosystem
3. **Observability first** - Built-in tracing and metrics
4. **Multi-agent** - Agents that collaborate
5. **Sessions** - Robust conversation state

### For Thymos

1. **Leverage memory versioning** - Branches/worktrees enable patterns others can't do ⭐ **UNIQUE**
2. **Expose versioning as patterns** - VersionedSubagent, SpeculativeExecution, ConsensusMerge
3. **Add workflow patterns** - Chain/Route/Parallel built on versioning primitives
4. **Implement MCP** - Critical for ecosystem integration
5. **Build tools** - Registry + discovery + built-ins
6. **Add observability** - Tracing for debugging (with commit history integration)

---

## Success Criteria

### Functional

- [ ] Agents can register and use tools/skills
- [ ] Workflow patterns execute correctly
- [ ] MCP server works with Claude
- [ ] Context doesn't rot over long sessions (leveraging commits/rollback)
- [ ] Subagents can be spawned and managed (leveraging worktrees)
- [ ] Full execution traces available (with commit correlation)

### Memory Versioning Integration

- [ ] VersionedSubagent spawns with isolated worktree
- [ ] SpeculativeExecution commits only on success
- [ ] Parallel workflows use worktrees for isolation
- [ ] Context rollback via checkout works correctly
- [ ] Bisect debugging finds regression commits
- [ ] LLM-assisted merge resolves conflicts

### Performance

- [ ] Tool execution < 100ms overhead
- [ ] MCP response < 200ms
- [ ] Memory grounding < 50ms
- [ ] Context compaction < 500ms

### Developer Experience

- [ ] Define tool in < 20 lines
- [ ] Define skill in < 50 lines
- [ ] Set up MCP server in < 10 lines
- [ ] Debug with traces in < 5 minutes

---

## Acceptance Checklists

These checklists are intended to gate “done” for roadmap features. They should remain measurable and testable.

### Phase 0 Acceptance (Safety + Replay/Eval Foundations)

- [ ] Tool calls enforce timeouts and propagate cancellation
- [ ] Tool calls run under a capability policy (deny-by-default for privileged capabilities)
- [ ] Secrets can be injected for tools and are redacted from traces/logs by default
- [ ] Tool results use a structured envelope (success/error/cancelled + warnings + provenance)
- [ ] Unified error taxonomy exists and is used consistently across tool + workflow layers
- [ ] Replay record captures model config, resolved prompts, tool I/O, retrieval provenance, and versioning events
- [ ] Offline evaluation mode exists (stubbed tools + deterministic memory snapshots)
- [ ] At least one golden test per workflow pattern exists and runs without network access

### Phase 1 Acceptance (Tool & Skill System)

- [ ] Tools are discoverable (registry) by name + semantic search over descriptions
- [ ] Skill bundles can scope tools + memory access + permissions as a unit
- [ ] Tool schema validation occurs before execution; invalid args produce structured errors
- [ ] Built-in memory tools (search/store) emit provenance metadata suitable for tracing/replay

### Phase 2 Acceptance (Workflow Patterns)

- [ ] Each workflow pattern has defined behavior for partial failure and cancellation
- [ ] Parallel workflows are bounded (max fan-out, max concurrency) with backpressure
- [ ] Evaluator-optimizer can compare attempts using replay records (not only ad-hoc logging)

### Phase 3 Acceptance (MCP)

- [ ] MCP server exposes only allowlisted tools by default
- [ ] MCP calls can operate statelessly or via explicit session handle (documented)
- [ ] MCP resources support pagination for large memory sets

### Phase 4/5 Acceptance (Context/Subagents/Observability)

- [ ] Context budgeting rules are configurable and test-covered
- [ ] Subagents run with isolated worktrees and reduced permissions by default
- [ ] Traces export to an OpenTelemetry-compatible sink (or a clearly mappable intermediate format)

---

## References

- [Anthropic: Building Effective Agents](https://www.anthropic.com/research/building-effective-agents)
- [Anthropic: Model Context Protocol](https://modelcontextprotocol.io)
- [Anthropic: Claude Subagents](https://docs.anthropic.com/en/docs/claude-code/sub-agents)
- [Google: Agent Development Kit](https://github.com/google/adk-docs)
- [Google: Multi-Agent AI Systems](https://cloud.google.com/architecture/multiagent-ai-system)
- [Google: Agent Designer](https://cloud.google.com/agentspace/docs/agent-designer)


