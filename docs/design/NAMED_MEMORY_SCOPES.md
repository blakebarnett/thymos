# Named Memory Scopes Design

**Status**: In Progress  
**Priority**: Critical  
**Affects**: `thymos-core/src/memory/`

## Overview

Extend Thymos memory system to support named scopes with configurable behavior, replacing the binary Private/Shared model with a flexible, multi-scope architecture.

## Current State

```rust
pub enum MemoryScope {
    Private,  // Embedded backend
    Shared,   // Server backend
}
```

This is limited to backend selection, not semantic categorization.

## Proposed Design

### Core Types

```rust
/// Named scope with configurable behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScopeConfig {
    /// Scope name (unique identifier)
    pub name: String,

    /// Decay rate in hours (for forgetting curve)
    /// Lower = faster decay
    pub decay_hours: f64,

    /// Importance multiplier for this scope's memories
    /// Higher = more resistant to forgetting
    pub importance_multiplier: f64,

    /// Weight when searching across scopes (0.0-1.0)
    /// Higher = prioritized in merged results
    pub search_weight: f64,

    /// Optional: Max memories before pruning oldest
    pub max_memories: Option<usize>,
}

impl Default for MemoryScopeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            decay_hours: 168.0,  // 1 week
            importance_multiplier: 1.0,
            search_weight: 1.0,
            max_memories: None,
        }
    }
}

/// Scope registry within MemorySystem
pub struct ScopeRegistry {
    scopes: HashMap<String, MemoryScopeConfig>,
}
```

### API Extensions

```rust
impl MemorySystem {
    /// Define a named scope with configuration
    pub fn define_scope(&mut self, config: MemoryScopeConfig) -> Result<()>;

    /// Get scope configuration
    pub fn get_scope(&self, name: &str) -> Option<&MemoryScopeConfig>;

    /// List all defined scopes
    pub fn list_scopes(&self) -> Vec<&MemoryScopeConfig>;

    /// Remember in a specific scope
    pub async fn remember_in_scope(
        &self,
        scope: &str,
        content: &str,
        options: Option<RememberOptions>,
    ) -> Result<String>;

    /// Search within a specific scope
    pub async fn search_in_scope(
        &self,
        scope: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>>;

    /// Search across multiple scopes with weighted merging
    pub async fn search_scopes(
        &self,
        scopes: &[&str],
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>>;

    /// Search all defined scopes
    pub async fn search_all_scopes(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>>;
}
```

### Storage Strategy

Scope is stored in memory metadata/tags:

```rust
// When storing a memory in scope "personal"
let options = RememberOptions::new()
    .with_tag("_scope:personal");

memory.remember_with_options(content, options).await?;
```

This allows:
- Filtering by scope in search queries
- Scope-aware decay calculation
- No schema changes to underlying storage

### Decay Integration

Modify `MemoryLifecycle.calculate_strength()`:

```rust
pub fn calculate_strength(&self, memory: &Memory, scope_config: Option<&MemoryScopeConfig>) -> f64 {
    let config = scope_config.unwrap_or(&DEFAULT_SCOPE);
    
    let hours_since_access = self.hours_since_access(memory);
    let stability = config.decay_hours * config.importance_multiplier;
    
    // Forgetting curve: R = e^(-t/S)
    (-hours_since_access / stability).exp()
}
```

### Search Merging

When searching multiple scopes:

```rust
pub async fn search_scopes(&self, scopes: &[&str], query: &str, limit: usize) -> Result<Vec<ScoredMemory>> {
    let mut all_results = Vec::new();
    
    for scope_name in scopes {
        let scope = self.get_scope(scope_name)?;
        let results = self.search_in_scope(scope_name, query, limit).await?;
        
        // Apply scope weight to scores
        for memory in results {
            all_results.push(ScoredMemory {
                memory,
                score: memory.score * scope.search_weight,
                scope: scope_name.to_string(),
            });
        }
    }
    
    // Sort by weighted score and take top `limit`
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    all_results.truncate(limit);
    
    Ok(all_results)
}
```

## Backward Compatibility

- Default scope "default" with current behavior
- `remember()` without scope uses "default"
- `search()` without scope searches "default"
- Existing memories without scope tag treated as "default"

## Example Usage

```rust
let mut agent = Agent::builder()
    .id("tla")
    .build()
    .await?;

// Define scopes
agent.memory().define_scope(MemoryScopeConfig {
    name: "personal".to_string(),
    decay_hours: 336.0,  // 2 weeks
    importance_multiplier: 1.0,
    search_weight: 1.0,
    max_memories: None,
})?;

agent.memory().define_scope(MemoryScopeConfig {
    name: "observations".to_string(),
    decay_hours: 24.0,   // 1 day
    importance_multiplier: 0.5,
    search_weight: 0.3,
    max_memories: Some(1000),
})?;

// Store in scope
agent.memory().remember_in_scope(
    "personal",
    "Decided to use Redis for caching",
    None,
).await?;

// Search specific scope
let results = agent.memory().search_in_scope(
    "personal",
    "caching decision",
    10,
).await?;

// Search across scopes
let merged = agent.memory().search_scopes(
    &["personal", "team"],
    "project status",
    10,
).await?;
```

## Implementation Phases

### Phase 1: Core Types and Registry
- Add `MemoryScopeConfig` to `memory/scope.rs`
- Add `ScopeRegistry` to `MemorySystem`
- Implement `define_scope()`, `get_scope()`, `list_scopes()`

### Phase 2: Scoped Storage
- Add `remember_in_scope()` using tag-based storage
- Implement scope tag extraction from metadata

### Phase 3: Scoped Search
- Add `search_in_scope()` with scope filtering
- Add `search_scopes()` with weighted merging
- Add `search_all_scopes()`

### Phase 4: Decay Integration
- Modify `MemoryLifecycle` to accept scope config
- Apply scope-specific decay rates

### Phase 5: Agent API
- Add convenience methods to `Agent`
- Update documentation

## Testing Strategy

1. Unit tests for `MemoryScopeConfig` and `ScopeRegistry`
2. Integration tests for scoped remember/search
3. Decay calculation tests with different scope configs
4. Backward compatibility tests (no scope = default behavior)
