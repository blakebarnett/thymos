//! Game-specific relevance evaluator for Zera NPCs

use async_trait::async_trait;
use thymos_core::error::Result;
use thymos_core::lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceScore};

/// Zera-specific relevance evaluator
pub struct ZeraRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for ZeraRelevanceEvaluator {
    async fn evaluate(
        &self,
        _agent_id: &str,
        context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        // Extract Zera-specific context
        let in_party: bool = context.get("in_party").unwrap_or(false);
        let zones_away: i32 = context.get("zones_away").unwrap_or(999);
        let last_interaction_turns: Option<i32> = context.get("last_interaction_turns");
        let in_active_quest: bool = context.get("in_active_quest").unwrap_or(false);
        let mentioned_recently: bool = context.get("mentioned_recently").unwrap_or(false);

        // Calculate relevance score
        let score = if in_party {
            1.0 // Always active if in party
        } else if let Some(turns) = last_interaction_turns {
            if turns < 3 {
                1.0 // Recent interaction
            } else {
                (0.5 - (turns as f64 * 0.05)).max(0.0) // Decay over time
            }
        } else if zones_away == 0 {
            0.8 // Same location
        } else if in_active_quest && zones_away <= 2 {
            0.7 // Quest-relevant and nearby
        } else if zones_away <= 3 || mentioned_recently {
            0.4 // Nearby or mentioned
        } else if in_active_quest {
            0.2 // Quest-relevant but distant
        } else {
            0.05 // Not relevant
        };

        Ok(RelevanceScore::new(score))
    }
}
