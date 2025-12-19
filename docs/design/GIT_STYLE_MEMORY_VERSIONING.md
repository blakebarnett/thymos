# Git-Style Memory Versioning for Thymos

**Date**: December 2024  
**Status**: ✅ Implemented  
**Purpose**: Design a Git-like version control system for agent memories  
**Implementation**: `thymos-core/src/memory/versioning/`

## Executive Summary

By combining Locai memory versioning with Git-style operations, Thymos could enable powerful workflows:

- **Memory Branches**: Different memory timelines (like Git branches)
- **Checkout**: Switch between memory states (like `git checkout`)
- **Worktrees**: Concurrent agent instances with different memory states (like `git worktree`)
- **Commits**: Snapshot memory changes (like `git commit`)
- **Merging**: Combine memories from different branches (like `git merge`)
- **Rebasing**: Replay memories on different bases (like `git rebase`)

This would enable agents to explore alternative scenarios concurrently, test different strategies, and merge the best outcomes back into the main memory timeline.

---

## Core Concepts

### Memory Repository

A memory repository is like a Git repository - it contains:
- **Memory objects**: The actual memories (like Git blobs)
- **Commits**: Snapshots of memory state at points in time
- **Branches**: Named pointers to commits (different memory timelines)
- **HEAD**: Current branch/commit (active memory state)
- **Index**: Staging area for memory changes

### Memory Commits

Each commit represents a snapshot of memory state:

```rust
/// Memory commit (like Git commit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCommit {
    /// Commit hash (SHA-256 of commit content)
    pub hash: String,
    
    /// Parent commit(s) (for merge commits)
    pub parents: Vec<String>,
    
    /// Author (agent ID)
    pub author: String,
    
    /// Commit timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Commit message
    pub message: String,
    
    /// Memory changes in this commit
    pub changes: MemoryChanges,
    
    /// Tree hash (points to memory tree)
    pub tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryChanges {
    /// New memories added
    pub added: Vec<MemoryEntry>,
    
    /// Memories modified
    pub modified: Vec<MemoryModification>,
    
    /// Memories deleted
    pub deleted: Vec<String>,  // Memory IDs
    
    /// Concepts changed
    pub concepts_changed: Vec<ConceptChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryModification {
    pub memory_id: String,
    pub old_version: String,
    pub new_version: String,
    pub diff: MemoryDiff,
}
```

### Memory Branches

Branches represent different memory timelines:

```rust
/// Memory branch (like Git branch)
#[derive(Debug, Clone)]
pub struct MemoryBranch {
    /// Branch name
    pub name: String,
    
    /// Commit hash this branch points to
    pub commit: String,
    
    /// Branch description
    pub description: Option<String>,
    
    /// Whether this is the active branch
    pub is_active: bool,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Branch manager
pub struct MemoryBranchManager {
    branches: Arc<RwLock<HashMap<String, MemoryBranch>>>,
    current_branch: Arc<RwLock<String>>,
    commits: Arc<RwLock<HashMap<String, MemoryCommit>>>,
}
```

---

## Git-Like Operations

### 1. Memory Commits

```rust
impl MemoryRepository {
    /// Stage memory changes (like `git add`)
    pub async fn stage_memory(
        &self,
        memory_id: &str,
        operation: MemoryOperation,
    ) -> Result<()> {
        let mut index = self.index.write().await;
        
        match operation {
            MemoryOperation::Add(memory) => {
                index.staged_additions.insert(memory_id.to_string(), memory);
            }
            MemoryOperation::Modify { old_version, new_version } => {
                index.staged_modifications.insert(
                    memory_id.to_string(),
                    MemoryModification {
                        memory_id: memory_id.to_string(),
                        old_version,
                        new_version,
                        diff: self.compute_diff(&old_version, &new_version).await?,
                    },
                );
            }
            MemoryOperation::Delete => {
                index.staged_deletions.insert(memory_id.to_string());
            }
        }
        
        Ok(())
    }
    
    /// Commit staged changes (like `git commit`)
    pub async fn commit(
        &self,
        message: &str,
        author: &str,
    ) -> Result<String> {
        let index = self.index.read().await;
        
        // Create commit from staged changes
        let commit = MemoryCommit {
            hash: self.compute_commit_hash(&index).await?,
            parents: vec![self.get_head_commit().await?],
            author: author.to_string(),
            timestamp: Utc::now(),
            message: message.to_string(),
            changes: MemoryChanges {
                added: index.staged_additions.values().cloned().collect(),
                modified: index.staged_modifications.values().cloned().collect(),
                deleted: index.staged_deletions.iter().cloned().collect(),
                concepts_changed: vec![], // TODO: Track concept changes
            },
            tree: self.create_memory_tree(&index).await?,
        };
        
        // Store commit
        self.commits.write().await.insert(commit.hash.clone(), commit.clone());
        
        // Update current branch to point to new commit
        let mut branches = self.branches.write().await;
        let current_branch_name = self.current_branch.read().await.clone();
        if let Some(branch) = branches.get_mut(&current_branch_name) {
            branch.commit = commit.hash.clone();
        }
        
        // Clear staging area
        drop(branches);
        let mut index = self.index.write().await;
        index.staged_additions.clear();
        index.staged_modifications.clear();
        index.staged_deletions.clear();
        
        Ok(commit.hash)
    }
}
```

### 2. Branch Operations

```rust
impl MemoryBranchManager {
    /// Create a new branch (like `git branch`)
    pub async fn create_branch(
        &self,
        name: &str,
        description: Option<&str>,
        from_commit: Option<&str>,
    ) -> Result<()> {
        let commit = if let Some(commit_hash) = from_commit {
            commit_hash.to_string()
        } else {
            // Create from current HEAD
            self.get_head_commit().await?
        };
        
        let branch = MemoryBranch {
            name: name.to_string(),
            commit,
            description: description.map(|s| s.to_string()),
            is_active: false,
            created_at: Utc::now(),
        };
        
        self.branches.write().await.insert(name.to_string(), branch);
        Ok(())
    }
    
    /// Switch to a branch (like `git checkout`)
    pub async fn checkout(
        &self,
        branch_name: &str,
        agent: &mut Agent,
    ) -> Result<()> {
        // Validate branch exists
        let branch = self.branches.read().await
            .get(branch_name)
            .ok_or_else(|| ThymosError::BranchNotFound(branch_name.to_string()))?
            .clone();
        
        // Get commit this branch points to
        let commit = self.get_commit(&branch.commit).await?;
        
        // Restore memory state from commit
        self.restore_memory_state(agent, &commit).await?;
        
        // Update current branch
        *self.current_branch.write().await = branch_name.to_string();
        
        // Mark branch as active
        let mut branches = self.branches.write().await;
        for b in branches.values_mut() {
            b.is_active = b.name == branch_name;
        }
        
        Ok(())
    }
    
    /// List all branches
    pub async fn list_branches(&self) -> Result<Vec<MemoryBranch>> {
        Ok(self.branches.read().await.values().cloned().collect())
    }
    
    /// Delete a branch
    pub async fn delete_branch(
        &self,
        branch_name: &str,
        force: bool,
    ) -> Result<()> {
        let branches = self.branches.read().await;
        let branch = branches.get(branch_name)
            .ok_or_else(|| ThymosError::BranchNotFound(branch_name.to_string()))?;
        
        // Can't delete active branch unless forced
        if branch.is_active && !force {
            return Err(ThymosError::CannotDeleteActiveBranch);
        }
        
        drop(branches);
        self.branches.write().await.remove(branch_name);
        Ok(())
    }
}
```

### 3. Checkout Operations

```rust
impl MemoryRepository {
    /// Checkout a specific commit (like `git checkout <commit>`)
    pub async fn checkout_commit(
        &self,
        commit_hash: &str,
        agent: &mut Agent,
        create_branch: Option<&str>,
    ) -> Result<()> {
        let commit = self.get_commit(commit_hash).await?;
        
        // Restore memory state
        self.restore_memory_state(agent, &commit).await?;
        
        // If creating a branch, do so in detached HEAD state
        if let Some(branch_name) = create_branch {
            self.create_branch(branch_name, None, Some(commit_hash)).await?;
            self.checkout(branch_name, agent).await?;
        } else {
            // Detached HEAD state
            *self.current_branch.write().await = format!("HEAD@{}", &commit_hash[..8]);
        }
        
        Ok(())
    }
    
    /// Restore memory state from a commit
    async fn restore_memory_state(
        &self,
        agent: &mut Agent,
        commit: &MemoryCommit,
    ) -> Result<()> {
        // Get memory tree for this commit
        let tree = self.get_memory_tree(&commit.tree).await?;
        
        // Restore all memories from tree
        for memory_entry in &tree.memories {
            agent.restore_memory_version(&memory_entry.memory_id, &memory_entry.version).await?;
        }
        
        // Restore concepts
        for concept_entry in &tree.concepts {
            agent.restore_concept_version(&concept_entry.concept_id, &concept_entry.version).await?;
        }
        
        Ok(())
    }
}
```

### 4. Merge Operations

```rust
impl MemoryBranchManager {
    /// Merge a branch into current branch (like `git merge`)
    pub async fn merge(
        &self,
        source_branch: &str,
        target_branch: &str,
        agent: &mut Agent,
        merge_strategy: MergeStrategy,
    ) -> Result<MergeResult> {
        // Get commits for both branches
        let source_branch_obj = self.branches.read().await
            .get(source_branch)
            .ok_or_else(|| ThymosError::BranchNotFound(source_branch.to_string()))?
            .clone();
        
        let target_branch_obj = self.branches.read().await
            .get(target_branch)
            .ok_or_else(|| ThymosError::BranchNotFound(target_branch.to_string()))?
            .clone();
        
        // Find common ancestor
        let common_ancestor = self.find_common_ancestor(
            &source_branch_obj.commit,
            &target_branch_obj.commit,
        ).await?;
        
        // Get changes since common ancestor
        let source_changes = self.get_changes_since(&common_ancestor, &source_branch_obj.commit).await?;
        let target_changes = self.get_changes_since(&common_ancestor, &target_branch_obj.commit).await?;
        
        // Detect conflicts
        let conflicts = self.detect_conflicts(&source_changes, &target_changes).await?;
        
        if !conflicts.is_empty() {
            // Handle conflicts based on strategy
            match merge_strategy {
                MergeStrategy::AutoMerge { llm } => {
                    // Use LLM to resolve conflicts
                    let resolved = self.resolve_conflicts_with_llm(
                        &conflicts,
                        llm,
                    ).await?;
                    
                    // Apply resolved changes
                    self.apply_changes(agent, &resolved).await?;
                }
                
                MergeStrategy::Manual => {
                    // Return conflicts for manual resolution
                    return Ok(MergeResult::Conflicts { conflicts });
                }
                
                MergeStrategy::Ours => {
                    // Keep target branch changes
                    self.checkout(target_branch, agent).await?;
                    return Ok(MergeResult::Success { commit: None });
                }
                
                MergeStrategy::Theirs => {
                    // Use source branch changes
                    self.checkout(source_branch, agent).await?;
                    return Ok(MergeResult::Success { commit: None });
                }
            }
        }
        
        // Create merge commit
        let merge_commit = self.create_merge_commit(
            &source_branch_obj.commit,
            &target_branch_obj.commit,
            &conflicts,
            "Merge branch '{}' into '{}'",
            source_branch,
            target_branch,
        ).await?;
        
        // Update target branch
        let mut branches = self.branches.write().await;
        if let Some(branch) = branches.get_mut(target_branch) {
            branch.commit = merge_commit.hash.clone();
        }
        
        Ok(MergeResult::Success {
            commit: Some(merge_commit.hash),
        })
    }
    
    /// Detect conflicts between two sets of changes
    async fn detect_conflicts(
        &self,
        source_changes: &MemoryChanges,
        target_changes: &MemoryChanges,
    ) -> Result<Vec<MemoryConflict>> {
        let mut conflicts = Vec::new();
        
        // Check for conflicting memory modifications
        for source_mod in &source_changes.modified {
            if let Some(target_mod) = target_changes.modified.iter()
                .find(|m| m.memory_id == source_mod.memory_id)
            {
                // Same memory modified in both branches
                if source_mod.new_version != target_mod.new_version {
                    conflicts.push(MemoryConflict {
                        memory_id: source_mod.memory_id.clone(),
                        source_version: source_mod.new_version.clone(),
                        target_version: target_mod.new_version.clone(),
                        conflict_type: ConflictType::ContentConflict,
                    });
                }
            }
        }
        
        // Check for deletions vs modifications
        for deleted_id in &source_changes.deleted {
            if target_changes.modified.iter().any(|m| &m.memory_id == deleted_id) {
                conflicts.push(MemoryConflict {
                    memory_id: deleted_id.clone(),
                    source_version: "DELETED".to_string(),
                    target_version: "MODIFIED".to_string(),
                    conflict_type: ConflictType::DeleteModifyConflict,
                });
            }
        }
        
        Ok(conflicts)
    }
}

#[derive(Debug, Clone)]
pub enum MergeStrategy {
    AutoMerge { llm: Arc<dyn LLMProvider> },
    Manual,
    Ours,  // Keep target branch
    Theirs, // Use source branch
}

#[derive(Debug)]
pub enum MergeResult {
    Success { commit: Option<String> },
    Conflicts { conflicts: Vec<MemoryConflict> },
}
```

### 5. Worktrees (Concurrent Scenarios)

```rust
/// Memory worktree (like Git worktree)
/// Allows multiple concurrent agent instances with different memory states
pub struct MemoryWorktree {
    /// Worktree ID
    pub id: String,
    
    /// Branch this worktree is on
    pub branch: String,
    
    /// Commit this worktree is at
    pub commit: String,
    
    /// Agent instance for this worktree
    pub agent: Arc<Agent>,
    
    /// Worktree path (for storage isolation)
    pub path: PathBuf,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Worktree manager
pub struct MemoryWorktreeManager {
    worktrees: Arc<RwLock<HashMap<String, MemoryWorktree>>>,
    repository: Arc<MemoryRepository>,
}

impl MemoryWorktreeManager {
    /// Create a new worktree (like `git worktree add`)
    pub async fn create_worktree(
        &self,
        branch_name: &str,
        worktree_id: Option<&str>,
    ) -> Result<String> {
        let worktree_id = worktree_id.unwrap_or(&Uuid::new_v4().to_string());
        
        // Get branch commit
        let branch = self.repository.branches.read().await
            .get(branch_name)
            .ok_or_else(|| ThymosError::BranchNotFound(branch_name.to_string()))?
            .clone();
        
        // Create isolated storage path
        let worktree_path = self.repository.path.join("worktrees").join(worktree_id);
        tokio::fs::create_dir_all(&worktree_path).await?;
        
        // Create agent instance for this worktree
        let agent_config = MemoryConfig {
            mode: MemoryMode::Embedded {
                data_dir: worktree_path.join("memory"),
            },
            ..Default::default()
        };
        
        let agent = Agent::builder()
            .id(&format!("agent_{}", worktree_id))
            .with_memory_config(agent_config)
            .build()
            .await?;
        
        // Restore memory state from branch commit
        let mut agent_mut = agent.clone();
        self.repository.restore_memory_state(&mut agent_mut, &self.repository.get_commit(&branch.commit).await?).await?;
        
        let worktree = MemoryWorktree {
            id: worktree_id.to_string(),
            branch: branch_name.to_string(),
            commit: branch.commit,
            agent: Arc::new(agent_mut),
            path: worktree_path,
            created_at: Utc::now(),
        };
        
        self.worktrees.write().await.insert(worktree_id.to_string(), worktree);
        Ok(worktree_id.to_string())
    }
    
    /// Get worktree agent
    pub async fn get_worktree_agent(
        &self,
        worktree_id: &str,
    ) -> Result<Arc<Agent>> {
        let worktrees = self.worktrees.read().await;
        let worktree = worktrees.get(worktree_id)
            .ok_or_else(|| ThymosError::WorktreeNotFound(worktree_id.to_string()))?;
        
        Ok(worktree.agent.clone())
    }
    
    /// Commit changes from worktree back to branch
    pub async fn commit_worktree_changes(
        &self,
        worktree_id: &str,
        message: &str,
    ) -> Result<String> {
        let worktree = self.worktrees.read().await
            .get(worktree_id)
            .ok_or_else(|| ThymosError::WorktreeNotFound(worktree_id.to_string()))?
            .clone();
        
        // Get changes in worktree
        let changes = self.repository.get_worktree_changes(&worktree).await?;
        
        // Create commit
        let commit_hash = self.repository.commit_changes(
            &worktree.branch,
            changes,
            message,
            &worktree.agent.id(),
        ).await?;
        
        // Update worktree commit
        drop(worktree);
        let mut worktrees = self.worktrees.write().await;
        if let Some(wt) = worktrees.get_mut(worktree_id) {
            wt.commit = commit_hash.clone();
        }
        
        Ok(commit_hash)
    }
    
    /// Remove worktree (like `git worktree remove`)
    pub async fn remove_worktree(
        &self,
        worktree_id: &str,
        force: bool,
    ) -> Result<()> {
        let worktree = self.worktrees.read().await
            .get(worktree_id)
            .ok_or_else(|| ThymosError::WorktreeNotFound(worktree_id.to_string()))?
            .clone();
        
        // Check for uncommitted changes
        let has_changes = self.repository.has_worktree_changes(&worktree).await?;
        if has_changes && !force {
            return Err(ThymosError::WorktreeHasUncommittedChanges);
        }
        
        // Remove worktree directory
        if worktree.path.exists() {
            tokio::fs::remove_dir_all(&worktree.path).await?;
        }
        
        // Remove from worktrees map
        self.worktrees.write().await.remove(worktree_id);
        
        Ok(())
    }
    
    /// List all worktrees
    pub async fn list_worktrees(&self) -> Result<Vec<MemoryWorktree>> {
        Ok(self.worktrees.read().await.values().cloned().collect())
    }
}
```

### 6. Rebase Operations

```rust
impl MemoryBranchManager {
    /// Rebase branch onto another (like `git rebase`)
    pub async fn rebase(
        &self,
        branch_name: &str,
        onto_branch: &str,
        agent: &mut Agent,
    ) -> Result<RebaseResult> {
        let branch = self.branches.read().await
            .get(branch_name)
            .ok_or_else(|| ThymosError::BranchNotFound(branch_name.to_string()))?
            .clone();
        
        let onto_branch_obj = self.branches.read().await
            .get(onto_branch)
            .ok_or_else(|| ThymosError::BranchNotFound(onto_branch.to_string()))?
            .clone();
        
        // Find common ancestor
        let common_ancestor = self.find_common_ancestor(
            &branch.commit,
            &onto_branch_obj.commit,
        ).await?;
        
        // Get commits to replay
        let commits_to_replay = self.get_commits_between(
            &common_ancestor,
            &branch.commit,
        ).await?;
        
        // Checkout onto branch
        self.checkout(onto_branch, agent).await?;
        
        // Replay commits one by one
        let mut rebased_commits = Vec::new();
        for commit in commits_to_replay {
            // Apply commit changes
            let result = self.apply_commit(agent, &commit).await?;
            
            if let Some(conflicts) = result.conflicts {
                // Rebase conflict - need manual resolution
                return Ok(RebaseResult::Conflict {
                    commit: commit.hash,
                    conflicts,
                });
            }
            
            // Create new commit on top of current HEAD
            let new_commit = self.create_rebased_commit(
                &commit,
                &self.get_head_commit().await?,
            ).await?;
            
            rebased_commits.push(new_commit.hash.clone());
        }
        
        // Update branch to point to last rebased commit
        if let Some(last_commit) = rebased_commits.last() {
            let mut branches = self.branches.write().await;
            if let Some(branch) = branches.get_mut(branch_name) {
                branch.commit = last_commit.clone();
            }
        }
        
        Ok(RebaseResult::Success {
            rebased_commits,
        })
    }
}
```

---

## Use-Cases

### Use-Case 1: A/B Testing Agent Strategies

```rust
// Main branch: Current agent behavior
let main_agent = repository.get_agent("main").await?;

// Create branch for aggressive strategy
repository.create_branch("strategy-aggressive", Some("Try aggressive approach"), None).await?;
repository.checkout("strategy-aggressive", &mut main_agent).await?;

// Agent learns aggressive behavior
main_agent.remember("I should be more direct and assertive").await?;
main_agent.remember("I'll take risks to achieve goals").await?;
repository.commit("Adopt aggressive strategy", "agent").await?;

// Create branch for cautious strategy
repository.create_branch("strategy-cautious", Some("Try cautious approach"), Some("main")).await?;
repository.checkout("strategy-cautious", &mut main_agent).await?;

// Agent learns cautious behavior
main_agent.remember("I should be careful and methodical").await?;
main_agent.remember("I'll avoid risks and plan carefully").await?;
repository.commit("Adopt cautious strategy", "agent").await?;

// Test both strategies concurrently using worktrees
let aggressive_worktree = worktree_manager.create_worktree("strategy-aggressive", None).await?;
let cautious_worktree = worktree_manager.create_worktree("strategy-cautious", None).await?;

let aggressive_agent = worktree_manager.get_worktree_agent(&aggressive_worktree).await?;
let cautious_agent = worktree_manager.get_worktree_agent(&cautious_worktree).await?;

// Run both agents in parallel
let aggressive_result = run_scenario(aggressive_agent).await?;
let cautious_result = run_scenario(cautious_agent).await?;

// Compare results and merge best strategy
if aggressive_result.score > cautious_result.score {
    repository.merge("strategy-aggressive", "main", &mut main_agent, MergeStrategy::AutoMerge { llm }).await?;
} else {
    repository.merge("strategy-cautious", "main", &mut main_agent, MergeStrategy::AutoMerge { llm }).await?;
}
```

### Use-Case 2: Narrative Branching in RPG

```rust
// Main timeline: Player kills the dragon
let main_agent = repository.get_agent("main").await?;
main_agent.remember("I killed the dragon and saved the kingdom").await?;
repository.commit("Killed dragon", "player").await?;

// Branch: What if player spared the dragon?
repository.create_branch("spare-dragon", Some("Alternative: spare dragon"), Some("main~1")).await?;
repository.checkout("spare-dragon", &mut main_agent).await?;

main_agent.remember("I spared the dragon. It promised to help us").await?;
main_agent.remember("The dragon became our ally").await?;
repository.commit("Spared dragon", "player").await?;

// Create worktrees to explore both timelines
let kill_worktree = worktree_manager.create_worktree("main", None).await?;
let spare_worktree = worktree_manager.create_worktree("spare-dragon", None).await?;

// Play out both scenarios
let kill_outcome = play_scenario(worktree_manager.get_worktree_agent(&kill_worktree).await?).await?;
let spare_outcome = play_scenario(worktree_manager.get_worktree_agent(&spare_worktree).await?).await?;

// Player chooses which timeline to continue
if player_prefers_spare_outcome {
    repository.merge("spare-dragon", "main", &mut main_agent, MergeStrategy::Theirs).await?;
}
```

### Use-Case 3: Experimentation Without Risk

```rust
// Main branch: Production agent
let production_agent = repository.get_agent("main").await?;

// Create experimental branch
repository.create_branch("experiment-new-learning", Some("Test new learning algorithm"), None).await?;
repository.checkout("experiment-new-learning", &mut production_agent).await?;

// Try new learning approach
production_agent.remember("New learning: focus on long-term patterns").await?;
// ... experiment with new approach ...

repository.commit("Experiment with new learning", "researcher").await?;

// Create worktree to test experiment
let experiment_worktree = worktree_manager.create_worktree("experiment-new-learning", None).await?;
let experiment_agent = worktree_manager.get_worktree_agent(&experiment_worktree).await?;

// Test experiment
let experiment_results = test_agent(experiment_agent).await?;

// If successful, merge to main
if experiment_results.success_rate > 0.9 {
    repository.checkout("main", &mut production_agent).await?;
    repository.merge("experiment-new-learning", "main", &mut production_agent, MergeStrategy::AutoMerge { llm }).await?;
} else {
    // Discard experiment
    repository.delete_branch("experiment-new-learning", true).await?;
}
```

### Use-Case 4: Multi-Agent Collaboration with Branches

```rust
// Main branch: Shared world state
let shared_repo = MemoryRepository::new("shared_world").await?;

// Agent A creates branch for their perspective
shared_repo.create_branch("agent-a-perspective", Some("Agent A's view"), None).await?;
let mut agent_a = shared_repo.get_agent("agent-a-perspective").await?;
shared_repo.checkout("agent-a-perspective", &mut agent_a).await?;

agent_a.remember_shared("The merchant seemed suspicious").await?;
shared_repo.commit("Agent A: merchant observation", "agent-a").await?;

// Agent B creates branch for their perspective
shared_repo.create_branch("agent-b-perspective", Some("Agent B's view"), Some("main")).await?;
let mut agent_b = shared_repo.get_agent("agent-b-perspective").await?;
shared_repo.checkout("agent-b-perspective", &mut agent_b).await?;

agent_b.remember_shared("The merchant was helpful and friendly").await?;
shared_repo.commit("Agent B: merchant observation", "agent-b").await?;

// Merge both perspectives into main
shared_repo.checkout("main", &mut agent_a).await?;
shared_repo.merge("agent-a-perspective", "main", &mut agent_a, MergeStrategy::AutoMerge { llm }).await?;
shared_repo.merge("agent-b-perspective", "main", &mut agent_a, MergeStrategy::AutoMerge { llm }).await?;

// Result: "The merchant's behavior was ambiguous - Agent A found them suspicious, 
//         but Agent B found them helpful"
```

---

## Implementation Architecture

### Storage Structure

```
.memory-repo/
├── objects/              # Memory objects (like Git objects/)
│   ├── memories/        # Memory blobs
│   ├── concepts/        # Concept objects
│   └── trees/           # Memory trees
├── refs/                # References (like Git refs/)
│   ├── heads/           # Branch refs
│   │   ├── main
│   │   ├── experiment-1
│   │   └── ...
│   └── tags/            # Tag refs
├── worktrees/           # Worktree directories
│   ├── worktree-1/
│   │   └── memory/      # Isolated memory storage
│   └── worktree-2/
│       └── memory/
├── index                # Staging area
├── HEAD                  # Current branch/commit
└── config                # Repository configuration
```

### Integration with Locai

```rust
/// Locai-backed memory repository
pub struct LocaiMemoryRepository {
    /// Locai instance for memory storage
    locai: Arc<Locai>,
    
    /// Git-like operations
    git_ops: GitOperations,
    
    /// Branch manager
    branches: MemoryBranchManager,
    
    /// Worktree manager
    worktrees: MemoryWorktreeManager,
}

impl LocaiMemoryRepository {
    /// Store memory with versioning
    pub async fn remember(
        &self,
        content: &str,
        metadata: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<String> {
        // Store in Locai
        let memory_id = self.locai.remember(content, metadata).await?;
        
        // Stage for commit
        self.git_ops.stage_memory(
            &memory_id,
            MemoryOperation::Add(MemoryEntry {
                memory_id: memory_id.clone(),
                content: content.to_string(),
                metadata: metadata.cloned(),
            }),
        ).await?;
        
        Ok(memory_id)
    }
    
    /// Commit staged changes
    pub async fn commit_changes(
        &self,
        branch: &str,
        changes: MemoryChanges,
        message: &str,
        author: &str,
    ) -> Result<String> {
        // Create commit
        let commit = self.git_ops.create_commit(changes, message, author).await?;
        
        // Update branch
        self.branches.update_branch(branch, &commit.hash).await?;
        
        // Store commit in Locai (as metadata or separate storage)
        self.store_commit(&commit).await?;
        
        Ok(commit.hash)
    }
}
```

---

## Benefits

### 1. **Experimentation Without Risk**
- Test new strategies without affecting main memory
- Easy rollback if experiments fail
- Compare multiple approaches

### 2. **Concurrent Scenario Exploration**
- Worktrees enable parallel agent instances
- Test different strategies simultaneously
- Choose best outcome

### 3. **Narrative Branching**
- Support branching storylines
- Player choice preservation
- Multiple timeline exploration

### 4. **Collaborative Development**
- Multiple developers can work on different branches
- Merge agent improvements
- Conflict resolution for memory changes

### 5. **Version Control for Agents**
- Full history of agent learning
- Rollback to previous states
- Audit trail of changes

### 6. **A/B Testing**
- Test different agent configurations
- Compare performance metrics
- Merge winning strategies

---

## Challenges and Considerations

### 1. **Storage Overhead**
- **Problem**: Storing multiple memory versions can be expensive
- **Solution**: Use deltas/diffs instead of full copies, compression

### 2. **Conflict Resolution**
- **Problem**: Merging memories can create conflicts
- **Solution**: LLM-assisted conflict resolution, manual resolution UI

### 3. **Performance**
- **Problem**: Switching branches requires restoring memory state
- **Solution**: Lazy loading, caching, incremental updates

### 4. **Consistency**
- **Problem**: Worktrees need to stay in sync with branches
- **Solution**: Periodic sync, explicit update operations

### 5. **Complexity**
- **Problem**: Git operations can be complex
- **Solution**: Simplified API, high-level operations, good documentation

---

## Future Enhancements

1. **Memory Tags**: Tag important commits (like `git tag`)
2. **Cherry-Picking**: Apply specific commits from one branch to another
3. **Stashing**: Temporarily save uncommitted changes
4. **Bisect**: Find when a memory issue was introduced
5. **Submodules**: Reference other memory repositories
6. **Hooks**: Pre/post commit hooks for validation
7. **Graph Visualization**: Visualize memory branch history

---

## Conclusion

Git-style memory versioning would enable powerful workflows:

- **Experimentation**: Test strategies without risk
- **Concurrency**: Explore scenarios in parallel with worktrees
- **Branching**: Support narrative branching and A/B testing
- **Collaboration**: Multiple agents/developers working on branches
- **Version Control**: Full history and rollback capabilities

This would be a **truly unique** feature - no other agent framework combines:
- Memory versioning
- Git-like operations
- Concurrent worktrees
- Multi-agent coordination
- Temporal awareness

The combination of these features would make Thymos uniquely powerful for agent experimentation, narrative branching, and collaborative agent development.



