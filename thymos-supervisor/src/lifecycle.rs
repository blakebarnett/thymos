//! Agent lifecycle manager that bridges relevance evaluation with process management

use crate::{AgentSupervisor, AgentMode, ReconciliationReport, Result};
use thymos_core::{
    agent::AgentStatus,
    lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceThresholds},
};
use std::sync::Arc;
use tracing::{debug, info};

/// Manages agent lifecycle (start/stop/transition) based on relevance
pub struct AgentLifecycleManager {
    supervisor: Arc<dyn AgentSupervisor>,
    evaluator: Arc<dyn RelevanceEvaluator>,
    thresholds: RelevanceThresholds,
}

impl AgentLifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(
        supervisor: Arc<dyn AgentSupervisor>,
        evaluator: Arc<dyn RelevanceEvaluator>,
        thresholds: RelevanceThresholds,
    ) -> Self {
        Self {
            supervisor,
            evaluator,
            thresholds,
        }
    }
    
    /// Reconcile agent states based on current context
    ///
    /// Evaluates relevance for all agents and transitions them to the appropriate
    /// state (Active, Listening, Dormant, Archived) based on relevance scores.
    pub async fn reconcile(
        &self,
        context: &RelevanceContext,
    ) -> Result<ReconciliationReport> {
        let mut report = ReconciliationReport::default();
        
        // Get all known agents
        let agents = self.supervisor.list_agents().await?;
        
        debug!("Reconciling {} agents", agents.len());
        
        for agent_id in agents {
            let relevance = self.evaluator.evaluate(&agent_id, context).await?;
            let current_status = self.supervisor.get_status(&agent_id).await?;
            let desired_status = relevance.to_status(&self.thresholds);
            
            if current_status != desired_status {
                self.transition_agent(
                    &agent_id,
                    current_status,
                    desired_status,
                    context,
                    &mut report,
                ).await?;
            }
        }
        
        if !report.started.is_empty() || !report.stopped.is_empty() {
            info!(
                "Reconciliation complete: started={:?}, stopped={:?}, upgraded={:?}, downgraded={:?}",
                report.started, report.stopped, report.upgraded, report.downgraded
            );
        }
        
        Ok(report)
    }
    
    async fn transition_agent(
        &self,
        agent_id: &str,
        from: AgentStatus,
        to: AgentStatus,
        context: &RelevanceContext,
        report: &mut ReconciliationReport,
    ) -> Result<()> {
        debug!(
            "Transitioning agent {}: {:?} → {:?}",
            agent_id, from, to
        );
        
        match (from, to) {
            (AgentStatus::Dormant | AgentStatus::Archived, AgentStatus::Active) => {
                self.supervisor.start(agent_id, AgentMode::Active, context).await?;
                report.started.push(agent_id.to_string());
            }
            
            (AgentStatus::Dormant | AgentStatus::Archived, AgentStatus::Listening) => {
                self.supervisor.start(agent_id, AgentMode::Passive, context).await?;
                report.started.push(agent_id.to_string());
            }
            
            (AgentStatus::Active | AgentStatus::Listening, AgentStatus::Dormant | AgentStatus::Archived) => {
                self.supervisor.stop(agent_id, true).await?;
                report.stopped.push(agent_id.to_string());
            }
            
            (AgentStatus::Active, AgentStatus::Listening) => {
                self.supervisor.set_mode(agent_id, AgentMode::Passive).await?;
                report.downgraded.push(agent_id.to_string());
            }
            
            (AgentStatus::Listening, AgentStatus::Active) => {
                self.supervisor.set_mode(agent_id, AgentMode::Active).await?;
                report.upgraded.push(agent_id.to_string());
            }
            
            _ => {
                // No-op for other transitions
                debug!("No action needed for transition {:?} → {:?}", from, to);
            }
        }
        
        Ok(())
    }
}

