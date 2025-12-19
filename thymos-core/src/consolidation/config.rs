use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Scope for consolidation operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsolidationScope {
    /// Consolidate memories from a specific session
    Session(String),

    /// Consolidate memories within a time range
    TimeRange {
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    },

    /// Consolidate memories with a specific tag
    Tagged(String),
}

/// Configuration for the consolidation engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    /// Minimum number of memories before consolidation (default: 10)
    pub min_memories: usize,

    /// Time window for consolidation (default: 1 hour)
    pub consolidation_window: Duration,

    /// Batch size for processing memories (default: 50)
    pub batch_size: usize,

    /// Whether to generate insights (requires LLM) (default: false for MVP)
    pub generate_insights: bool,

    /// Whether to update importance scores (default: true)
    pub update_importance_scores: bool,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            min_memories: 10,
            consolidation_window: Duration::from_secs(3600), // 1 hour
            batch_size: 50,
            generate_insights: false,
            update_importance_scores: true,
        }
    }
}

impl ConsolidationConfig {
    /// Create a new consolidation configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum number of memories before consolidation.
    pub fn with_min_memories(mut self, min: usize) -> Self {
        self.min_memories = min;
        self
    }

    /// Set the consolidation time window.
    pub fn with_consolidation_window(mut self, duration: Duration) -> Self {
        self.consolidation_window = duration;
        self
    }

    /// Set the batch size for processing.
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size.max(1); // Ensure at least 1
        self
    }

    /// Enable or disable insight generation.
    pub fn with_generate_insights(mut self, enabled: bool) -> Self {
        self.generate_insights = enabled;
        self
    }

    /// Enable or disable importance score updates.
    pub fn with_update_importance_scores(mut self, enabled: bool) -> Self {
        self.update_importance_scores = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.min_memories, 10);
        assert_eq!(config.batch_size, 50);
        assert!(!config.generate_insights);
    }

    #[test]
    fn test_config_builder() {
        let config = ConsolidationConfig::new()
            .with_min_memories(20)
            .with_batch_size(100)
            .with_generate_insights(true);

        assert_eq!(config.min_memories, 20);
        assert_eq!(config.batch_size, 100);
        assert!(config.generate_insights);
    }

    #[test]
    fn test_batch_size_minimum() {
        let config = ConsolidationConfig::new().with_batch_size(0);
        assert_eq!(config.batch_size, 1);
    }

    #[test]
    fn test_consolidation_scope() {
        let session = ConsolidationScope::Session("session_123".to_string());
        assert!(matches!(session, ConsolidationScope::Session(_)));

        let tagged = ConsolidationScope::Tagged("important".to_string());
        assert!(matches!(tagged, ConsolidationScope::Tagged(_)));
    }
}
