//! Speculative Execution Pattern
//!
//! Try multiple approaches in parallel branches, evaluate, and commit the best.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::error::{Result, ThymosError};
use crate::llm::LLMProvider;
use crate::memory::versioning::MemoryRepository;

use super::evaluator::{ApproachEvaluator, ApproachResult, EvaluationScore};

/// Trace of speculative execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeTrace {
    /// All approach results
    pub approaches: Vec<ApproachResult>,
    /// Index of selected approach
    pub selected_index: Option<usize>,
    /// Total duration in ms
    pub total_duration_ms: u64,
    /// Whether a result was committed
    pub committed: bool,
}

/// Configuration for speculative execution
#[derive(Debug, Clone)]
pub struct SpeculativeConfig {
    /// Minimum acceptable score
    pub min_score: f64,
    /// Whether to run approaches in parallel
    pub parallel: bool,
    /// Whether to commit the best result
    pub auto_commit: bool,
    /// Commit message template
    pub commit_message: Option<String>,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            min_score: 0.5,
            parallel: true,
            auto_commit: true,
            commit_message: None,
        }
    }
}

/// An approach execution function
pub type ApproachFn = Arc<
    dyn Fn(serde_json::Value) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send>,
    > + Send
        + Sync,
>;

/// An approach to try during speculative execution
struct Approach {
    name: String,
    execute: ApproachFn,
}

/// Speculative execution pattern
pub struct SpeculativeExecution {
    /// Name for tracing
    name: String,
    /// Memory repository for branching
    repository: Arc<MemoryRepository>,
    /// Approaches to try
    approaches: Vec<Approach>,
    /// Evaluator for comparing results
    evaluator: Option<Arc<dyn ApproachEvaluator>>,
    /// Configuration
    config: SpeculativeConfig,
}

impl SpeculativeExecution {
    /// Create a new builder
    pub fn builder(
        name: impl Into<String>,
        repository: Arc<MemoryRepository>,
    ) -> SpeculativeExecutionBuilder {
        SpeculativeExecutionBuilder::new(name, repository)
    }

    /// Execute all approaches and return the best result
    pub async fn execute(
        &self,
        input: serde_json::Value,
    ) -> Result<(serde_json::Value, SpeculativeTrace)> {
        let start = Instant::now();
        let base_branch = self.repository.get_current_branch().await;

        let mut results = if self.config.parallel {
            self.execute_parallel(&input, &base_branch).await?
        } else {
            self.execute_sequential(&input, &base_branch).await?
        };

        // Evaluate results if we have an evaluator
        if let Some(ref evaluator) = self.evaluator {
            for result in &mut results {
                if result.success {
                    let score = evaluator.evaluate(result, &input).await?;
                    result.score = Some(score);
                }
            }
        }

        // Find the best result
        let (selected_index, best_result) = self.select_best(&results)?;

        // Commit if configured
        let committed = if self.config.auto_commit && best_result.is_some() {
            if let Some(ref result) = best_result {
                let message = self.config.commit_message.clone().unwrap_or_else(|| {
                    format!(
                        "Speculative execution: selected approach '{}'",
                        result.name
                    )
                });

                // Commit to the winning branch
                self.repository
                    .commit(&message, "speculative_execution", None)
                    .await?;

                true
            } else {
                false
            }
        } else {
            false
        };

        // Clean up non-winning branches
        for result in &results {
            if Some(&result.name) != best_result.as_ref().map(|r| &r.name) {
                let _ = self.repository.delete_branch(&result.branch, true).await;
            }
        }

        let trace = SpeculativeTrace {
            approaches: results,
            selected_index,
            total_duration_ms: start.elapsed().as_millis() as u64,
            committed,
        };

        let output = best_result
            .map(|r| r.output.clone())
            .unwrap_or(serde_json::Value::Null);

        Ok((output, trace))
    }

    async fn execute_parallel(
        &self,
        input: &serde_json::Value,
        _base_branch: &str,
    ) -> Result<Vec<ApproachResult>> {
        let mut handles = Vec::new();

        for approach in &self.approaches {
            let branch_name = format!("speculative-{}-{}", self.name, approach.name);

            // Create branch for this approach
            self.repository
                .create_branch(&branch_name, Some(&format!("Speculative: {}", approach.name)))
                .await?;

            let input = input.clone();
            let execute = approach.execute.clone();
            let name = approach.name.clone();
            let branch = branch_name.clone();

            handles.push(tokio::spawn(async move {
                let start = Instant::now();

                match execute(input).await {
                    Ok(output) => ApproachResult {
                        name,
                        branch,
                        output,
                        score: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: true,
                        error: None,
                    },
                    Err(e) => ApproachResult {
                        name,
                        branch,
                        output: serde_json::Value::Null,
                        score: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        success: false,
                        error: Some(e.to_string()),
                    },
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(ApproachResult {
                        name: "unknown".to_string(),
                        branch: "unknown".to_string(),
                        output: serde_json::Value::Null,
                        score: None,
                        duration_ms: 0,
                        success: false,
                        error: Some(format!("Task panicked: {}", e)),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn execute_sequential(
        &self,
        input: &serde_json::Value,
        _base_branch: &str,
    ) -> Result<Vec<ApproachResult>> {
        let mut results = Vec::new();

        for approach in &self.approaches {
            let branch_name = format!("speculative-{}-{}", self.name, approach.name);

            // Create branch for this approach
            self.repository
                .create_branch(&branch_name, Some(&format!("Speculative: {}", approach.name)))
                .await?;

            let start = Instant::now();

            let result = match (approach.execute)(input.clone()).await {
                Ok(output) => ApproachResult {
                    name: approach.name.clone(),
                    branch: branch_name,
                    output,
                    score: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: true,
                    error: None,
                },
                Err(e) => ApproachResult {
                    name: approach.name.clone(),
                    branch: branch_name,
                    output: serde_json::Value::Null,
                    score: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    success: false,
                    error: Some(e.to_string()),
                },
            };

            results.push(result);
        }

        Ok(results)
    }

    fn select_best(&self, results: &[ApproachResult]) -> Result<(Option<usize>, Option<ApproachResult>)> {
        let successful: Vec<_> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.success)
            .collect();

        if successful.is_empty() {
            return Ok((None, None));
        }

        // If we have scores, use them
        let with_scores: Vec<_> = successful
            .iter()
            .filter(|(_, r)| r.score.is_some())
            .collect();

        if !with_scores.is_empty() {
            let best = with_scores
                .iter()
                .filter(|(_, r)| {
                    r.score
                        .as_ref()
                        .map(|s| s.score >= self.config.min_score)
                        .unwrap_or(false)
                })
                .max_by(|(_, a), (_, b)| {
                    let score_a = a.score.as_ref().map(|s| s.score).unwrap_or(0.0);
                    let score_b = b.score.as_ref().map(|s| s.score).unwrap_or(0.0);
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some((idx, result)) = best {
                return Ok((Some(*idx), Some((*result).clone())));
            }
        }

        // No scores or none meet threshold - return first successful
        Ok((Some(successful[0].0), Some(successful[0].1.clone())))
    }
}

/// Builder for SpeculativeExecution
pub struct SpeculativeExecutionBuilder {
    name: String,
    repository: Arc<MemoryRepository>,
    approaches: Vec<Approach>,
    evaluator: Option<Arc<dyn ApproachEvaluator>>,
    config: SpeculativeConfig,
}

impl SpeculativeExecutionBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>, repository: Arc<MemoryRepository>) -> Self {
        Self {
            name: name.into(),
            repository,
            approaches: Vec::new(),
            evaluator: None,
            config: SpeculativeConfig::default(),
        }
    }

    /// Add an approach with a closure
    pub fn add_approach<F, Fut>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        let name = name.into();
        self.approaches.push(Approach {
            name,
            execute: Arc::new(move |input| Box::pin(f(input))),
        });
        self
    }

    /// Set the evaluator
    pub fn with_evaluator<E: ApproachEvaluator + 'static>(mut self, evaluator: E) -> Self {
        self.evaluator = Some(Arc::new(evaluator));
        self
    }

    /// Set minimum score threshold
    pub fn with_min_score(mut self, min_score: f64) -> Self {
        self.config.min_score = min_score;
        self
    }

    /// Run approaches sequentially instead of parallel
    pub fn sequential(mut self) -> Self {
        self.config.parallel = false;
        self
    }

    /// Disable auto-commit
    pub fn no_auto_commit(mut self) -> Self {
        self.config.auto_commit = false;
        self
    }

    /// Set commit message
    pub fn with_commit_message(mut self, message: impl Into<String>) -> Self {
        self.config.commit_message = Some(message.into());
        self
    }

    /// Build the SpeculativeExecution
    pub fn build(self) -> SpeculativeExecution {
        SpeculativeExecution {
            name: self.name,
            repository: self.repository,
            approaches: self.approaches,
            evaluator: self.evaluator,
            config: self.config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_config_default() {
        let config = SpeculativeConfig::default();
        assert_eq!(config.min_score, 0.5);
        assert!(config.parallel);
        assert!(config.auto_commit);
    }

    #[test]
    fn test_approach_result() {
        let result = ApproachResult {
            name: "test".to_string(),
            branch: "test-branch".to_string(),
            output: serde_json::json!({"value": 42}),
            score: Some(EvaluationScore::new(0.8)),
            duration_ms: 100,
            success: true,
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.score.as_ref().unwrap().score, 0.8);
    }
}
