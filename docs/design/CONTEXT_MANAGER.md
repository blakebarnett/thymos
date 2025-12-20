# Context Manager Design

**Status**: Implemented  
**Priority**: High  
**Affects**: New module `thymos-core/src/context/`

## Overview

A high-level API that integrates conversation session management, memory grounding, automatic summarization, and version-based rollback to prevent "context rot" in long-running agents.

## Problem Statement

Long-running agents accumulate noise in their context window:
- Old, irrelevant messages consume token budget
- Response quality degrades over time
- No automatic recovery mechanism

Existing building blocks:
- `ConversationSession` - manages history
- `TruncationStrategy` - handles overflow (sliding window, summary)
- `MemoryRepository` - enables commit/rollback

No integrated solution combines these for automated context management.

## Proposed Design

### Core Types

```rust
/// High-level context manager
pub struct ContextManager {
    session: ConversationSession,
    memory: Arc<MemorySystem>,
    repo: Option<Arc<MemoryRepository>>,
    llm: Arc<dyn LLMProvider>,
    config: ContextConfig,

    // State
    turns_since_checkpoint: usize,
    last_quality_score: f64,
    checkpoint_commits: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum context tokens before compaction
    pub max_tokens: usize,

    /// Ratio at which to trigger summarization (e.g., 0.8 = 80%)
    pub summarize_at_ratio: f64,

    /// Quality threshold below which to rollback
    pub quality_threshold: f64,

    /// Turns between automatic checkpoints
    pub checkpoint_interval: usize,

    /// Number of recent memories to ground responses with
    pub grounding_memories: usize,

    /// Number of recent turns to keep verbatim after summarization
    pub recent_turns_to_keep: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8192,
            summarize_at_ratio: 0.8,
            quality_threshold: 0.6,
            checkpoint_interval: 10,
            grounding_memories: 5,
            recent_turns_to_keep: 3,
        }
    }
}
```

### Primary API

```rust
impl ContextManager {
    /// Create new context manager
    pub fn new(
        memory: Arc<MemorySystem>,
        llm: Arc<dyn LLMProvider>,
        config: ContextConfig,
    ) -> Self;

    /// Create with versioning support (enables rollback)
    pub fn with_versioning(
        memory: Arc<MemorySystem>,
        repo: Arc<MemoryRepository>,
        llm: Arc<dyn LLMProvider>,
        config: ContextConfig,
    ) -> Self;

    /// Process a turn with automatic context management
    pub async fn process_turn(
        &mut self,
        input: &str,
    ) -> Result<ContextTurnResult>;

    /// Ground a query with relevant memories
    pub async fn ground_query(
        &self,
        query: &str,
    ) -> Result<GroundedContext>;

    /// Get messages formatted for LLM context window
    pub fn get_context_messages(&self) -> Vec<Message>;

    /// Get estimated token count
    pub fn estimated_tokens(&self) -> usize;

    /// Get context quality score (0.0-1.0)
    pub fn quality_score(&self) -> f64;

    /// Force summarization of older turns
    pub async fn summarize(&mut self) -> Result<SummarizationResult>;

    /// Create checkpoint (if versioning enabled)
    pub async fn checkpoint(&mut self, message: &str) -> Result<Option<String>>;

    /// Rollback to last checkpoint (if versioning enabled)
    pub async fn rollback(&mut self) -> Result<RollbackResult>;
}
```

### Result Types

```rust
#[derive(Debug)]
pub struct ContextTurnResult {
    pub grounding_used: Vec<String>,  // Memory IDs used for grounding
    pub summarization_triggered: bool,
    pub checkpoint_created: Option<String>,
    pub quality_score: f64,
    pub token_count: usize,
}

#[derive(Debug)]
pub struct GroundedContext {
    pub memories: Vec<Memory>,
    pub summary: String,
    pub relevance_scores: Vec<f64>,
}

#[derive(Debug)]
pub struct SummarizationResult {
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub turns_summarized: usize,
    pub summary: String,
}

#[derive(Debug)]
pub struct RollbackResult {
    pub commit_restored: String,
    pub turns_lost: usize,
}
```

## Quality Scoring

Quality score based on multiple factors:

```rust
pub struct QualityScorer {
    /// Weight for memory retrieval relevance
    memory_relevance_weight: f64,
    /// Weight for response coherence
    coherence_weight: f64,
    /// Weight for session length ratio
    session_ratio_weight: f64,
}

impl QualityScorer {
    pub fn calculate(&self, ctx: &ContextState) -> f64 {
        let memory_score = ctx.avg_memory_relevance;
        let coherence_score = ctx.last_coherence_score;
        let session_score = 1.0 - (ctx.turn_count as f64 / ctx.summarization_count.max(1) as f64).min(1.0);

        (memory_score * self.memory_relevance_weight)
            + (coherence_score * self.coherence_weight)
            + (session_score * self.session_ratio_weight)
    }
}
```

For MVP, use heuristic scoring. LLM-based coherence evaluation can be added later.

## Summarization Flow

```rust
async fn summarize(&mut self) -> Result<SummarizationResult> {
    let turns = self.session.history().turns();
    let keep_recent = self.config.recent_turns_to_keep;
    
    if turns.len() <= keep_recent {
        return Ok(SummarizationResult::no_op());
    }
    
    // Get turns to summarize (exclude recent)
    let to_summarize = &turns[..turns.len() - keep_recent];
    
    // Generate summary via LLM
    let summary = self.generate_summary(to_summarize).await?;
    
    // Store summary as memory for future grounding
    self.memory.remember(format!(
        "[context-summary] {}",
        summary
    )).await?;
    
    // Update session with summarized history
    self.session.apply_summary(&summary, keep_recent);
    
    Ok(SummarizationResult { /* ... */ })
}
```

## Checkpoint/Rollback Flow

```rust
async fn checkpoint(&mut self, message: &str) -> Result<Option<String>> {
    let Some(repo) = &self.repo else {
        return Ok(None);
    };
    
    let commit = repo.commit(message, "context-manager", None).await?;
    self.checkpoint_commits.push(commit.clone());
    self.turns_since_checkpoint = 0;
    
    Ok(Some(commit))
}

async fn rollback(&mut self) -> Result<RollbackResult> {
    let Some(repo) = &self.repo else {
        return Err(ThymosError::Configuration("Versioning not enabled".into()));
    };
    
    let commit = self.checkpoint_commits.pop()
        .ok_or_else(|| ThymosError::Memory("No checkpoint to rollback to".into()))?;
    
    repo.checkout(&commit).await?;
    
    // Reset session state
    // (Implementation depends on how we want to restore session state)
    
    Ok(RollbackResult { commit_restored: commit, /* ... */ })
}
```

## Integration with Agent

```rust
impl Agent {
    /// Create a context manager for this agent
    pub fn context_manager(&self, config: ContextConfig) -> ContextManager {
        ContextManager::new(
            self.memory_arc(),
            self.llm_provider().expect("LLM required"),
            config,
        )
    }
    
    /// Create a context manager with versioning
    pub fn context_manager_with_versioning(
        &self,
        repo: Arc<MemoryRepository>,
        config: ContextConfig,
    ) -> ContextManager {
        ContextManager::with_versioning(
            self.memory_arc(),
            repo,
            self.llm_provider().expect("LLM required"),
            config,
        )
    }
}
```

## Example Usage

```rust
let agent = Agent::builder()
    .id("tla")
    .llm_provider(llm)
    .build()
    .await?;

let mut ctx = agent.context_manager(ContextConfig {
    max_tokens: 4096,
    summarize_at_ratio: 0.7,
    checkpoint_interval: 5,
    ..Default::default()
});

// Process turns with automatic management
loop {
    let input = get_user_input();
    
    let result = ctx.process_turn(&input).await?;
    
    if result.summarization_triggered {
        println!("Context summarized");
    }
    
    if result.quality_score < 0.5 {
        println!("Quality degraded, consider rollback");
    }
    
    // Get grounded context for LLM
    let messages = ctx.get_context_messages();
    let response = llm.complete(messages).await?;
    
    ctx.add_response(&response);
}
```

## Implementation Phases

### Phase 1: Module Structure
- Create `context/mod.rs`, `context/manager.rs`, `context/config.rs`
- Basic `ContextManager` with session integration

### Phase 2: Grounding
- Implement `ground_query()` with memory search
- Add grounding to `process_turn()`

### Phase 3: Summarization
- Implement LLM-powered summarization
- Token counting and threshold checking
- Summary storage as memory

### Phase 4: Quality Scoring
- Heuristic quality scorer
- Track quality over time

### Phase 5: Checkpointing
- Integrate with `MemoryRepository`
- Implement checkpoint/rollback

### Phase 6: Agent Integration
- Add convenience methods to `Agent`
- Documentation and examples

## Testing Strategy

1. Unit tests for `ContextConfig` validation
2. Integration tests for summarization flow
3. Integration tests for checkpoint/rollback
4. Quality scoring tests with mock data
5. Token counting accuracy tests
