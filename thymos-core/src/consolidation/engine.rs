use std::sync::Arc;

use crate::error::{Result, ThymosError};
use crate::llm::{LLMProvider, LLMRequest};
use crate::memory::{MemorySystem, RememberOptions};
use locai::models::Memory;

use super::config::{ConsolidationConfig, ConsolidationScope};
use super::insights::Insight;

/// Memory consolidation engine.
///
/// The consolidation engine periodically processes memories to generate insights,
/// identify patterns, and update importance scores. It operates with a configurable
/// LLM provider for insight generation.
///
/// # Example
///
/// ```rust,ignore
/// use thymos_core::consolidation::{ConsolidationEngine, ConsolidationConfig, ConsolidationScope};
///
/// let engine = ConsolidationEngine::new(memory, llm, ConsolidationConfig::default());
///
/// // Consolidate memories from the last hour
/// let scope = ConsolidationScope::TimeRange {
///     start: Utc::now() - Duration::hours(1),
///     end: Utc::now(),
/// };
///
/// let insights = engine.consolidate(&scope).await?;
/// ```
pub struct ConsolidationEngine {
    memory: Arc<MemorySystem>,
    llm: Arc<dyn LLMProvider>,
    config: ConsolidationConfig,
}

impl ConsolidationEngine {
    /// Create a new consolidation engine.
    pub fn new(
        memory: Arc<MemorySystem>,
        llm: Arc<dyn LLMProvider>,
        config: ConsolidationConfig,
    ) -> Self {
        Self {
            memory,
            llm,
            config,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &ConsolidationConfig {
        &self.config
    }

    /// Run consolidation on memories within a scope.
    pub async fn consolidate(&self, scope: &ConsolidationScope) -> Result<Vec<Insight>> {
        // Fetch memories in scope
        let memories = self.fetch_memories(scope).await?;

        if memories.len() < self.config.min_memories {
            tracing::debug!(
                "Not enough memories for consolidation ({} < {})",
                memories.len(),
                self.config.min_memories
            );
            return Ok(Vec::new());
        }

        tracing::info!(
            "Consolidating {} memories with scope {:?}",
            memories.len(),
            scope
        );

        let mut insights = Vec::new();

        // Generate insights if enabled
        if self.config.generate_insights {
            insights = self.generate_insights(&memories).await?;

            // Store insights as memories
            for insight in &insights {
                if let Err(e) = self.store_insight(insight).await {
                    tracing::warn!("Failed to store insight: {}", e);
                }
            }
        }

        // Update importance scores if enabled
        if self.config.update_importance_scores {
            self.update_importance_scores(&memories).await?;
        }

        Ok(insights)
    }

    /// Fetch memories within a scope.
    async fn fetch_memories(&self, scope: &ConsolidationScope) -> Result<Vec<Memory>> {
        match scope {
            ConsolidationScope::Session(session_id) => {
                // Search for memories with session context
                let query = format!("session:{}", session_id);
                self.memory
                    .search(&query, Some(self.config.batch_size))
                    .await
            }
            ConsolidationScope::TimeRange { start, end } => {
                // For time range, we search for recent memories
                // Note: Locai doesn't have direct time-range queries, so we use general search
                // and filter client-side
                let all_memories = self.memory.search("", Some(self.config.batch_size * 2)).await?;

                let filtered: Vec<Memory> = all_memories
                    .into_iter()
                    .filter(|m| m.created_at >= *start && m.created_at <= *end)
                    .take(self.config.batch_size)
                    .collect();

                Ok(filtered)
            }
            ConsolidationScope::Tagged(tag) => {
                // Search for memories with specific tag
                self.memory
                    .search(tag, Some(self.config.batch_size))
                    .await
            }
        }
    }

    /// Generate insights from memories using LLM.
    async fn generate_insights(&self, memories: &[Memory]) -> Result<Vec<Insight>> {
        if memories.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare memory content for LLM
        let memory_summaries: Vec<String> = memories
            .iter()
            .enumerate()
            .map(|(i, m)| format!("[{}] {}", i + 1, m.content))
            .collect();

        let memories_text = memory_summaries.join("\n");

        let system_prompt = "You are an AI assistant that analyzes memories and generates insights. Be concise and focus on actionable patterns.";
        
        let user_prompt = format!(
            r#"Analyze the following memories and identify key insights, patterns, and themes.

MEMORIES:
{}

Please provide:
1. A brief summary of the main themes (1-2 sentences)
2. Key patterns you observe
3. Any connections between memories

Respond in a structured format."#,
            memories_text
        );

        let request = LLMRequest::with_system_prompt(system_prompt, user_prompt);

        let response = self
            .llm
            .generate_request(&request)
            .await
            .map_err(|e| ThymosError::Agent(format!("LLM error: {}", e)))?;
        
        let response_text = response.content;

        // Parse response into insights
        let source_ids: Vec<String> = memories.iter().map(|m| m.id.clone()).collect();

        let insight = Insight::pattern(response_text, source_ids, 0.8);

        Ok(vec![insight])
    }

    /// Update importance scores for memories based on patterns.
    async fn update_importance_scores(&self, memories: &[Memory]) -> Result<()> {
        // Calculate a simple importance boost based on:
        // 1. Memory age (older memories that persist are more important)
        // 2. Connection to other memories (frequently referenced)

        for memory in memories {
            let strength = self.memory.calculate_strength(memory);

            // Log importance calculations (actual updates would require Locai support
            // for updating memory metadata)
            tracing::debug!(
                "Memory {} strength: {:.2}",
                &memory.id[..8],
                strength
            );
        }

        Ok(())
    }

    /// Store an insight as a fact memory.
    async fn store_insight(&self, insight: &Insight) -> Result<String> {
        let content = format!(
            "[INSIGHT - {}] {}\n\nSources: {} memories",
            insight.insight_type.as_str(),
            insight.summary,
            insight.source_memory_ids.len()
        );

        let options = RememberOptions::new()
            .with_tag("insight")
            .with_tag(format!("insight-type:{}", insight.insight_type.as_str()))
            .with_priority(5); // Higher priority for insights

        self.memory.remember_with_options(content, options).await
    }

    /// Create a consolidated memory from insights.
    ///
    /// This stores insights as a new memory with references to source memories.
    pub async fn create_consolidated_memory(
        &self,
        insight: &Insight,
        tags: Vec<String>,
    ) -> Result<String> {
        let content = format!(
            "INSIGHT [{}]: {}\n\nSources: {}",
            insight.insight_type.as_str(),
            insight.summary,
            insight.source_memory_ids.join(", ")
        );

        let mut options = RememberOptions::new()
            .with_tag("consolidated")
            .with_priority(10);

        for tag in tags {
            options = options.with_tag(tag);
        }

        self.memory.remember_with_options(content, options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consolidation_config() {
        let config = ConsolidationConfig::new()
            .with_min_memories(5)
            .with_generate_insights(false);

        assert_eq!(config.min_memories, 5);
        assert!(!config.generate_insights);
    }

    #[test]
    fn test_consolidation_engine_creation() {
        // Note: MemorySystem requires async initialization
        // This is a compile-time verification that the types work
        let _config = ConsolidationConfig::default();
        // Engine would be created with a real MemorySystem instance
    }

    #[tokio::test]
    async fn test_consolidation_scope() {
        let session = ConsolidationScope::Session("session_123".to_string());
        assert!(matches!(session, ConsolidationScope::Session(_)));

        let time_range = ConsolidationScope::TimeRange {
            start: chrono::Utc::now() - chrono::Duration::hours(1),
            end: chrono::Utc::now(),
        };
        assert!(matches!(time_range, ConsolidationScope::TimeRange { .. }));

        let tagged = ConsolidationScope::Tagged("important".to_string());
        assert!(matches!(tagged, ConsolidationScope::Tagged(_)));
    }
}
