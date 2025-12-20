//! Context Manager Configuration

use serde::{Deserialize, Serialize};

/// Configuration for context management behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum context tokens before compaction
    pub max_tokens: usize,

    /// Ratio at which to trigger summarization (e.g., 0.8 = 80%)
    pub summarize_at_ratio: f64,

    /// Quality threshold below which to trigger rollback (0.0-1.0)
    pub quality_threshold: f64,

    /// Number of turns between automatic checkpoints
    pub checkpoint_interval: usize,

    /// Number of recent memories to ground responses with
    pub grounding_memories: usize,

    /// Number of recent turns to keep verbatim after summarization
    pub recent_turns_to_keep: usize,

    /// Whether to enable automatic summarization
    pub auto_summarize: bool,

    /// Whether to enable automatic checkpointing
    pub auto_checkpoint: bool,

    /// Whether to enable automatic rollback on quality degradation
    pub auto_rollback: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8192,
            summarize_at_ratio: 0.8,
            quality_threshold: 0.6,
            checkpoint_interval: 10,
            grounding_memories: 5,
            recent_turns_to_keep: 3,
            auto_summarize: true,
            auto_checkpoint: true,
            auto_rollback: false,
        }
    }
}

impl ContextConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set summarization ratio threshold
    pub fn with_summarize_at_ratio(mut self, ratio: f64) -> Self {
        self.summarize_at_ratio = ratio.clamp(0.1, 1.0);
        self
    }

    /// Set quality threshold for rollback
    pub fn with_quality_threshold(mut self, threshold: f64) -> Self {
        self.quality_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set checkpoint interval
    pub fn with_checkpoint_interval(mut self, interval: usize) -> Self {
        self.checkpoint_interval = interval.max(1);
        self
    }

    /// Set number of memories to use for grounding
    pub fn with_grounding_memories(mut self, count: usize) -> Self {
        self.grounding_memories = count;
        self
    }

    /// Set number of recent turns to keep after summarization
    pub fn with_recent_turns_to_keep(mut self, count: usize) -> Self {
        self.recent_turns_to_keep = count.max(1);
        self
    }

    /// Enable or disable automatic summarization
    pub fn with_auto_summarize(mut self, enabled: bool) -> Self {
        self.auto_summarize = enabled;
        self
    }

    /// Enable or disable automatic checkpointing
    pub fn with_auto_checkpoint(mut self, enabled: bool) -> Self {
        self.auto_checkpoint = enabled;
        self
    }

    /// Enable or disable automatic rollback
    pub fn with_auto_rollback(mut self, enabled: bool) -> Self {
        self.auto_rollback = enabled;
        self
    }

    /// Calculate the token threshold for triggering summarization
    pub fn summarization_threshold(&self) -> usize {
        ((self.max_tokens as f64) * self.summarize_at_ratio) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ContextConfig::default();
        assert_eq!(config.max_tokens, 8192);
        assert_eq!(config.summarize_at_ratio, 0.8);
        assert_eq!(config.quality_threshold, 0.6);
        assert_eq!(config.checkpoint_interval, 10);
    }

    #[test]
    fn test_config_builder() {
        let config = ContextConfig::new()
            .with_max_tokens(4096)
            .with_summarize_at_ratio(0.7)
            .with_quality_threshold(0.5);

        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.summarize_at_ratio, 0.7);
        assert_eq!(config.quality_threshold, 0.5);
    }

    #[test]
    fn test_ratio_clamping() {
        let config = ContextConfig::new().with_summarize_at_ratio(2.0);
        assert_eq!(config.summarize_at_ratio, 1.0);

        let config = ContextConfig::new().with_summarize_at_ratio(-0.5);
        assert_eq!(config.summarize_at_ratio, 0.1);
    }

    #[test]
    fn test_summarization_threshold() {
        let config = ContextConfig::new()
            .with_max_tokens(10000)
            .with_summarize_at_ratio(0.8);

        assert_eq!(config.summarization_threshold(), 8000);
    }
}
