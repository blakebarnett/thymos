//! Quality Scoring for Context Management
//!
//! Provides heuristic and optional LLM-based quality scoring to detect
//! context degradation in long-running conversations.

use crate::error::Result;
use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// State information used for quality calculation
#[derive(Debug, Clone, Default)]
pub struct ContextState {
    /// Average relevance score of retrieved memories (0.0-1.0)
    pub avg_memory_relevance: f64,

    /// Last coherence score from LLM evaluation (0.0-1.0)
    pub last_coherence_score: Option<f64>,

    /// Total number of turns in the session
    pub turn_count: usize,

    /// Number of summarizations performed
    pub summarization_count: usize,

    /// Estimated current token count
    pub token_count: usize,

    /// Maximum token limit
    pub max_tokens: usize,
}

impl ContextState {
    /// Create a new context state
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            ..Default::default()
        }
    }

    /// Update memory relevance from search results
    pub fn update_memory_relevance(&mut self, relevance_scores: &[f64]) {
        if relevance_scores.is_empty() {
            return;
        }
        self.avg_memory_relevance =
            relevance_scores.iter().sum::<f64>() / relevance_scores.len() as f64;
    }

    /// Update coherence score from LLM evaluation
    pub fn update_coherence(&mut self, score: f64) {
        self.last_coherence_score = Some(score.clamp(0.0, 1.0));
    }

    /// Increment turn count
    pub fn increment_turns(&mut self) {
        self.turn_count += 1;
    }

    /// Increment summarization count
    pub fn increment_summarizations(&mut self) {
        self.summarization_count += 1;
    }

    /// Update token count
    pub fn update_tokens(&mut self, count: usize) {
        self.token_count = count;
    }

    /// Calculate token utilization ratio
    pub fn token_utilization(&self) -> f64 {
        if self.max_tokens == 0 {
            return 0.0;
        }
        (self.token_count as f64 / self.max_tokens as f64).min(1.0)
    }
}

/// Configuration for quality scorer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScorerConfig {
    /// Weight for memory retrieval relevance in final score
    pub memory_relevance_weight: f64,

    /// Weight for response coherence in final score
    pub coherence_weight: f64,

    /// Weight for session health (turns vs summarizations) in final score
    pub session_health_weight: f64,

    /// Weight for token utilization (lower is better) in final score
    pub token_utilization_weight: f64,

    /// Whether to use LLM for coherence evaluation
    pub use_llm_coherence: bool,
}

impl Default for QualityScorerConfig {
    fn default() -> Self {
        Self {
            memory_relevance_weight: 0.3,
            coherence_weight: 0.3,
            session_health_weight: 0.2,
            token_utilization_weight: 0.2,
            use_llm_coherence: false,
        }
    }
}

/// Quality scorer for context management
pub struct QualityScorer {
    config: QualityScorerConfig,
    llm: Option<Arc<dyn LLMProvider>>,
}

impl QualityScorer {
    /// Create a new quality scorer with heuristic-only scoring
    pub fn new(config: QualityScorerConfig) -> Self {
        Self { config, llm: None }
    }

    /// Create a quality scorer with optional LLM-based coherence evaluation
    pub fn with_llm(config: QualityScorerConfig, llm: Arc<dyn LLMProvider>) -> Self {
        Self {
            config,
            llm: Some(llm),
        }
    }

    /// Calculate overall quality score (0.0-1.0)
    pub fn calculate(&self, state: &ContextState) -> f64 {
        let memory_score = state.avg_memory_relevance;

        let coherence_score = state.last_coherence_score.unwrap_or(1.0);

        // Session health: penalize high turn count relative to summarizations
        // More summarizations per turn = better context management
        let session_score = if state.turn_count == 0 {
            1.0
        } else {
            let summarization_ratio =
                state.summarization_count as f64 / state.turn_count.max(1) as f64;
            // Ideal: roughly 1 summarization per 10 turns
            let ideal_ratio = 0.1;
            1.0 - (summarization_ratio - ideal_ratio).abs().min(1.0)
        };

        // Token utilization: lower is better (more headroom)
        let token_score = 1.0 - state.token_utilization();

        // Weighted combination
        let score = (memory_score * self.config.memory_relevance_weight)
            + (coherence_score * self.config.coherence_weight)
            + (session_score * self.config.session_health_weight)
            + (token_score * self.config.token_utilization_weight);

        score.clamp(0.0, 1.0)
    }

    /// Evaluate coherence using LLM (if configured)
    pub async fn evaluate_coherence(
        &self,
        recent_messages: &[Message],
        last_response: &str,
    ) -> Result<Option<f64>> {
        let Some(llm) = &self.llm else {
            return Ok(None);
        };

        if !self.config.use_llm_coherence {
            return Ok(None);
        }

        // Build context from recent messages
        let context: String = recent_messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Evaluate the coherence of this conversation response on a scale of 0.0 to 1.0.

Consider:
- Does the response logically follow from the conversation?
- Is the response consistent with earlier context?
- Does the response address what was asked?

Conversation context:
{}

Response to evaluate:
{}

Return ONLY a decimal number between 0.0 and 1.0, nothing else."#,
            context, last_response
        );

        let request = LLMRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt,
            }],
            temperature: Some(0.0),
            max_tokens: Some(10),
            stop_sequences: vec![],
        };

        let response = llm.generate_request(&request).await?;
        let score: f64 = response
            .content
            .trim()
            .parse()
            .unwrap_or(0.7); // Default to neutral if parsing fails

        Ok(Some(score.clamp(0.0, 1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_state_defaults() {
        let state = ContextState::new(8192);
        assert_eq!(state.max_tokens, 8192);
        assert_eq!(state.turn_count, 0);
        assert_eq!(state.avg_memory_relevance, 0.0);
    }

    #[test]
    fn test_memory_relevance_update() {
        let mut state = ContextState::new(8192);
        state.update_memory_relevance(&[0.8, 0.9, 0.7]);
        assert!((state.avg_memory_relevance - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_token_utilization() {
        let mut state = ContextState::new(10000);
        state.update_tokens(5000);
        assert_eq!(state.token_utilization(), 0.5);

        state.update_tokens(15000);
        assert_eq!(state.token_utilization(), 1.0); // Capped at 1.0
    }

    #[test]
    fn test_quality_scorer_perfect_state() {
        let scorer = QualityScorer::new(QualityScorerConfig::default());
        let mut state = ContextState::new(10000);
        state.avg_memory_relevance = 1.0;
        state.last_coherence_score = Some(1.0);
        state.turn_count = 10;
        state.summarization_count = 1;
        state.token_count = 0;

        let score = scorer.calculate(&state);
        assert!(score > 0.8);
    }

    #[test]
    fn test_quality_scorer_degraded_state() {
        let scorer = QualityScorer::new(QualityScorerConfig::default());
        let mut state = ContextState::new(10000);
        state.avg_memory_relevance = 0.3;
        state.last_coherence_score = Some(0.4);
        state.turn_count = 100;
        state.summarization_count = 0;
        state.token_count = 9500;

        let score = scorer.calculate(&state);
        assert!(score < 0.5);
    }

    #[test]
    fn test_quality_scorer_empty_state() {
        let scorer = QualityScorer::new(QualityScorerConfig::default());
        let state = ContextState::new(10000);

        let score = scorer.calculate(&state);
        // Should be reasonable score for fresh state
        assert!(score > 0.3 && score < 1.0);
    }
}
