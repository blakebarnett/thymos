//! Memory repository core - branch management

use crate::error::{Result, ThymosError};
use chrono::{DateTime, Utc};
use locai::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory repository managing branches and commits
pub struct MemoryRepository {
    /// Underlying Locai instance
    locai: Arc<Locai>,
    
    /// Branch metadata (name -> branch info)
    branches: Arc<RwLock<HashMap<String, MemoryBranch>>>,
    
    /// Commit metadata (hash -> commit info)
    commits: Arc<RwLock<HashMap<String, crate::memory::versioning::commit::MemoryCommit>>>,
    
    /// Current active branch name
    current_branch: Arc<RwLock<String>>,
    
    /// Stored snapshots (snapshot_id -> snapshot)
    snapshots: Arc<RwLock<HashMap<String, locai::storage::models::MemorySnapshot>>>,
    
    /// Staging areas per branch (branch_name -> CommitIndex)
    staging_areas: Arc<RwLock<HashMap<String, crate::memory::versioning::commit::CommitIndex>>>,
    
    /// Branch to commit mapping (branch_name -> latest commit hash)
    branch_heads: Arc<RwLock<HashMap<String, String>>>,
}

/// Memory branch pointing to a Locai snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBranch {
    /// Branch name
    pub name: String,
    
    /// Locai snapshot ID this branch points to
    pub snapshot_id: String,
    
    /// Optional branch description
    pub description: Option<String>,
    
    /// When branch was created
    pub created_at: DateTime<Utc>,
    
    /// Whether this branch is currently active
    pub is_active: bool,
}

impl MemoryRepository {
    /// Create a new memory repository
    ///
    /// # Arguments
    /// * `locai` - Locai instance to use for snapshots
    ///
    /// # Returns
    /// Initialized repository with a default "main" branch
    pub async fn new(locai: Arc<Locai>) -> Result<Self> {
        let branches = Arc::new(RwLock::new(HashMap::new()));
        let commits = Arc::new(RwLock::new(HashMap::new()));
        let current_branch = Arc::new(RwLock::new("main".to_string()));
        let snapshots = Arc::new(RwLock::new(HashMap::new()));
        let staging_areas = Arc::new(RwLock::new(HashMap::new()));
        let branch_heads = Arc::new(RwLock::new(HashMap::new()));
        
        let repo = Self {
            locai,
            branches,
            commits,
            current_branch,
            snapshots,
            staging_areas,
            branch_heads,
        };
        
        // Create default "main" branch
        let _ = repo.create_branch("main", None).await?;
        
        // Initialize staging area for main branch
        repo.staging_areas.write().await.insert("main".to_string(), crate::memory::versioning::commit::CommitIndex::new());
        
        Ok(repo)
    }
    
    /// Create a new branch from current state
    ///
    /// # Arguments
    /// * `name` - Branch name (must be unique)
    /// * `description` - Optional branch description
    ///
    /// # Returns
    /// Created branch
    ///
    /// # Errors
    /// Returns error if branch name already exists
    pub async fn create_branch(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<MemoryBranch> {
        let mut branches = self.branches.write().await;
        
        if branches.contains_key(name) {
            return Err(ThymosError::Configuration(format!(
                "Branch '{}' already exists",
                name
            )));
        }
        
        // Create snapshot of current memory state
        let snapshot = self
            .locai()
            .create_snapshot(None, None)
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to create snapshot: {}", e)))?;
        
        // Store snapshot
        self.snapshots.write().await.insert(snapshot.snapshot_id.clone(), snapshot.clone());
        
        let branch = MemoryBranch {
            name: name.to_string(),
            snapshot_id: snapshot.snapshot_id.clone(),
            description: description.map(|s| s.to_string()),
            created_at: Utc::now(),
            is_active: false,
        };
        
        branches.insert(name.to_string(), branch.clone());
        
        // Initialize staging area for new branch
        self.staging_areas.write().await.insert(name.to_string(), crate::memory::versioning::commit::CommitIndex::new());
        
        Ok(branch)
    }
    
    /// List all branches
    ///
    /// # Returns
    /// Vector of all branches
    pub async fn list_branches(&self) -> Result<Vec<MemoryBranch>> {
        let branches = self.branches.read().await;
        Ok(branches.values().cloned().collect())
    }
    
    /// Get branch by name
    ///
    /// # Arguments
    /// * `name` - Branch name
    ///
    /// # Returns
    /// Branch if found, None otherwise
    pub async fn get_branch(&self, name: &str) -> Result<Option<MemoryBranch>> {
        let branches = self.branches.read().await;
        Ok(branches.get(name).cloned())
    }
    
    /// Delete a branch
    ///
    /// # Arguments
    /// * `name` - Branch name to delete
    /// * `force` - If true, delete even if it's the current branch
    ///
    /// # Errors
    /// Returns error if trying to delete current branch without force
    pub async fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let current = self.current_branch.read().await.clone();
        
        if name == current && !force {
            return Err(ThymosError::Configuration(format!(
                "Cannot delete current branch '{}' without force",
                name
            )));
        }
        
        // Get snapshot ID before removing branch (need to read first)
        let snapshot_id = {
            let branches = self.branches.read().await;
            if !branches.contains_key(name) {
                return Err(ThymosError::AgentNotFound(format!("Branch '{}' not found", name)));
            }
            branches.get(name).map(|b| b.snapshot_id.clone())
        };
        
        // Remove branch metadata (now we can write)
        {
            let mut branches = self.branches.write().await;
            branches.remove(name);
        }
        
        // Clean up staging area
        self.staging_areas.write().await.remove(name);
        
        // Clean up branch head
        self.branch_heads.write().await.remove(name);
        
        // Optionally delete snapshot if no other branches reference it
        if let Some(snapshot_id) = snapshot_id {
            let snapshot_in_use = {
                let branches = self.branches.read().await;
                branches.values().any(|b| b.snapshot_id == snapshot_id)
            };
            
            if !snapshot_in_use {
                // No other branches use this snapshot, we could delete it
                // For now, we'll keep snapshots for potential recovery
                // In production, you might want to delete unused snapshots
                self.snapshots.write().await.remove(&snapshot_id);
            }
        }
        
        Ok(())
    }
    
    /// Get current branch name
    ///
    /// # Returns
    /// Current branch name
    pub async fn get_current_branch(&self) -> String {
        self.current_branch.read().await.clone()
    }
    
    /// Set current branch (internal use, checkout should be used instead)
    pub(crate) async fn set_current_branch(&self, name: &str) -> Result<()> {
        let branches = self.branches.read().await;
        
        if !branches.contains_key(name) {
            return Err(ThymosError::AgentNotFound(format!("Branch '{}' not found", name)));
        }
        
        // Update active status
        let mut branches = self.branches.write().await;
        
        // Deactivate all branches
        for branch in branches.values_mut() {
            branch.is_active = false;
        }
        
        // Activate target branch
        if let Some(branch) = branches.get_mut(name) {
            branch.is_active = true;
        }
        
        // Update current branch
        *self.current_branch.write().await = name.to_string();
        
        Ok(())
    }
    
    /// Get the underlying Locai instance
    pub fn locai(&self) -> &Arc<Locai> {
        &self.locai
    }
    
    /// Get snapshot by ID (from stored snapshots)
    pub(crate) async fn get_snapshot(
        &self,
        snapshot_id: &str,
    ) -> Result<Option<locai::storage::models::MemorySnapshot>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.get(snapshot_id).cloned())
    }
    
    /// Store snapshot (for use by commit module)
    pub(crate) async fn store_snapshot(
        &self,
        snapshot: locai::storage::models::MemorySnapshot,
    ) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot.snapshot_id.clone(), snapshot);
        Ok(())
    }
    
    /// Get commit metadata (for use by commit module)
    pub(crate) async fn add_commit(
        &self,
        commit: crate::memory::versioning::commit::MemoryCommit,
    ) -> Result<()> {
        let mut commits = self.commits.write().await;
        commits.insert(commit.hash.clone(), commit);
        Ok(())
    }
    
    /// Get commit by hash
    pub async fn get_commit(
        &self,
        hash: &str,
    ) -> Result<Option<crate::memory::versioning::commit::MemoryCommit>> {
        let commits_arc = self.get_commits();
        let commits = commits_arc.read().await;
        Ok(commits.get(hash).cloned())
    }
    
    /// Get commit history for a branch
    pub async fn get_commit_history(
        &self,
        branch: &str,
        limit: Option<usize>,
    ) -> Result<Vec<crate::memory::versioning::commit::MemoryCommit>> {
        // Get branch head commit
        let branch_heads_arc = self.get_branch_heads();
        let branch_heads = branch_heads_arc.read().await;
        let head_commit_hash = branch_heads.get(branch);
        
        let commits = self.commits.read().await;
        let mut history = Vec::new();
        let mut visited = std::collections::HashSet::new();
        
        // Start from branch head and traverse parent commits
        if let Some(head_hash) = head_commit_hash {
            let mut current_hash = Some(head_hash.clone());
            
            while let Some(hash) = current_hash {
                if visited.contains(&hash) {
                    break; // Cycle detected
                }
                visited.insert(hash.clone());
                
                if let Some(commit) = commits.get(&hash) {
                    history.push(commit.clone());
                    
                    // Move to parent commits (for now, use first parent)
                    current_hash = commit.parent_commits.first().cloned();
                } else {
                    break;
                }
                
                // Apply limit during traversal
                if let Some(limit_val) = limit && history.len() >= limit_val {
                    break;
                }
            }
        } else {
            // No head commit, return empty history
            return Ok(Vec::new());
        }
        
        Ok(history)
    }
    
    /// Get staging area for current branch
    #[allow(dead_code)]
    pub(crate) async fn get_staging_area(&self) -> Result<()> {
        // This method is kept for potential future use
        // Staging is handled through stage_memory_with_branch
        Ok(())
    }
    
    /// Update branch head after commit
    pub(crate) async fn update_branch_head(&self, branch: &str, commit_hash: &str) -> Result<()> {
        let branch_heads_arc = self.get_branch_heads();
        let mut branch_heads = branch_heads_arc.write().await;
        branch_heads.insert(branch.to_string(), commit_hash.to_string());
        Ok(())
    }
    
    /// Get staging area for a branch (for use by commit module)
    #[allow(dead_code)]
    pub(crate) async fn get_staging_area_for_branch(
        &self,
        branch: &str,
    ) -> Result<std::sync::Arc<tokio::sync::RwLock<crate::memory::versioning::commit::CommitIndex>>> {
        let mut staging_areas = self.staging_areas.write().await;
        if !staging_areas.contains_key(branch) {
            staging_areas.insert(branch.to_string(), crate::memory::versioning::commit::CommitIndex::new());
        }
        // We can't return a reference, so we'll use the method approach instead
        Err(ThymosError::Configuration("Use stage_memory_with_branch instead".to_string()))
    }
    
    /// Get commits map (for use by merge module)
    pub(crate) fn get_commits(&self) -> std::sync::Arc<tokio::sync::RwLock<HashMap<String, crate::memory::versioning::commit::MemoryCommit>>> {
        self.commits.clone()
    }
    
    /// Get staging areas map (for use by commit module)
    pub(crate) fn get_staging_areas(&self) -> std::sync::Arc<tokio::sync::RwLock<HashMap<String, crate::memory::versioning::commit::CommitIndex>>> {
        self.staging_areas.clone()
    }
    
    /// Get branch heads map (for use by merge module)
    pub(crate) fn get_branch_heads(&self) -> std::sync::Arc<tokio::sync::RwLock<HashMap<String, String>>> {
        self.branch_heads.clone()
    }
    
    /// Get branches map (for use by merge module)
    pub(crate) fn get_branches(&self) -> std::sync::Arc<tokio::sync::RwLock<HashMap<String, MemoryBranch>>> {
        self.branches.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_repo() -> Result<MemoryRepository> {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let locai = Locai::with_data_dir(temp_dir.path())
            .await
            .map_err(|e| ThymosError::MemoryInit(e.to_string()))?;
        
        MemoryRepository::new(Arc::new(locai)).await
    }

    #[tokio::test]
    async fn test_repository_creation() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        let current = repo.get_current_branch().await;
        assert_eq!(current, "main");
        
        let branches = repo.list_branches().await.expect("Failed to list branches");
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
    }

    #[tokio::test]
    async fn test_create_branch() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        let branch = repo
            .create_branch("experiment", Some("Test branch"))
            .await
            .expect("Failed to create branch");
        
        assert_eq!(branch.name, "experiment");
        assert_eq!(branch.description, Some("Test branch".to_string()));
        
        let branches = repo.list_branches().await.expect("Failed to list branches");
        assert_eq!(branches.len(), 2);
    }

    #[tokio::test]
    async fn test_create_duplicate_branch() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        repo.create_branch("experiment", None)
            .await
            .expect("Failed to create branch");
        
        let result = repo.create_branch("experiment", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_branch() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        repo.create_branch("experiment", None)
            .await
            .expect("Failed to create branch");
        
        repo.delete_branch("experiment", false)
            .await
            .expect("Failed to delete branch");
        
        let branches = repo.list_branches().await.expect("Failed to list branches");
        assert_eq!(branches.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_current_branch() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        // Cannot delete current branch without force
        let result = repo.delete_branch("main", false).await;
        assert!(result.is_err());
        
        // Can delete with force
        repo.delete_branch("main", true)
            .await
            .expect("Failed to delete branch with force");
    }

    #[tokio::test]
    async fn test_commit_creation() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        // Create a commit
        let commit_hash = repo
            .commit("Initial commit", "test_agent", None)
            .await
            .expect("Failed to create commit");
        
        assert!(!commit_hash.is_empty());
        
        // Retrieve the commit
        let commit = repo
            .get_commit(&commit_hash)
            .await
            .expect("Failed to get commit")
            .expect("Commit not found");
        
        assert_eq!(commit.message, "Initial commit");
        assert_eq!(commit.author, "test_agent");
        assert!(commit.parent_commits.is_empty());
    }

    #[tokio::test]
    async fn test_commit_history() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        // Create multiple commits
        let commit1 = repo
            .commit("First commit", "test_agent", None)
            .await
            .expect("Failed to create commit");
        
        let commit2 = repo
            .commit("Second commit", "test_agent", Some(vec![commit1.clone()]))
            .await
            .expect("Failed to create commit");
        
        // Get commit history
        let history = repo
            .get_commit_history("main", None)
            .await
            .expect("Failed to get commit history");
        
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].hash, commit2);
        assert_eq!(history[1].hash, commit1);
    }

    #[tokio::test]
    async fn test_commit_history_with_limit() {
        let repo = create_test_repo().await.expect("Failed to create repository");
        
        // Create multiple commits, linking them properly
        let commit1 = repo.commit("Commit 1", "test_agent", None)
            .await
            .expect("Failed to create commit");
        
        let commit2 = repo.commit("Commit 2", "test_agent", Some(vec![commit1.clone()]))
            .await
            .expect("Failed to create commit");
        
        let commit3 = repo.commit("Commit 3", "test_agent", Some(vec![commit2.clone()]))
            .await
            .expect("Failed to create commit");
        
        // Get limited history (should return 2 most recent commits)
        let history = repo
            .get_commit_history("main", Some(2))
            .await
            .expect("Failed to get commit history");
        
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].hash, commit3);
        assert_eq!(history[1].hash, commit2);
    }
}

