//! Consensus Merge Pattern
//!
//! Multi-agent consensus via voting or LLM-assisted merge.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Result, ThymosError};
use crate::llm::LLMProvider;
use crate::memory::versioning::{MemoryRepository, MergeStrategy};

/// Strategy for achieving consensus
#[derive(Clone)]
pub enum ConsensusStrategy {
    /// Majority vote - pick result with most agreement
    Majority {
        /// Minimum votes required
        min_votes: usize,
    },
    /// Unanimous - all must agree
    Unanimous,
    /// Quorum - require N agreeing agents
    Quorum {
        /// Required agreement count
        required: usize,
    },
    /// LLM picks the best result
    LLMSelect {
        /// LLM provider
        provider: Arc<dyn LLMProvider>,
        /// Selection criteria
        criteria: String,
    },
    /// LLM synthesizes from all results
    LLMSynthesize {
        /// LLM provider
        provider: Arc<dyn LLMProvider>,
        /// Synthesis instructions
        instructions: String,
    },
}

/// Result of consensus process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// Whether consensus was reached
    pub reached: bool,
    /// Final result (if consensus reached)
    pub result: Option<serde_json::Value>,
    /// Individual agent results
    pub agent_results: Vec<AgentResult>,
    /// Votes per result (for voting strategies)
    pub votes: HashMap<String, usize>,
    /// Reasoning for the decision
    pub reasoning: Option<String>,
    /// Merge commit if changes were committed
    pub merge_commit: Option<String>,
}

/// Result from a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Agent ID
    pub agent_id: String,
    /// Branch name
    pub branch: String,
    /// Result value
    pub result: serde_json::Value,
    /// Result hash (for comparison)
    pub result_hash: String,
}

impl AgentResult {
    /// Create from a result
    pub fn new(
        agent_id: impl Into<String>,
        branch: impl Into<String>,
        result: serde_json::Value,
    ) -> Self {
        // Simple hash using the default hasher
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        result.to_string().hash(&mut hasher);
        let result_hash = format!("{:016x}", hasher.finish());

        Self {
            agent_id: agent_id.into(),
            branch: branch.into(),
            result,
            result_hash,
        }
    }
}

/// Configuration for consensus merge
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Whether to auto-merge on consensus
    pub auto_merge: bool,
    /// Target branch for merge
    pub target_branch: Option<String>,
    /// Whether to require user judgment on no consensus
    pub require_user_judgment: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            auto_merge: true,
            target_branch: None,
            require_user_judgment: true,
        }
    }
}

/// Consensus merge pattern
pub struct ConsensusMerge {
    /// Name for tracing
    name: String,
    /// Memory repository
    repository: Arc<MemoryRepository>,
    /// Consensus strategy
    strategy: ConsensusStrategy,
    /// Configuration
    config: ConsensusConfig,
}

impl ConsensusMerge {
    /// Create a new ConsensusMerge
    pub fn new(
        name: impl Into<String>,
        repository: Arc<MemoryRepository>,
        strategy: ConsensusStrategy,
    ) -> Self {
        Self {
            name: name.into(),
            repository,
            strategy,
            config: ConsensusConfig::default(),
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: ConsensusConfig) -> Self {
        self.config = config;
        self
    }

    /// Attempt to reach consensus from agent results
    pub async fn reach_consensus(
        &self,
        agent_results: Vec<AgentResult>,
    ) -> Result<ConsensusResult> {
        if agent_results.is_empty() {
            return Ok(ConsensusResult {
                reached: false,
                result: None,
                agent_results: Vec::new(),
                votes: HashMap::new(),
                reasoning: Some("No agent results provided".to_string()),
                merge_commit: None,
            });
        }

        match &self.strategy {
            ConsensusStrategy::Majority { min_votes } => {
                self.majority_consensus(&agent_results, *min_votes).await
            }
            ConsensusStrategy::Unanimous => self.unanimous_consensus(&agent_results).await,
            ConsensusStrategy::Quorum { required } => {
                self.quorum_consensus(&agent_results, *required).await
            }
            ConsensusStrategy::LLMSelect { provider, criteria } => {
                self.llm_select(&agent_results, provider, criteria).await
            }
            ConsensusStrategy::LLMSynthesize {
                provider,
                instructions,
            } => {
                self.llm_synthesize(&agent_results, provider, instructions)
                    .await
            }
        }
    }

    async fn majority_consensus(
        &self,
        results: &[AgentResult],
        min_votes: usize,
    ) -> Result<ConsensusResult> {
        let votes = self.count_votes(results);

        // Find the result with most votes
        let best = votes.iter().max_by_key(|(_, count)| *count);

        if let Some((hash, count)) = best {
            if *count >= min_votes {
                let winning_result = results.iter().find(|r| &r.result_hash == hash);
                let count_val = *count;

                return Ok(ConsensusResult {
                    reached: true,
                    result: winning_result.map(|r| r.result.clone()),
                    agent_results: results.to_vec(),
                    votes: votes.clone(),
                    reasoning: Some(format!("Majority consensus with {} votes", count_val)),
                    merge_commit: None,
                });
            }
        }

        Ok(ConsensusResult {
            reached: false,
            result: None,
            agent_results: results.to_vec(),
            votes,
            reasoning: Some(format!(
                "No result reached minimum {} votes",
                min_votes
            )),
            merge_commit: None,
        })
    }

    async fn unanimous_consensus(&self, results: &[AgentResult]) -> Result<ConsensusResult> {
        let votes = self.count_votes(results);

        // Check if all results are the same
        if votes.len() == 1 {
            let (_, count) = votes.iter().next().unwrap();
            if *count == results.len() {
                return Ok(ConsensusResult {
                    reached: true,
                    result: Some(results[0].result.clone()),
                    agent_results: results.to_vec(),
                    votes,
                    reasoning: Some("Unanimous consensus".to_string()),
                    merge_commit: None,
                });
            }
        }

        Ok(ConsensusResult {
            reached: false,
            result: None,
            agent_results: results.to_vec(),
            votes,
            reasoning: Some("Results differ - no unanimous consensus".to_string()),
            merge_commit: None,
        })
    }

    async fn quorum_consensus(
        &self,
        results: &[AgentResult],
        required: usize,
    ) -> Result<ConsensusResult> {
        let votes = self.count_votes(results);

        // Find any result with enough votes
        for (hash, count) in &votes {
            if *count >= required {
                let winning_result = results.iter().find(|r| &r.result_hash == hash);
                let count_val = *count;

                return Ok(ConsensusResult {
                    reached: true,
                    result: winning_result.map(|r| r.result.clone()),
                    agent_results: results.to_vec(),
                    votes: votes.clone(),
                    reasoning: Some(format!("Quorum reached with {} agreeing agents", count_val)),
                    merge_commit: None,
                });
            }
        }

        Ok(ConsensusResult {
            reached: false,
            result: None,
            agent_results: results.to_vec(),
            votes,
            reasoning: Some(format!(
                "Quorum of {} not reached",
                required
            )),
            merge_commit: None,
        })
    }

    async fn llm_select(
        &self,
        results: &[AgentResult],
        provider: &Arc<dyn LLMProvider>,
        criteria: &str,
    ) -> Result<ConsensusResult> {
        let results_json: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                serde_json::json!({
                    "index": i,
                    "agent_id": r.agent_id,
                    "result": r.result,
                })
            })
            .collect();

        let prompt = format!(
            "Given these agent results, select the best one based on the criteria.\n\n\
             Results:\n{}\n\n\
             Selection Criteria: {}\n\n\
             Respond with JSON: {{\"selected_index\": N, \"reasoning\": \"...\"}}",
            serde_json::to_string_pretty(&results_json).unwrap_or_default(),
            criteria
        );

        let request = crate::llm::LLMRequest {
            messages: vec![
                crate::llm::Message {
                    role: crate::llm::MessageRole::System,
                    content: "You are a judge selecting the best result from multiple agents.".to_string(),
                },
                crate::llm::Message {
                    role: crate::llm::MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(512),
            stop_sequences: Vec::new(),
        };

        let response = provider.generate_request(&request).await?;

        let parsed: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or_else(|_| serde_json::json!({"selected_index": 0}));

        let selected_index = parsed["selected_index"].as_u64().unwrap_or(0) as usize;
        let reasoning = parsed["reasoning"].as_str().map(|s| s.to_string());

        let selected = results.get(selected_index);

        Ok(ConsensusResult {
            reached: selected.is_some(),
            result: selected.map(|r| r.result.clone()),
            agent_results: results.to_vec(),
            votes: self.count_votes(results),
            reasoning,
            merge_commit: None,
        })
    }

    async fn llm_synthesize(
        &self,
        results: &[AgentResult],
        provider: &Arc<dyn LLMProvider>,
        instructions: &str,
    ) -> Result<ConsensusResult> {
        let results_json: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "agent_id": r.agent_id,
                    "result": r.result,
                })
            })
            .collect();

        let prompt = format!(
            "Synthesize a final result from these agent results.\n\n\
             Results:\n{}\n\n\
             Synthesis Instructions: {}\n\n\
             Respond with JSON: {{\"synthesized_result\": ..., \"reasoning\": \"...\"}}",
            serde_json::to_string_pretty(&results_json).unwrap_or_default(),
            instructions
        );

        let request = crate::llm::LLMRequest {
            messages: vec![
                crate::llm::Message {
                    role: crate::llm::MessageRole::System,
                    content: "You are synthesizing the best aspects of multiple agent results.".to_string(),
                },
                crate::llm::Message {
                    role: crate::llm::MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(1024),
            stop_sequences: Vec::new(),
        };

        let response = provider.generate_request(&request).await?;

        let parsed: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or_else(|_| serde_json::json!({"synthesized_result": response.content}));

        let synthesized = parsed.get("synthesized_result").cloned();
        let reasoning = parsed["reasoning"].as_str().map(|s| s.to_string());

        Ok(ConsensusResult {
            reached: synthesized.is_some(),
            result: synthesized,
            agent_results: results.to_vec(),
            votes: self.count_votes(results),
            reasoning,
            merge_commit: None,
        })
    }

    fn count_votes(&self, results: &[AgentResult]) -> HashMap<String, usize> {
        let mut votes = HashMap::new();
        for result in results {
            *votes.entry(result.result_hash.clone()).or_insert(0) += 1;
        }
        votes
    }

    /// Merge the consensus result back to a branch
    pub async fn merge_consensus(
        &self,
        consensus: &ConsensusResult,
        winning_branch: &str,
        target_branch: &str,
    ) -> Result<Option<String>> {
        if !consensus.reached {
            return Ok(None);
        }

        let merge_result = self
            .repository
            .merge(winning_branch, target_branch, MergeStrategy::Theirs)
            .await?;

        match merge_result {
            crate::memory::versioning::MergeResult::Success { commit } => Ok(commit),
            crate::memory::versioning::MergeResult::Conflicts { conflicts } => {
                Err(ThymosError::Memory(format!(
                    "Merge conflicts: {} conflicts detected",
                    conflicts.len()
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_result_creation() {
        let result = AgentResult::new("agent-1", "branch-1", serde_json::json!({"value": 42}));

        assert_eq!(result.agent_id, "agent-1");
        assert!(!result.result_hash.is_empty());
    }

    #[test]
    fn test_agent_result_hash_consistency() {
        let result1 = AgentResult::new("agent-1", "branch-1", serde_json::json!({"value": 42}));
        let result2 = AgentResult::new("agent-2", "branch-2", serde_json::json!({"value": 42}));

        // Same result value should produce same hash
        assert_eq!(result1.result_hash, result2.result_hash);
    }

    #[test]
    fn test_consensus_config_default() {
        let config = ConsensusConfig::default();
        assert!(config.auto_merge);
        assert!(config.require_user_judgment);
    }
}
