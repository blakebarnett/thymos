//! Merge operations - merging branches with conflict resolution

use crate::error::{Result, ThymosError};
use crate::llm::LLMProvider;
use locai::prelude::*;
use locai::storage::models::MemorySnapshot;
use std::sync::Arc;

/// Merge strategy for resolving conflicts
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum MergeStrategy {
    /// Automatic merge using LLM for conflict resolution
    AutoMerge {
        /// LLM provider for conflict resolution
        llm: Arc<dyn LLMProvider>,
    },
    
    /// Manual merge (requires user intervention)
    Manual,
    
    /// Keep target branch (ours)
    Ours,
    
    /// Use source branch (theirs)
    Theirs,
    
    /// Custom conflict resolver
    Custom {
        /// Custom resolver implementation
        resolver: Arc<dyn ConflictResolver>,
    },
}

/// Conflict resolver trait
#[async_trait::async_trait]
pub trait ConflictResolver: Send + Sync {
    /// Resolve a memory conflict
    async fn resolve_conflict(
        &self,
        conflict: &MemoryConflict,
    ) -> Result<ConflictResolution>;
}

/// Merge result
#[derive(Debug, Clone)]
pub enum MergeResult {
    /// Merge succeeded (optionally with commit hash)
    Success {
        /// Commit hash if merge commit was created
        commit: Option<String>,
    },
    
    /// Merge has conflicts that need resolution
    Conflicts {
        /// List of conflicts
        conflicts: Vec<MemoryConflict>,
    },
}

/// Memory conflict detected during merge
#[derive(Debug, Clone)]
pub struct MemoryConflict {
    /// Memory ID that has conflicts
    pub memory_id: String,
    
    /// Target branch version
    pub target_version: Memory,
    
    /// Source branch version
    pub source_version: Memory,
    
    /// Conflict description
    pub description: String,
}

/// Conflict resolution
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    /// Resolved memory content
    pub resolved_content: String,
    
    /// Resolved properties/metadata
    pub resolved_properties: Option<serde_json::Value>,
}

/// Extension methods for MemoryRepository to handle merges
impl super::repository::MemoryRepository {
    /// Merge source branch into target branch
    ///
    /// # Arguments
    /// * `source_branch` - Branch to merge from
    /// * `target_branch` - Branch to merge into
    /// * `strategy` - Merge strategy
    ///
    /// # Returns
    /// Merge result (success or conflicts)
    pub async fn merge(
        &self,
        source_branch: &str,
        target_branch: &str,
        strategy: MergeStrategy,
    ) -> Result<MergeResult> {
        // Get branches
        let source = self
            .get_branch(source_branch)
            .await?
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Branch '{}' not found", source_branch)))?;
        
        let target = self
            .get_branch(target_branch)
            .await?
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Branch '{}' not found", target_branch)))?;
        
        // Get snapshots
        let source_snapshot = self
            .get_snapshot(&source.snapshot_id)
            .await?
            .ok_or_else(|| ThymosError::Memory(format!("Source snapshot '{}' not found", source.snapshot_id)))?;
        
        let target_snapshot = self
            .get_snapshot(&target.snapshot_id)
            .await?
            .ok_or_else(|| ThymosError::Memory(format!("Target snapshot '{}' not found", target.snapshot_id)))?;
        
        // Get branch head commits for finding common ancestor
        let branch_heads_arc = self.get_branch_heads();
        let branch_heads_read = branch_heads_arc.read().await;
        let source_head = branch_heads_read.get(source_branch).cloned();
        let target_head = branch_heads_read.get(target_branch).cloned();
        drop(branch_heads_read);
        
        // Find common ancestor if both branches have commits
        let _common_ancestor = if let (Some(source_hash), Some(target_hash)) = (&source_head, &target_head) {
            self.find_common_ancestor(source_hash, target_hash).await.ok()
        } else {
            None
        };
        
        // Detect conflicts
        let conflicts = self
            .detect_conflicts(&source_snapshot, &target_snapshot)
            .await?;
        
        if !conflicts.is_empty() {
            // Resolve conflicts based on strategy
            match strategy {
                MergeStrategy::Ours => {
                    // Keep target branch - no action needed
                    return Ok(MergeResult::Success { commit: None });
                }
                MergeStrategy::Theirs => {
                    // Use source branch - update target branch to point to source snapshot
                    let branches_arc = self.get_branches();
                    let mut branches = branches_arc.write().await;
                    if let Some(target_branch_ref) = branches.get_mut(target_branch) {
                        target_branch_ref.snapshot_id = source.snapshot_id.clone();
                    }
                    drop(branches);
                    
                    // Update branch head if source has a commit
                    if let Some(source_hash) = source_head {
                        let branch_heads_arc = self.get_branch_heads();
                        let mut branch_heads = branch_heads_arc.write().await;
                        branch_heads.insert(target_branch.to_string(), source_hash);
                    }
                    
                    return Ok(MergeResult::Success { commit: None });
                }
                MergeStrategy::AutoMerge { .. } | MergeStrategy::Manual | MergeStrategy::Custom { .. } => {
                    // Need conflict resolution
                    return Ok(MergeResult::Conflicts { conflicts });
                }
            }
        }
        
        // No conflicts - create merge commit
        // Merge commits have both source and target as parents
        let parent_commits = match (&source_head, &target_head) {
            (Some(source_hash), Some(target_hash)) => {
                vec![source_hash.clone(), target_hash.clone()]
            }
            (Some(source_hash), None) => vec![source_hash.clone()],
            (None, Some(target_hash)) => vec![target_hash.clone()],
            (None, None) => Vec::new(),
        };
        
        // Create merge commit
        let merge_message = format!("Merge branch '{}' into '{}'", source_branch, target_branch);
        let merge_commit_hash = self
            .commit(&merge_message, "system", Some(parent_commits))
            .await?;
        
        // Update target branch snapshot to merged state
        let locai = self.locai();
        let merged_snapshot = locai
            .create_snapshot(None, None)
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to create merge snapshot: {}", e)))?;
        
        self.store_snapshot(merged_snapshot.clone()).await?;
        
        let branches_arc = self.get_branches();
        let mut branches = branches_arc.write().await;
        if let Some(target_branch_ref) = branches.get_mut(target_branch) {
            target_branch_ref.snapshot_id = merged_snapshot.snapshot_id;
        }
        
        Ok(MergeResult::Success {
            commit: Some(merge_commit_hash),
        })
    }
    
    /// Find common ancestor commit (git merge-base equivalent)
    ///
    /// Uses a two-pointer approach: traverse both commit histories
    /// and find the first common commit.
    ///
    /// # Arguments
    /// * `commit_a` - First commit hash
    /// * `commit_b` - Second commit hash
    ///
    /// # Returns
    /// Common ancestor commit hash
    async fn find_common_ancestor(
        &self,
        commit_a: &str,
        commit_b: &str,
    ) -> Result<String> {
        let commits_arc = self.get_commits();
        let commits = commits_arc.read().await;
        
        // If commits are the same, return it
        if commit_a == commit_b {
            return Ok(commit_a.to_string());
        }
        
        // Collect all ancestors of commit_a
        let mut ancestors_a = std::collections::HashSet::new();
        let mut current_a = Some(commit_a.to_string());
        while let Some(hash) = current_a {
            ancestors_a.insert(hash.clone());
            if let Some(commit) = commits.get(&hash) {
                current_a = commit.parent_commits.first().cloned();
            } else {
                break;
            }
        }
        
        // Traverse commit_b's history to find first common ancestor
        let mut current_b = Some(commit_b.to_string());
        while let Some(hash) = current_b {
            if ancestors_a.contains(&hash) {
                return Ok(hash);
            }
            if let Some(commit) = commits.get(&hash) {
                current_b = commit.parent_commits.first().cloned();
            } else {
                break;
            }
        }
        
        // No common ancestor found - return empty string or first commit
        // In Git, this would be an error, but we'll return commit_a as fallback
        Ok(commit_a.to_string())
    }
    
    /// Detect conflicts between two snapshots
    ///
    /// A conflict occurs when the same memory ID exists in both snapshots
    /// but with different content or properties.
    ///
    /// # Arguments
    /// * `source_snapshot` - Source branch snapshot
    /// * `target_snapshot` - Target branch snapshot
    ///
    /// # Returns
    /// List of detected conflicts
    async fn detect_conflicts(
        &self,
        source_snapshot: &MemorySnapshot,
        target_snapshot: &MemorySnapshot,
    ) -> Result<Vec<MemoryConflict>> {
        let mut conflicts = Vec::new();
        
        // Get memories from both snapshots
        // Note: We need to query Locai for the actual memory content from snapshots
        // For now, we'll compare memory IDs and version IDs
        
        // Check for memories that exist in both snapshots with different versions
        for (memory_id, source_version_id) in &source_snapshot.version_map {
            if let Some(target_version_id) = target_snapshot.version_map.get(memory_id) {
                // Same memory ID exists in both snapshots
                if source_version_id != target_version_id {
                    // Different versions - potential conflict
                    // Try to get the actual memory objects to compare content
                    // Use search_snapshot to find the memory by ID
                    let locai = self.locai();
                    let source_memories = locai
                        .search_snapshot(source_snapshot, memory_id, None)
                        .await
                        .map_err(|e| ThymosError::Memory(format!("Failed to search source snapshot: {}", e)))?;
                    
                    let target_memories = locai
                        .search_snapshot(target_snapshot, memory_id, None)
                        .await
                        .map_err(|e| ThymosError::Memory(format!("Failed to search target snapshot: {}", e)))?;
                    
                    let source_memory = source_memories.iter().find(|m| m.id == *memory_id);
                    let target_memory = target_memories.iter().find(|m| m.id == *memory_id);
                    
                    if let (Some(source_mem), Some(target_mem)) = (source_memory, target_memory) {
                        // Compare content and properties
                        if source_mem.content != target_mem.content
                            || source_mem.properties != target_mem.properties
                        {
                            conflicts.push(MemoryConflict {
                                memory_id: memory_id.clone(),
                                target_version: target_mem.clone(),
                                source_version: source_mem.clone(),
                                description: format!(
                                    "Memory '{}' was modified differently in source and target branches",
                                    memory_id
                                ),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(conflicts)
    }
}

