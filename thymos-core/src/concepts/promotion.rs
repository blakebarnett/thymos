use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;

use super::tier::ConceptTier;

/// Configuration for the concept promotion pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionConfig {
    /// Significance threshold for automatic promotion (0.0-1.0, default: 0.6)
    pub promotion_threshold: f64,

    /// Minimum mention count before considering promotion to Provisional (default: 2)
    pub min_mentions_provisional: usize,

    /// Minimum mention count before promoting to Tracked (default: 5)
    pub min_mentions_tracked: usize,

    /// Whether to use LLM for validation (default: false for MVP)
    pub use_llm_validation: bool,

    /// Time window (in seconds) to consider mentions as recent (default: 86400 = 1 day)
    pub recency_window_secs: u64,
}

impl Default for PromotionConfig {
    fn default() -> Self {
        Self {
            promotion_threshold: 0.6,
            min_mentions_provisional: 2,
            min_mentions_tracked: 5,
            use_llm_validation: false,
            recency_window_secs: 86400,
        }
    }
}

impl PromotionConfig {
    /// Create a new promotion configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the promotion threshold.
    pub fn with_promotion_threshold(mut self, threshold: f64) -> Self {
        self.promotion_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set minimum mentions for Provisional tier.
    pub fn with_min_mentions_provisional(mut self, count: usize) -> Self {
        self.min_mentions_provisional = count;
        self
    }

    /// Set minimum mentions for Tracked tier.
    pub fn with_min_mentions_tracked(mut self, count: usize) -> Self {
        self.min_mentions_tracked = count;
        self
    }

    /// Enable or disable LLM validation.
    pub fn with_llm_validation(mut self, enabled: bool) -> Self {
        self.use_llm_validation = enabled;
        self
    }
}

/// Information about a single mention of a concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptMention {
    /// ID of the memory where this mention occurred
    pub memory_id: String,

    /// Timestamp of the mention
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Contextual text around the mention
    pub context: String,

    /// Significance score at time of mention
    pub significance: f64,
}

impl ConceptMention {
    /// Create a new concept mention.
    pub fn new(
        memory_id: impl Into<String>,
        context: impl Into<String>,
        significance: f64,
    ) -> Self {
        Self {
            memory_id: memory_id.into(),
            timestamp: chrono::Utc::now(),
            context: context.into(),
            significance,
        }
    }
}

/// Promotion statistics for a concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionStats {
    /// Current tier
    pub tier: ConceptTier,

    /// Total number of mentions
    pub mention_count: usize,

    /// Average significance of mentions
    pub avg_significance: f64,

    /// Peak significance of mentions
    pub peak_significance: f64,

    /// When the concept was first mentioned
    pub first_mention: chrono::DateTime<chrono::Utc>,

    /// When the concept was last mentioned
    pub last_mention: chrono::DateTime<chrono::Utc>,

    /// Mentions in the recency window
    pub recent_mention_count: usize,
}

impl PromotionStats {
    /// Create initial promotion stats for a new concept.
    pub fn new_initial(significance: f64) -> Self {
        let now = chrono::Utc::now();
        Self {
            tier: ConceptTier::Mentioned,
            mention_count: 1,
            avg_significance: significance,
            peak_significance: significance,
            first_mention: now,
            last_mention: now,
            recent_mention_count: 1,
        }
    }

    /// Record a new mention and update statistics.
    pub fn record_mention(&mut self, significance: f64) {
        self.mention_count += 1;
        self.avg_significance = (self.avg_significance * (self.mention_count - 1) as f64
            + significance)
            / self.mention_count as f64;
        self.peak_significance = self.peak_significance.max(significance);
        self.last_mention = chrono::Utc::now();
        self.recent_mention_count += 1;
    }

    /// Determine promotion tier based on criteria.
    pub fn determine_tier(&self, config: &PromotionConfig) -> ConceptTier {
        // Check for Tracked tier
        if self.peak_significance >= config.promotion_threshold
            && self.mention_count >= config.min_mentions_tracked
        {
            return ConceptTier::Tracked;
        }

        // Check for Provisional tier
        if self.peak_significance >= config.promotion_threshold * 0.8
            && self.mention_count >= config.min_mentions_provisional
        {
            return ConceptTier::Provisional;
        }

        ConceptTier::Mentioned
    }
}

/// Concept promotion pipeline for tracking and promoting concepts.
pub struct ConceptPromotionPipeline {
    config: PromotionConfig,
    // In-memory tracking of concepts and their mentions
    concepts: Arc<std::sync::Mutex<HashMap<String, PromotionStats>>>,
    mention_history: Arc<std::sync::Mutex<HashMap<String, Vec<ConceptMention>>>>,
}

impl ConceptPromotionPipeline {
    /// Create a new concept promotion pipeline.
    pub fn new(config: PromotionConfig) -> Self {
        Self {
            config,
            concepts: Arc::new(std::sync::Mutex::new(HashMap::new())),
            mention_history: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Track a mention of a concept.
    pub async fn track_mention(
        &self,
        concept_text: &str,
        memory_id: &str,
        context: &str,
        significance: f64,
    ) -> Result<ConceptTier> {
        let mut concepts = self.concepts.lock().map_err(|e| {
            crate::error::ThymosError::Agent(format!("Failed to lock concepts: {}", e))
        })?;

        let mut mention_history = self.mention_history.lock().map_err(|e| {
            crate::error::ThymosError::Agent(format!("Failed to lock mention history: {}", e))
        })?;

        // Initialize or update stats
        let concept_key = concept_text.to_string();
        if !concepts.contains_key(&concept_key) {
            concepts.insert(
                concept_key.clone(),
                PromotionStats::new_initial(significance),
            );
        } else {
            let stats = concepts.get_mut(&concept_key).unwrap();
            stats.record_mention(significance);
        }

        // Record the mention
        let mention = ConceptMention::new(memory_id, context, significance);
        mention_history
            .entry(concept_text.to_string())
            .or_insert_with(Vec::new)
            .push(mention);

        // Determine new tier
        let stats = concepts.get_mut(&concept_key).unwrap();
        let new_tier = stats.determine_tier(&self.config);
        stats.tier = new_tier;

        Ok(new_tier)
    }

    /// Get the promotion statistics for a concept.
    pub fn get_stats(&self, concept_text: &str) -> Result<Option<PromotionStats>> {
        let concepts = self.concepts.lock().map_err(|e| {
            crate::error::ThymosError::Agent(format!("Failed to lock concepts: {}", e))
        })?;

        Ok(concepts.get(concept_text).cloned())
    }

    /// Get the current tier for a concept.
    pub fn get_tier(&self, concept_text: &str) -> Result<Option<ConceptTier>> {
        Ok(self.get_stats(concept_text)?.map(|s| s.tier))
    }

    /// Get the mention count for a concept.
    pub fn get_mention_count(&self, concept_text: &str) -> Result<usize> {
        Ok(self
            .get_stats(concept_text)?
            .map(|s| s.mention_count)
            .unwrap_or(0))
    }

    /// Get all tracked concepts.
    pub fn get_all_concepts(&self) -> Result<Vec<(String, ConceptTier)>> {
        let concepts = self.concepts.lock().map_err(|e| {
            crate::error::ThymosError::Agent(format!("Failed to lock concepts: {}", e))
        })?;

        Ok(concepts
            .iter()
            .map(|(text, stats)| (text.clone(), stats.tier))
            .collect())
    }

    /// Get the mention history for a concept.
    pub fn get_mention_history(&self, concept_text: &str) -> Result<Vec<ConceptMention>> {
        let history = self.mention_history.lock().map_err(|e| {
            crate::error::ThymosError::Agent(format!("Failed to lock mention history: {}", e))
        })?;

        Ok(history.get(concept_text).cloned().unwrap_or_default())
    }

    /// Clear all tracking data (useful for testing).
    pub fn clear(&self) -> Result<()> {
        self.concepts
            .lock()
            .map_err(|e| {
                crate::error::ThymosError::Agent(format!("Failed to lock concepts: {}", e))
            })?
            .clear();

        self.mention_history
            .lock()
            .map_err(|e| {
                crate::error::ThymosError::Agent(format!("Failed to lock mention history: {}", e))
            })?
            .clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promotion_stats_new() {
        let stats = PromotionStats::new_initial(0.8);
        assert_eq!(stats.tier, ConceptTier::Mentioned);
        assert_eq!(stats.mention_count, 1);
        assert_eq!(stats.avg_significance, 0.8);
    }

    #[test]
    fn test_promotion_stats_record() {
        let mut stats = PromotionStats::new_initial(0.7);
        stats.record_mention(0.9);

        assert_eq!(stats.mention_count, 2);
        assert_eq!(stats.peak_significance, 0.9);
        assert!((stats.avg_significance - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_determine_tier() {
        let config = PromotionConfig::default();
        let mut stats = PromotionStats::new_initial(0.8);

        // Initially Mentioned
        assert_eq!(stats.determine_tier(&config), ConceptTier::Mentioned);

        // Record enough mentions for Provisional
        for _ in 0..2 {
            stats.record_mention(0.7);
        }
        assert_eq!(stats.determine_tier(&config), ConceptTier::Provisional);

        // Record enough mentions for Tracked
        for _ in 0..3 {
            stats.record_mention(0.8);
        }
        assert_eq!(stats.determine_tier(&config), ConceptTier::Tracked);
    }

    #[tokio::test]
    async fn test_pipeline_tracking() {
        let config = PromotionConfig::new()
            .with_min_mentions_provisional(3)
            .with_min_mentions_tracked(6);
        let pipeline = ConceptPromotionPipeline::new(config);

        // Track mentions
        let tier1 = pipeline
            .track_mention("Elder Rowan", "mem1", "Elder Rowan lived", 0.8)
            .await
            .unwrap();
        assert_eq!(tier1, ConceptTier::Mentioned);

        let tier2 = pipeline
            .track_mention("Elder Rowan", "mem2", "Elder Rowan was wise", 0.85)
            .await
            .unwrap();
        assert_eq!(tier2, ConceptTier::Mentioned); // Still only 2 mentions

        // Add more mentions to reach provisional
        let tier3 = pipeline
            .track_mention("Elder Rowan", "mem3", "Elder Rowan was respected", 0.8)
            .await
            .unwrap();
        assert_eq!(tier3, ConceptTier::Provisional);

        // Verify stats
        let stats = pipeline.get_stats("Elder Rowan").unwrap();
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.mention_count, 3);
        assert!(stats.avg_significance > 0.80);
    }
}
