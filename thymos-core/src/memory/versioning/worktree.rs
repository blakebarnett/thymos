//! Worktrees - concurrent agent instances with isolated memory

use crate::agent::Agent;
use crate::config::MemoryConfig;
use crate::error::{Result, ThymosError};
use chrono::{DateTime, Utc};
use locai::prelude::*;
use locai::storage::models::RestoreMode;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory worktree for concurrent agent instances
pub struct MemoryWorktree {
    /// Worktree unique ID
    pub id: String,
    
    /// Branch this worktree is based on
    pub branch: String,
    
    /// Commit this worktree is based on
    pub commit: String,
    
    /// Agent instance with isolated memory
    pub agent: Arc<Agent>,
    
    /// Isolated Locai instance for this worktree
    pub locai_instance: Arc<Locai>,
    
    /// Worktree directory path
    pub path: PathBuf,
    
    /// When worktree was created
    pub created_at: DateTime<Utc>,
}

/// Worktree manager for creating and managing worktrees
pub struct MemoryWorktreeManager {
    /// Active worktrees (id -> worktree)
    worktrees: Arc<RwLock<HashMap<String, MemoryWorktree>>>,
    
    /// Repository this manager belongs to
    repository: Arc<super::repository::MemoryRepository>,
}

impl MemoryWorktreeManager {
    /// Create a new worktree manager
    ///
    /// # Arguments
    /// * `repository` - Memory repository to use
    ///
    /// # Returns
    /// New worktree manager
    pub fn new(repository: Arc<super::repository::MemoryRepository>) -> Self {
        Self {
            worktrees: Arc::new(RwLock::new(HashMap::new())),
            repository,
        }
    }
    
    /// Create a new worktree from a branch
    ///
    /// # Arguments
    /// * `branch_name` - Branch to create worktree from
    /// * `worktree_id` - Optional worktree ID (auto-generated if None)
    /// * `agent_id` - Agent ID for the worktree agent
    /// * `memory_config` - Memory configuration for isolated Locai instance
    ///
    /// # Returns
    /// Worktree ID
    pub async fn create_worktree(
        &self,
        branch_name: &str,
        worktree_id: Option<&str>,
        agent_id: &str,
        memory_config: MemoryConfig,
    ) -> Result<String> {
        // Get branch
        let branch = self
            .repository
            .get_branch(branch_name)
            .await?
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Branch '{}' not found", branch_name)))?;
        
        // Generate worktree ID
        let id = worktree_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        // Create isolated Locai instance
        let worktree_path = std::env::temp_dir().join(format!("thymos_worktree_{}", id));
        std::fs::create_dir_all(&worktree_path)
            .map_err(ThymosError::Io)?;
        
        let locai_instance = match &memory_config.mode {
            crate::config::MemoryMode::Embedded { data_dir: _ } => {
                let worktree_data_dir = worktree_path.join("data");
                Locai::with_data_dir(&worktree_data_dir)
                    .await
                    .map_err(|e| ThymosError::MemoryInit(e.to_string()))?
            }
            _ => {
                return Err(ThymosError::Configuration(
                    "Worktrees only support embedded memory mode".to_string(),
                ));
            }
        };
        
        // Get snapshot and restore to isolated instance
        let snapshot = self
            .repository
            .get_snapshot(&branch.snapshot_id)
            .await?
            .ok_or_else(|| ThymosError::Memory(format!("Snapshot '{}' not found", branch.snapshot_id)))?;
        
        locai_instance
            .restore_snapshot(&snapshot, RestoreMode::Overwrite)
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to restore snapshot: {}", e)))?;
        
        // Create agent with isolated memory
        let mut memory_config_worktree = memory_config.clone();
        memory_config_worktree.mode = crate::config::MemoryMode::Embedded {
            data_dir: worktree_path.join("data"),
        };
        
        let agent = crate::agent::Agent::builder()
            .id(agent_id)
            .with_memory_config(memory_config_worktree)
            .build()
            .await?;
        
        // Create worktree
        let worktree = MemoryWorktree {
            id: id.clone(),
            branch: branch_name.to_string(),
            commit: branch.snapshot_id.clone(), // Using snapshot ID as commit for now
            agent: Arc::new(agent),
            locai_instance: Arc::new(locai_instance),
            path: worktree_path,
            created_at: Utc::now(),
        };
        
        // Store worktree
        self.worktrees.write().await.insert(id.clone(), worktree);
        
        Ok(id)
    }
    
    /// Get worktree agent by ID
    ///
    /// # Arguments
    /// * `worktree_id` - Worktree ID
    ///
    /// # Returns
    /// Agent instance
    pub async fn get_worktree_agent(
        &self,
        worktree_id: &str,
    ) -> Result<Arc<Agent>> {
        let worktrees = self.worktrees.read().await;
        let worktree = worktrees
            .get(worktree_id)
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Worktree '{}' not found", worktree_id)))?;
        
        Ok(worktree.agent.clone())
    }
    
    /// Commit changes from worktree back to branch
    ///
    /// # Arguments
    /// * `worktree_id` - Worktree ID
    /// * `message` - Commit message
    ///
    /// # Returns
    /// Commit hash
    pub async fn commit_worktree_changes(
        &self,
        worktree_id: &str,
        message: &str,
    ) -> Result<String> {
        let worktrees = self.worktrees.read().await;
        let worktree = worktrees
            .get(worktree_id)
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Worktree '{}' not found", worktree_id)))?;
        
        // Create snapshot from worktree's Locai instance
        let _snapshot = worktree
            .locai_instance
            .create_snapshot(None, None)
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to create snapshot: {}", e)))?;
        
        // Create commit in repository
        let commit_hash = self
            .repository
            .commit(message, &worktree.agent.id, None)
            .await?;
        
        // Update worktree's commit reference
        let mut worktrees = self.worktrees.write().await;
        if let Some(worktree_ref) = worktrees.get_mut(worktree_id) {
            worktree_ref.commit = commit_hash.clone();
        }
        
        Ok(commit_hash)
    }
    
    /// Remove a worktree
    ///
    /// # Arguments
    /// * `worktree_id` - Worktree ID to remove
    /// * `force` - If true, remove even if there are uncommitted changes
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn remove_worktree(
        &self,
        worktree_id: &str,
        force: bool,
    ) -> Result<()> {
        let mut worktrees = self.worktrees.write().await;
        
        let worktree = worktrees
            .remove(worktree_id)
            .ok_or_else(|| ThymosError::AgentNotFound(format!("Worktree '{}' not found", worktree_id)))?;
        
        // Clean up worktree directory
        if force {
            std::fs::remove_dir_all(&worktree.path)
                .map_err(ThymosError::Io)?;
        }
        
        Ok(())
    }
    
    /// List all worktrees
    ///
    /// # Returns
    /// Vector of worktree IDs
    pub async fn list_worktrees(&self) -> Result<Vec<String>> {
        let worktrees = self.worktrees.read().await;
        Ok(worktrees.keys().cloned().collect())
    }
}

