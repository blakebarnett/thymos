//! Parallel Execution with Memory Isolation
//!
//! Run multiple workflows concurrently, each with isolated memory via worktrees.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::config::MemoryConfig;
use crate::error::{Result, ThymosError};
use crate::memory::versioning::{MemoryRepository, MemoryWorktreeManager};

/// Result from an isolated branch execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolatedBranchResult {
    /// Branch name
    pub name: String,
    /// Worktree ID
    pub worktree_id: String,
    /// Output from the branch
    pub output: serde_json::Value,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in ms
    pub duration_ms: u64,
    /// Commit hash if changes were committed
    pub commit: Option<String>,
}

/// Configuration for parallel isolated execution
#[derive(Debug, Clone)]
pub struct ParallelIsolatedConfig {
    /// Maximum concurrent executions
    pub max_concurrency: usize,
    /// Whether to commit changes in each worktree
    pub commit_changes: bool,
    /// Whether to merge successful results back
    pub merge_back: bool,
    /// Whether to clean up worktrees after execution
    pub cleanup: bool,
}

impl Default for ParallelIsolatedConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 4,
            commit_changes: true,
            merge_back: false,
            cleanup: true,
        }
    }
}

/// An isolated branch execution function
pub type IsolatedBranchFn = Arc<
    dyn Fn(serde_json::Value) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send>,
    > + Send
        + Sync,
>;

/// A branch to execute
struct IsolatedBranch {
    name: String,
    execute: IsolatedBranchFn,
}

/// Parallel execution with memory isolation
pub struct ParallelIsolated {
    /// Name for tracing
    name: String,
    /// Memory repository
    repository: Arc<MemoryRepository>,
    /// Worktree manager
    worktree_manager: Arc<MemoryWorktreeManager>,
    /// Branches to execute
    branches: Vec<IsolatedBranch>,
    /// Memory config for worktrees
    memory_config: MemoryConfig,
    /// Configuration
    config: ParallelIsolatedConfig,
}

impl ParallelIsolated {
    /// Create a new builder
    pub fn builder(
        name: impl Into<String>,
        repository: Arc<MemoryRepository>,
    ) -> ParallelIsolatedBuilder {
        ParallelIsolatedBuilder::new(name, repository)
    }

    /// Execute all branches with isolation
    pub async fn execute(
        &self,
        input: serde_json::Value,
    ) -> Result<Vec<IsolatedBranchResult>> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrency));

        let mut handles = Vec::new();

        for branch in &self.branches {
            let branch_name = format!("isolated-{}-{}", self.name, branch.name);

            // Create branch
            self.repository
                .create_branch(&branch_name, Some(&format!("Isolated: {}", branch.name)))
                .await?;

            // Create worktree
            let worktree_id = self
                .worktree_manager
                .create_worktree(
                    &branch_name,
                    None,
                    &format!("isolated-agent-{}", branch.name),
                    self.memory_config.clone(),
                )
                .await?;

            let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
                ThymosError::Configuration("Semaphore closed".to_string())
            })?;

            let input = input.clone();
            let execute = branch.execute.clone();
            let name = branch.name.clone();
            let worktree_manager = self.worktree_manager.clone();
            let commit_changes = self.config.commit_changes;

            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let start = Instant::now();

                let (output, success, error) = match execute(input).await {
                    Ok(out) => (out, true, None),
                    Err(e) => (serde_json::Value::Null, false, Some(e.to_string())),
                };

                // Commit changes if configured
                let commit = if commit_changes && success {
                    worktree_manager
                        .commit_worktree_changes(
                            &worktree_id,
                            &format!("Isolated execution: {}", name),
                        )
                        .await
                        .ok()
                } else {
                    None
                };

                IsolatedBranchResult {
                    name,
                    worktree_id,
                    output,
                    success,
                    error,
                    duration_ms: start.elapsed().as_millis() as u64,
                    commit,
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(IsolatedBranchResult {
                        name: "unknown".to_string(),
                        worktree_id: "unknown".to_string(),
                        output: serde_json::Value::Null,
                        success: false,
                        error: Some(format!("Task panicked: {}", e)),
                        duration_ms: 0,
                        commit: None,
                    });
                }
            }
        }

        // Cleanup worktrees if configured
        if self.config.cleanup {
            for result in &results {
                let _ = self
                    .worktree_manager
                    .remove_worktree(&result.worktree_id, true)
                    .await;
            }
        }

        Ok(results)
    }
}

/// Builder for ParallelIsolated
pub struct ParallelIsolatedBuilder {
    name: String,
    repository: Arc<MemoryRepository>,
    worktree_manager: Option<Arc<MemoryWorktreeManager>>,
    branches: Vec<IsolatedBranch>,
    memory_config: Option<MemoryConfig>,
    config: ParallelIsolatedConfig,
}

impl ParallelIsolatedBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>, repository: Arc<MemoryRepository>) -> Self {
        Self {
            name: name.into(),
            repository,
            worktree_manager: None,
            branches: Vec::new(),
            memory_config: None,
            config: ParallelIsolatedConfig::default(),
        }
    }

    /// Add a branch with a closure
    pub fn add_branch<F, Fut>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        self.branches.push(IsolatedBranch {
            name: name.into(),
            execute: Arc::new(move |input| Box::pin(f(input))),
        });
        self
    }

    /// Set worktree manager
    pub fn with_worktree_manager(mut self, manager: Arc<MemoryWorktreeManager>) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    /// Set memory config
    pub fn with_memory_config(mut self, config: MemoryConfig) -> Self {
        self.memory_config = Some(config);
        self
    }

    /// Set max concurrency
    pub fn with_max_concurrency(mut self, max: usize) -> Self {
        self.config.max_concurrency = max;
        self
    }

    /// Enable merge back
    pub fn merge_back(mut self) -> Self {
        self.config.merge_back = true;
        self
    }

    /// Disable cleanup
    pub fn no_cleanup(mut self) -> Self {
        self.config.cleanup = false;
        self
    }

    /// Build the ParallelIsolated
    pub fn build(self) -> Result<ParallelIsolated> {
        let worktree_manager = self.worktree_manager.unwrap_or_else(|| {
            Arc::new(MemoryWorktreeManager::new(self.repository.clone()))
        });

        let memory_config = self.memory_config.unwrap_or_else(|| {
            MemoryConfig {
                mode: crate::config::MemoryMode::Embedded {
                    data_dir: std::env::temp_dir().join("thymos_isolated"),
                },
                ..Default::default()
            }
        });

        Ok(ParallelIsolated {
            name: self.name,
            repository: self.repository,
            worktree_manager,
            branches: self.branches,
            memory_config,
            config: self.config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_branch_result() {
        let result = IsolatedBranchResult {
            name: "test".to_string(),
            worktree_id: "wt-123".to_string(),
            output: serde_json::json!({"value": 42}),
            success: true,
            error: None,
            duration_ms: 100,
            commit: Some("abc123".to_string()),
        };

        assert!(result.success);
        assert!(result.commit.is_some());
    }

    #[test]
    fn test_config_default() {
        let config = ParallelIsolatedConfig::default();
        assert_eq!(config.max_concurrency, 4);
        assert!(config.commit_changes);
        assert!(!config.merge_back);
        assert!(config.cleanup);
    }
}
