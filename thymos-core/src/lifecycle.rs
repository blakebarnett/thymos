//! Agent lifecycle and relevance management

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Context for relevance evaluation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelevanceContext {
    /// Domain-specific properties (extensible)
    pub properties: HashMap<String, serde_json::Value>,
}

impl RelevanceContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a property
    pub fn set(&mut self, key: impl Into<String>, value: impl Serialize) {
        self.properties.insert(
            key.into(),
            serde_json::to_value(value).expect("Failed to serialize value"),
        );
    }

    /// Get a property
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.properties
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Relevance score for an agent (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RelevanceScore(f64);

impl RelevanceScore {
    /// Create a new relevance score (clamped to 0.0-1.0)
    pub fn new(score: f64) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    /// Get the score value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Convert to agent status based on thresholds
    pub fn to_status(&self, thresholds: &RelevanceThresholds) -> crate::agent::AgentStatus {
        use crate::agent::AgentStatus;

        if self.0 >= thresholds.active {
            AgentStatus::Active
        } else if self.0 >= thresholds.listening {
            AgentStatus::Listening
        } else if self.0 >= thresholds.dormant {
            AgentStatus::Dormant
        } else {
            AgentStatus::Archived
        }
    }
}

/// Thresholds for relevance-based status transitions
#[derive(Debug, Clone)]
pub struct RelevanceThresholds {
    /// Threshold for active status (>= 0.7)
    pub active: f64,

    /// Threshold for listening status (>= 0.4)
    pub listening: f64,

    /// Threshold for dormant status (>= 0.1)
    pub dormant: f64,
}

impl Default for RelevanceThresholds {
    fn default() -> Self {
        Self {
            active: 0.7,
            listening: 0.4,
            dormant: 0.1,
        }
    }
}

/// Trait for evaluating agent relevance
#[async_trait]
pub trait RelevanceEvaluator: Send + Sync {
    /// Calculate relevance score for an agent
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore>;
}

/// Default relevance evaluator (always returns 0.5)
pub struct DefaultRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for DefaultRelevanceEvaluator {
    async fn evaluate(
        &self,
        _agent_id: &str,
        _context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        Ok(RelevanceScore::new(0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relevance_score() {
        let score = RelevanceScore::new(0.8);
        assert_eq!(score.value(), 0.8);

        // Test clamping
        let too_high = RelevanceScore::new(1.5);
        assert_eq!(too_high.value(), 1.0);

        let too_low = RelevanceScore::new(-0.5);
        assert_eq!(too_low.value(), 0.0);
    }

    #[test]
    fn test_relevance_to_status() {
        use crate::agent::AgentStatus;

        let thresholds = RelevanceThresholds::default();

        assert_eq!(
            RelevanceScore::new(0.9).to_status(&thresholds),
            AgentStatus::Active
        );
        assert_eq!(
            RelevanceScore::new(0.5).to_status(&thresholds),
            AgentStatus::Listening
        );
        assert_eq!(
            RelevanceScore::new(0.2).to_status(&thresholds),
            AgentStatus::Dormant
        );
        assert_eq!(
            RelevanceScore::new(0.05).to_status(&thresholds),
            AgentStatus::Archived
        );
    }

    #[test]
    fn test_relevance_context() {
        let mut context = RelevanceContext::new();

        context.set("in_party", true);
        context.set("distance", 5);

        assert_eq!(context.get::<bool>("in_party"), Some(true));
        assert_eq!(context.get::<i32>("distance"), Some(5));
        assert_eq!(context.get::<String>("missing"), None);
    }

    #[tokio::test]
    async fn test_default_evaluator() {
        let evaluator = DefaultRelevanceEvaluator;
        let context = RelevanceContext::new();

        let score = evaluator
            .evaluate("test_agent", &context)
            .await
            .expect("Evaluation failed");

        assert_eq!(score.value(), 0.5);
    }
}
