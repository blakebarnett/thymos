//! Versioning supervisor wrapper

use crate::{AgentSupervisor, AgentMode, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use thymos_core::lifecycle::RelevanceContext;
use thymos_core::memory::versioning::{MemoryRepository, MergeStrategy};
use thymos_core::metrics::MetricsCollector;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::decision::{AutoDecisionEngine, DecisionAction};

/// Configuration for versioning supervisor
#[derive(Clone)]
pub struct VersioningSupervisorConfig {
    /// Enable automatic branching
    pub auto_branching_enabled: bool,
    
    /// Enable automatic rollback
    pub auto_rollback_enabled: bool,
    
    /// Enable automatic merge
    pub auto_merge_enabled: bool,
    
    /// Enable A/B testing
    pub ab_testing_enabled: bool,
    
    /// Default merge strategy
    pub default_merge_strategy: MergeStrategy,
}

impl Default for VersioningSupervisorConfig {
    fn default() -> Self {
        Self {
            auto_branching_enabled: true,
            auto_rollback_enabled: true,
            auto_merge_enabled: true,
            ab_testing_enabled: false,
            default_merge_strategy: MergeStrategy::Theirs, // Use source branch by default
        }
    }
}

/// Supervisor with versioning capabilities
pub struct VersioningSupervisor {
    /// Base supervisor
    supervisor: Arc<dyn AgentSupervisor>,
    
    /// Memory repository (manages branches/worktrees)
    memory_repo: Arc<MemoryRepository>,

    /// Metrics collector
    #[allow(dead_code)]
    metrics: Arc<MetricsCollector>,
    
    /// Auto-decision engine
    decision_engine: Arc<AutoDecisionEngine>,
    
    /// Configuration
    config: VersioningSupervisorConfig,
    
    /// Active experiments (agent_id -> branch_name)
    active_experiments: Arc<RwLock<HashMap<String, String>>>,
}

impl VersioningSupervisor {
    /// Create a new versioning supervisor
    pub fn new(
        supervisor: Arc<dyn AgentSupervisor>,
        memory_repo: Arc<MemoryRepository>,
        metrics: Arc<MetricsCollector>,
        config: VersioningSupervisorConfig,
    ) -> Self {
        let decision_engine = Arc::new(AutoDecisionEngine::new(metrics.clone()));
        
        Self {
            supervisor,
            memory_repo,
            metrics,
            decision_engine,
            config,
            active_experiments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Automatically create experiment branch when performance drops
    pub async fn auto_create_experiment_branch(
        &self,
        agent_id: &str,
        reason: &str,
    ) -> Result<String> {
        if !self.config.auto_branching_enabled {
            return Err(crate::SupervisorError::Supervisor(
                "Auto-branching is disabled".to_string(),
            ));
        }

        // Check if should experiment
        if !self.decision_engine.should_experiment(agent_id).await? {
            return Err(crate::SupervisorError::Supervisor(
                "Agent does not meet criteria for experimentation".to_string(),
            ));
        }

        // Create branch name
        let branch_name = format!("experiment-{}-{}", agent_id, Utc::now().timestamp());
        
        // Create branch from current state
        let _branch = self.memory_repo.create_branch(
            &branch_name,
            Some(&format!("Auto-created experiment branch: {}", reason)),
        ).await.map_err(|e| crate::SupervisorError::Supervisor(e.to_string()))?;
        
        // Track active experiment
        self.active_experiments.write().await.insert(
            agent_id.to_string(),
            branch_name.clone(),
        );
        
        info!("Auto-created experiment branch '{}' for agent {}", branch_name, agent_id);
        
        Ok(branch_name)
    }

    /// Automatically rollback failed experiment
    pub async fn auto_rollback_on_failure(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        if !self.config.auto_rollback_enabled {
            return Err(crate::SupervisorError::Supervisor(
                "Auto-rollback is disabled".to_string(),
            ));
        }

        // Check if should rollback
        if !self.decision_engine.should_rollback(agent_id, branch_name).await? {
            return Ok(()); // No rollback needed
        }

        warn!("Agent {} on branch {} failed, rolling back", agent_id, branch_name);
        
        // Get current branch head
        let current_branch = self.memory_repo.get_current_branch().await;
        
        // Switch back to main branch
        if current_branch != "main" {
            // Would need agent instance to checkout - simplified for now
            // self.memory_repo.checkout_branch("main", agent).await?;
        }
        
        // Delete failed branch
        self.memory_repo.delete_branch(branch_name, true).await
            .map_err(|e| crate::SupervisorError::Supervisor(e.to_string()))?;
        
        // Remove from active experiments
        self.active_experiments.write().await.remove(agent_id);
        
        info!("Rolled back agent {} from branch {}", agent_id, branch_name);
        
        Ok(())
    }

    /// Automatically merge successful experiment
    pub async fn auto_merge_on_success(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        if !self.config.auto_merge_enabled {
            return Err(crate::SupervisorError::Supervisor(
                "Auto-merge is disabled".to_string(),
            ));
        }

        // Check if should merge
        if !self.decision_engine.should_merge(agent_id, branch_name).await? {
            return Ok(()); // Not ready to merge
        }

        info!("Merging successful experiment branch '{}' for agent {}", branch_name, agent_id);
        
        // Merge branch to main
        let merge_result = self.memory_repo.merge(
            branch_name,
            "main",
            self.config.default_merge_strategy.clone(),
        ).await.map_err(|e| crate::SupervisorError::Supervisor(e.to_string()))?;
        
        match merge_result {
            thymos_core::memory::versioning::MergeResult::Conflicts { conflicts } => {
                warn!("Merge conflicts detected for branch '{}', manual resolution required", branch_name);
                return Err(crate::SupervisorError::Supervisor(
                    format!("Merge conflicts detected: {} conflicts", conflicts.len()),
                ));
            }
            thymos_core::memory::versioning::MergeResult::Success { .. } => {
                // Merge successful
            }
        }
        
        // Delete merged branch (optional - could keep for history)
        self.memory_repo.delete_branch(branch_name, true).await
            .map_err(|e| crate::SupervisorError::Supervisor(e.to_string()))?;
        
        // Remove from active experiments
        self.active_experiments.write().await.remove(agent_id);
        
        info!("Successfully merged branch '{}' to main for agent {}", branch_name, agent_id);
        
        Ok(())
    }

    /// Monitor agent and make automatic decisions
    pub async fn monitor_agent(&self, agent_id: &str) -> Result<Vec<DecisionAction>> {
        // Evaluate all rules
        let actions = self.decision_engine.evaluate(agent_id).await?;
        
        // Execute actions
        for action in &actions {
            match action {
                DecisionAction::CreateBranch { description, .. } => {
                    if let Err(e) = self.auto_create_experiment_branch(agent_id, description).await {
                        warn!("Failed to create branch for agent {}: {}", agent_id, e);
                    }
                }
                DecisionAction::Rollback { branch_name, .. } => {
                    if let Err(e) = self.auto_rollback_on_failure(agent_id, branch_name).await {
                        warn!("Failed to rollback agent {}: {}", agent_id, e);
                    }
                }
                DecisionAction::MergeBranch { source_branch } => {
                    if let Err(e) = self.auto_merge_on_success(agent_id, source_branch).await {
                        warn!("Failed to merge branch for agent {}: {}", agent_id, e);
                    }
                }
                _ => {
                    // Other actions would need additional implementation
                    info!("Action {:?} not yet implemented", action);
                }
            }
        }
        
        Ok(actions)
    }

    /// Get active experiments
    pub async fn get_active_experiments(&self) -> HashMap<String, String> {
        self.active_experiments.read().await.clone()
    }
}

// Delegate supervisor methods to underlying supervisor
#[async_trait::async_trait]
impl AgentSupervisor for VersioningSupervisor {
    async fn start(
        &self,
        agent_id: &str,
        mode: AgentMode,
        context: &RelevanceContext,
    ) -> Result<crate::AgentHandle> {
        self.supervisor.start(agent_id, mode, context).await
    }

    async fn stop(&self, agent_id: &str, save_state: bool) -> Result<()> {
        self.supervisor.stop(agent_id, save_state).await
    }

    async fn get_status(&self, agent_id: &str) -> Result<thymos_core::agent::AgentStatus> {
        self.supervisor.get_status(agent_id).await
    }

    async fn set_mode(&self, agent_id: &str, mode: AgentMode) -> Result<()> {
        self.supervisor.set_mode(agent_id, mode).await
    }

    async fn list_agents(&self) -> Result<Vec<String>> {
        self.supervisor.list_agents().await
    }

    async fn health_check(&self, agent_id: &str) -> Result<crate::HealthStatus> {
        self.supervisor.health_check(agent_id).await
    }
}

