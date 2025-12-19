//! Commit system - staging area and commit creation

use crate::error::{Result, ThymosError};
use chrono::{DateTime, Utc};
use locai::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Memory commit wrapping a Locai snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCommit {
    /// Commit hash (SHA-256)
    pub hash: String,
    
    /// Locai snapshot ID
    pub snapshot_id: String,
    
    /// Parent commit hashes
    pub parent_commits: Vec<String>,
    
    /// Agent ID that created the commit
    pub author: String,
    
    /// Commit message
    pub message: String,
    
    /// Commit timestamp
    pub timestamp: DateTime<Utc>,
}

/// Staging area (index) for memory changes
pub struct CommitIndex {
    /// Staged memory additions
    staged_additions: HashMap<String, Memory>,
    
    /// Staged memory modifications
    staged_modifications: HashMap<String, MemoryModification>,
    
    /// Staged memory deletions
    staged_deletions: HashSet<String>,
}

/// Memory modification in staging area
#[derive(Debug, Clone)]
pub struct MemoryModification {
    /// Memory ID
    pub memory_id: String,
    
    /// New content (if changed)
    pub new_content: Option<String>,
    
    /// New properties/metadata (if changed)
    pub new_properties: Option<serde_json::Value>,
}

/// Memory operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOperation {
    /// Add new memory
    Add,
    
    /// Modify existing memory
    Modify,
    
    /// Delete memory
    Delete,
}

impl CommitIndex {
    /// Create a new empty staging area
    pub fn new() -> Self {
        Self {
            staged_additions: HashMap::new(),
            staged_modifications: HashMap::new(),
            staged_deletions: HashSet::new(),
        }
    }
    
    /// Stage a memory addition
    pub fn stage_addition(&mut self, memory: Memory) {
        let memory_id = memory.id.clone();
        // Remove from deletions if it was staged for deletion
        self.staged_deletions.remove(&memory_id);
        // Remove from modifications if it was staged for modification
        self.staged_modifications.remove(&memory_id);
        self.staged_additions.insert(memory_id, memory);
    }
    
    /// Stage a memory modification
    pub fn stage_modification(&mut self, memory_id: String, modification: MemoryModification) {
        // Remove from additions if it was staged as addition
        self.staged_additions.remove(&memory_id);
        // Remove from deletions if it was staged for deletion
        self.staged_deletions.remove(&memory_id);
        self.staged_modifications.insert(memory_id, modification);
    }
    
    /// Stage a memory deletion
    pub fn stage_deletion(&mut self, memory_id: String) {
        // Remove from additions if it was staged as addition
        self.staged_additions.remove(&memory_id);
        // Remove from modifications if it was staged for modification
        self.staged_modifications.remove(&memory_id);
        self.staged_deletions.insert(memory_id);
    }
    
    /// Check if staging area is empty
    pub fn is_empty(&self) -> bool {
        self.staged_additions.is_empty()
            && self.staged_modifications.is_empty()
            && self.staged_deletions.is_empty()
    }
    
    /// Clear staging area
    pub fn clear(&mut self) {
        self.staged_additions.clear();
        self.staged_modifications.clear();
        self.staged_deletions.clear();
    }
    
    /// Get all staged changes
    pub fn get_changes(&self) -> (Vec<&Memory>, Vec<&MemoryModification>, Vec<&String>) {
        (
            self.staged_additions.values().collect(),
            self.staged_modifications.values().collect(),
            self.staged_deletions.iter().collect(),
        )
    }
}

impl Default for CommitIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension methods for MemoryRepository to handle commits
impl super::repository::MemoryRepository {
    /// Stage a memory change
    ///
    /// # Arguments
    /// * `memory_id` - Memory ID
    /// * `operation` - Operation type (Add, Modify, Delete)
    /// * `memory` - Memory object (for Add/Modify operations)
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn stage_memory(
        &self,
        _memory_id: &str,
        operation: MemoryOperation,
        memory: Option<Memory>,
    ) -> Result<()> {
        let current_branch = self.get_current_branch().await;
        // For staging, we need the memory ID from the memory object
        let memory_id = memory.as_ref().map(|m| m.id.clone()).unwrap_or_default();
        self.stage_memory_with_branch(&current_branch, &memory_id, operation, memory).await
    }
    
    /// Stage a memory change for a specific branch
    ///
    /// # Arguments
    /// * `branch` - Branch name
    /// * `memory_id` - Memory ID
    /// * `operation` - Operation type (Add, Modify, Delete)
    /// * `memory` - Memory object (for Add/Modify operations)
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn stage_memory_with_branch(
        &self,
        branch: &str,
        memory_id: &str,
        operation: MemoryOperation,
        memory: Option<Memory>,
    ) -> Result<()> {
        let staging_areas_arc = self.get_staging_areas();
        let mut staging_areas = staging_areas_arc.write().await;
        
        // Ensure staging area exists for branch
        if !staging_areas.contains_key(branch) {
            staging_areas.insert(branch.to_string(), CommitIndex::new());
        }
        
        let staging_area = staging_areas.get_mut(branch)
            .ok_or_else(|| ThymosError::Configuration(format!("Failed to get staging area for branch '{}'", branch)))?;
        
        match operation {
            MemoryOperation::Add => {
                if let Some(mem) = memory {
                    staging_area.stage_addition(mem);
                } else {
                    return Err(ThymosError::Configuration(
                        "Memory object required for Add operation".to_string(),
                    ));
                }
            }
            MemoryOperation::Modify => {
                if let Some(mem) = memory {
                    let modification = MemoryModification {
                        memory_id: memory_id.to_string(),
                        new_content: Some(mem.content.clone()),
                        new_properties: Some(serde_json::to_value(&mem.properties).unwrap_or_default()),
                    };
                    staging_area.stage_modification(memory_id.to_string(), modification);
                } else {
                    return Err(ThymosError::Configuration(
                        "Memory object required for Modify operation".to_string(),
                    ));
                }
            }
            MemoryOperation::Delete => {
                staging_area.stage_deletion(memory_id.to_string());
            }
        }
        
        Ok(())
    }
    
    /// Clear staging area for current branch
    pub async fn clear_staging(&self) -> Result<()> {
        let current_branch = self.get_current_branch().await;
        self.clear_staging_for_branch(&current_branch).await
    }
    
    /// Clear staging area for a specific branch
    pub async fn clear_staging_for_branch(&self, branch: &str) -> Result<()> {
        let staging_areas_arc = self.get_staging_areas();
        let mut staging_areas = staging_areas_arc.write().await;
        if let Some(staging_area) = staging_areas.get_mut(branch) {
            staging_area.clear();
        }
        Ok(())
    }
    
    /// Create a commit from current memory state
    ///
    /// # Arguments
    /// * `message` - Commit message
    /// * `author` - Agent ID that created the commit
    /// * `parent_commits` - Optional parent commit hashes
    ///
    /// # Returns
    /// Commit hash
    pub async fn commit(
        &self,
        message: &str,
        author: &str,
        parent_commits: Option<Vec<String>>,
    ) -> Result<String> {
        // Create snapshot of current memory state
        let locai = self.locai();
        let snapshot = locai
            .create_snapshot(None, None)
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to create snapshot: {}", e)))?;
        
        // Store snapshot in repository
        self.store_snapshot(snapshot.clone()).await?;
        
        // Compute commit hash from snapshot + metadata
        let commit_data = format!(
            "{}{}{}{}",
            snapshot.snapshot_id,
            message,
            author,
            Utc::now().to_rfc3339()
        );
        
        let mut hasher = Sha256::new();
        hasher.update(commit_data.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        
        let commit = MemoryCommit {
            hash: hash.clone(),
            snapshot_id: snapshot.snapshot_id,
            parent_commits: parent_commits.unwrap_or_default(),
            author: author.to_string(),
            message: message.to_string(),
            timestamp: Utc::now(),
        };
        
        // Store commit metadata
        self.add_commit(commit.clone()).await?;
        
        // Update branch head
        let current_branch = self.get_current_branch().await;
        self.update_branch_head(&current_branch, &hash).await?;
        
        // Clear staging area after commit
        self.clear_staging().await?;
        
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_index_empty() {
        let index = CommitIndex::new();
        assert!(index.is_empty());
    }

    #[test]
    fn test_commit_index_stage_addition() {
        let mut index = CommitIndex::new();
        assert!(index.is_empty());
        
        // Test that we can stage operations (full test requires Memory objects)
        index.stage_deletion("test_id".to_string());
        assert!(!index.is_empty());
    }

    #[test]
    fn test_commit_index_clear() {
        let mut index = CommitIndex::new();
        index.clear();
        assert!(index.is_empty());
    }
}

