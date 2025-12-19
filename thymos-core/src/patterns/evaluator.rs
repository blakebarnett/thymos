//! Approach Evaluation for Speculative Execution

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::Result;
use crate::llm::LLMProvider;

/// Score for an approach result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationScore {
    /// Numeric score (0.0 to 1.0)
    pub score: f64,
    /// Reasoning for the score
    pub reasoning: Option<String>,
    /// Whether this result is acceptable
    pub acceptable: bool,
}

impl EvaluationScore {
    /// Create a new score
    pub fn new(score: f64) -> Self {
        Self {
            score: score.clamp(0.0, 1.0),
            reasoning: None,
            acceptable: score >= 0.5,
        }
    }

    /// Create with reasoning
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    /// Set acceptability threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.acceptable = self.score >= threshold;
        self
    }
}

/// Result from an approach execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproachResult {
    /// Approach name
    pub name: String,
    /// Branch name used
    pub branch: String,
    /// Output from the approach
    pub output: serde_json::Value,
    /// Evaluation score (if evaluated)
    pub score: Option<EvaluationScore>,
    /// Execution duration in ms
    pub duration_ms: u64,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Trait for evaluating approach results
#[async_trait]
pub trait ApproachEvaluator: Send + Sync {
    /// Evaluate an approach result
    async fn evaluate(
        &self,
        result: &ApproachResult,
        context: &serde_json::Value,
    ) -> Result<EvaluationScore>;

    /// Compare two results and pick the better one
    async fn compare(
        &self,
        a: &ApproachResult,
        b: &ApproachResult,
        context: &serde_json::Value,
    ) -> Result<std::cmp::Ordering> {
        let score_a = self.evaluate(a, context).await?;
        let score_b = self.evaluate(b, context).await?;

        Ok(score_a
            .score
            .partial_cmp(&score_b.score)
            .unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get evaluator name
    fn name(&self) -> &str;
}

/// Function-based evaluator
pub struct FunctionEvaluator<F>
where
    F: Fn(&ApproachResult, &serde_json::Value) -> EvaluationScore + Send + Sync,
{
    name: String,
    func: F,
}

impl<F> FunctionEvaluator<F>
where
    F: Fn(&ApproachResult, &serde_json::Value) -> EvaluationScore + Send + Sync,
{
    /// Create a new function evaluator
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }
}

#[async_trait]
impl<F> ApproachEvaluator for FunctionEvaluator<F>
where
    F: Fn(&ApproachResult, &serde_json::Value) -> EvaluationScore + Send + Sync,
{
    async fn evaluate(
        &self,
        result: &ApproachResult,
        context: &serde_json::Value,
    ) -> Result<EvaluationScore> {
        Ok((self.func)(result, context))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// LLM-based approach evaluator
pub struct LLMApproachEvaluator {
    provider: Arc<dyn LLMProvider>,
    criteria: String,
    system_prompt: Option<String>,
}

impl LLMApproachEvaluator {
    /// Create a new LLM evaluator
    pub fn new(provider: Arc<dyn LLMProvider>, criteria: impl Into<String>) -> Self {
        Self {
            provider,
            criteria: criteria.into(),
            system_prompt: None,
        }
    }

    /// Set custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
}

#[async_trait]
impl ApproachEvaluator for LLMApproachEvaluator {
    async fn evaluate(
        &self,
        result: &ApproachResult,
        context: &serde_json::Value,
    ) -> Result<EvaluationScore> {
        let system = self.system_prompt.clone().unwrap_or_else(|| {
            "You are an evaluator. Given an approach result and evaluation criteria, \
             provide a score from 0.0 to 1.0 and reasoning. Respond in JSON format: \
             {\"score\": 0.0-1.0, \"reasoning\": \"...\", \"acceptable\": true/false}"
                .to_string()
        });

        let prompt = format!(
            "Evaluate this approach result:\n\n\
             Approach: {}\n\
             Output: {}\n\
             Success: {}\n\
             Duration: {}ms\n\n\
             Context: {}\n\n\
             Evaluation Criteria: {}\n\n\
             Provide your evaluation as JSON.",
            result.name,
            serde_json::to_string_pretty(&result.output).unwrap_or_default(),
            result.success,
            result.duration_ms,
            serde_json::to_string_pretty(context).unwrap_or_default(),
            self.criteria
        );

        let request = crate::llm::LLMRequest {
            messages: vec![
                crate::llm::Message {
                    role: crate::llm::MessageRole::System,
                    content: system,
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

        let response = self.provider.generate_request(&request).await?;

        // Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or_else(|_| serde_json::json!({"score": 0.5, "reasoning": response.content}));

        let score = parsed["score"].as_f64().unwrap_or(0.5);
        let reasoning = parsed["reasoning"].as_str().map(|s| s.to_string());
        let acceptable = parsed["acceptable"].as_bool().unwrap_or(score >= 0.5);

        Ok(EvaluationScore {
            score,
            reasoning,
            acceptable,
        })
    }

    fn name(&self) -> &str {
        "llm_evaluator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluation_score() {
        let score = EvaluationScore::new(0.8).with_reasoning("Good result");

        assert_eq!(score.score, 0.8);
        assert!(score.acceptable);
        assert_eq!(score.reasoning.as_deref(), Some("Good result"));
    }

    #[test]
    fn test_score_clamping() {
        let score = EvaluationScore::new(1.5);
        assert_eq!(score.score, 1.0);

        let score = EvaluationScore::new(-0.5);
        assert_eq!(score.score, 0.0);
    }

    #[test]
    fn test_score_threshold() {
        let score = EvaluationScore::new(0.6).with_threshold(0.7);
        assert!(!score.acceptable);

        let score = EvaluationScore::new(0.8).with_threshold(0.7);
        assert!(score.acceptable);
    }

    #[tokio::test]
    async fn test_function_evaluator() {
        let evaluator = FunctionEvaluator::new("test", |result, _ctx| {
            if result.success {
                EvaluationScore::new(0.9)
            } else {
                EvaluationScore::new(0.1)
            }
        });

        let result = ApproachResult {
            name: "test".to_string(),
            branch: "test-branch".to_string(),
            output: serde_json::json!({"value": 42}),
            score: None,
            duration_ms: 100,
            success: true,
            error: None,
        };

        let score = evaluator
            .evaluate(&result, &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(score.score, 0.9);
    }
}
