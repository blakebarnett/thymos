//! Coordinator agent for multi-agent coordination example

use async_trait::async_trait;
use std::sync::Arc;
use thymos_core::lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceScore};
use thymos_core::prelude::*;

/// Simple relevance evaluator for coordination
pub struct SimpleRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for SimpleRelevanceEvaluator {
    async fn evaluate(
        &self,
        _agent_id: &str,
        _context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        // Simple evaluator: always return moderate relevance
        Ok(RelevanceScore::new(0.6))
    }
}

/// Coordinator agent that monitors and coordinates other agents
pub struct CoordinatorAgent {
    /// Agent IDs to coordinate
    pub agent_ids: Vec<String>,

    /// Relevance evaluator
    pub evaluator: Arc<dyn RelevanceEvaluator>,
}

impl CoordinatorAgent {
    /// Create a new coordinator agent
    pub fn new(agent_ids: Vec<String>) -> Self {
        Self {
            agent_ids,
            evaluator: Arc::new(SimpleRelevanceEvaluator),
        }
    }

    /// Build relevance context for an agent (simplified)
    pub async fn build_context(&self, _agent_id: &str) -> RelevanceContext {
        let mut context = RelevanceContext::new();
        // Simplified context - in a real scenario, this would query game state
        context.set("active", true);
        context.set("recent_activity", true);
        context
    }

    /// Coordinate agents by evaluating their relevance
    pub async fn coordinate(&self) -> Result<()> {
        println!("Coordinating {} agents...", self.agent_ids.len());

        for agent_id in &self.agent_ids {
            let context = self.build_context(agent_id).await;
            let relevance = self.evaluator.evaluate(agent_id, &context).await?;

            println!(
                "  Agent {} relevance: {:.2} ({:?})",
                agent_id,
                relevance.value(),
                relevance.to_status(&thymos_core::lifecycle::RelevanceThresholds::default())
            );
        }

        Ok(())
    }
}
