//! Evaluator-Optimizer Workflow Pattern
//!
//! Iterative refinement with generate-evaluate-refine loops.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};

use super::execution::{WorkflowError, WorkflowResult};
use super::step::StepOutput;

/// Evaluation result with score and feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    /// Quality score (0.0 to 1.0)
    pub score: f32,
    /// Feedback for improvement
    pub feedback: String,
    /// Whether the result is acceptable
    pub acceptable: bool,
    /// Specific issues found
    pub issues: Vec<String>,
}

impl Evaluation {
    /// Create a new evaluation
    pub fn new(score: f32, feedback: impl Into<String>) -> Self {
        let score = score.clamp(0.0, 1.0);
        Self {
            score,
            feedback: feedback.into(),
            acceptable: score >= 0.7,
            issues: Vec::new(),
        }
    }

    /// Create with custom acceptance threshold
    pub fn with_threshold(score: f32, feedback: impl Into<String>, threshold: f32) -> Self {
        let score = score.clamp(0.0, 1.0);
        Self {
            score,
            feedback: feedback.into(),
            acceptable: score >= threshold,
            issues: Vec::new(),
        }
    }

    /// Add issues
    pub fn with_issues(mut self, issues: Vec<String>) -> Self {
        self.issues = issues;
        self
    }
}

/// Trait for content generation
#[async_trait]
pub trait Generator: Send + Sync {
    /// Generate content from input
    async fn generate(
        &self,
        input: &serde_json::Value,
        feedback: Option<&str>,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<StepOutput>;
}

/// Trait for content evaluation
#[async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate generated content
    async fn evaluate(
        &self,
        input: &serde_json::Value,
        output: &StepOutput,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<Evaluation>;
}

/// LLM-based generator
pub struct LLMGenerator {
    system_prompt: String,
    user_prompt_template: String,
}

impl LLMGenerator {
    /// Create a new LLM generator
    pub fn new(
        system_prompt: impl Into<String>,
        user_prompt_template: impl Into<String>,
    ) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            user_prompt_template: user_prompt_template.into(),
        }
    }
}

#[async_trait]
impl Generator for LLMGenerator {
    async fn generate(
        &self,
        input: &serde_json::Value,
        feedback: Option<&str>,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<StepOutput> {
        let input_str = match input {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };

        let mut prompt = self.user_prompt_template.replace("{{input}}", &input_str);

        if let Some(fb) = feedback {
            prompt.push_str(&format!(
                "\n\nPrevious attempt feedback:\n{}\n\nPlease improve based on this feedback.",
                fb
            ));
        }

        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(2000),
            stop_sequences: Vec::new(),
        };

        let response = provider
            .generate_request(&request)
            .await
            .map_err(|e| WorkflowError::LLMError(e.to_string()))?;

        Ok(StepOutput::with_text(response.content))
    }
}

/// LLM-based evaluator
pub struct LLMEvaluator {
    system_prompt: String,
    evaluation_criteria: Vec<String>,
}

impl LLMEvaluator {
    /// Create a new LLM evaluator
    pub fn new() -> Self {
        Self {
            system_prompt: Self::default_system_prompt(),
            evaluation_criteria: Vec::new(),
        }
    }

    /// Create with custom criteria
    pub fn with_criteria(criteria: Vec<String>) -> Self {
        Self {
            system_prompt: Self::default_system_prompt(),
            evaluation_criteria: criteria,
        }
    }

    fn default_system_prompt() -> String {
        r#"You are an expert evaluator. Evaluate the given content and provide:
1. A score from 0.0 to 1.0
2. Specific feedback for improvement
3. A list of issues found

Respond in JSON format:
{
  "score": 0.8,
  "feedback": "Overall good but...",
  "issues": ["issue1", "issue2"]
}"#.to_string()
    }
}

impl Default for LLMEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for LLMEvaluator {
    async fn evaluate(
        &self,
        input: &serde_json::Value,
        output: &StepOutput,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<Evaluation> {
        let output_str = match &output.data {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };

        let mut user_prompt = format!(
            "Original request:\n{}\n\nGenerated content:\n{}",
            serde_json::to_string_pretty(input).unwrap_or_default(),
            output_str
        );

        if !self.evaluation_criteria.is_empty() {
            user_prompt.push_str("\n\nEvaluation criteria:\n");
            for (i, criterion) in self.evaluation_criteria.iter().enumerate() {
                user_prompt.push_str(&format!("{}. {}\n", i + 1, criterion));
            }
        }

        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: MessageRole::User,
                    content: user_prompt,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(500),
            stop_sequences: Vec::new(),
        };

        let response = provider
            .generate_request(&request)
            .await
            .map_err(|e| WorkflowError::LLMError(e.to_string()))?;

        // Parse JSON response
        #[derive(Deserialize)]
        struct EvalResponse {
            score: f32,
            feedback: String,
            issues: Option<Vec<String>>,
        }

        let eval: EvalResponse = serde_json::from_str(&response.content)
            .map_err(|e| WorkflowError::ParseError(format!("Failed to parse evaluation: {}", e)))?;

        Ok(Evaluation::new(eval.score, eval.feedback)
            .with_issues(eval.issues.unwrap_or_default()))
    }
}

/// Simple threshold-based evaluator
pub struct ThresholdEvaluator<F>
where
    F: Fn(&StepOutput) -> (f32, String) + Send + Sync,
{
    scorer: F,
    threshold: f32,
}

impl<F> ThresholdEvaluator<F>
where
    F: Fn(&StepOutput) -> (f32, String) + Send + Sync,
{
    /// Create a new threshold evaluator
    pub fn new(scorer: F, threshold: f32) -> Self {
        Self { scorer, threshold }
    }
}

#[async_trait]
impl<F> Evaluator for ThresholdEvaluator<F>
where
    F: Fn(&StepOutput) -> (f32, String) + Send + Sync,
{
    async fn evaluate(
        &self,
        _input: &serde_json::Value,
        output: &StepOutput,
        _provider: &dyn LLMProvider,
    ) -> WorkflowResult<Evaluation> {
        let (score, feedback) = (self.scorer)(output);
        Ok(Evaluation::with_threshold(score, feedback, self.threshold))
    }
}

/// Configuration for evaluator-optimizer
#[derive(Debug, Clone)]
pub struct EvaluatorOptimizerConfig {
    /// Maximum iterations
    pub max_iterations: usize,
    /// Quality threshold to accept
    pub quality_threshold: f32,
    /// Keep all attempts for comparison
    pub keep_attempts: bool,
}

impl Default for EvaluatorOptimizerConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            quality_threshold: 0.8,
            keep_attempts: false,
        }
    }
}

/// Attempt record
#[derive(Debug, Clone)]
pub struct Attempt {
    /// Attempt number (1-indexed)
    pub iteration: usize,
    /// Generated output
    pub output: StepOutput,
    /// Evaluation result
    pub evaluation: Evaluation,
}

/// Execution trace
#[derive(Debug, Clone)]
pub struct EvaluatorOptimizerTrace {
    /// Workflow name
    pub name: String,
    /// All attempts
    pub attempts: Vec<Attempt>,
    /// Final iteration count
    pub iterations: usize,
    /// Whether quality threshold was met
    pub threshold_met: bool,
    /// Total duration
    pub total_duration_ms: u64,
}

impl EvaluatorOptimizerTrace {
    /// Get the best attempt
    pub fn best_attempt(&self) -> Option<&Attempt> {
        self.attempts.iter().max_by(|a, b| {
            a.evaluation
                .score
                .partial_cmp(&b.evaluation.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Evaluator-Optimizer workflow
pub struct EvaluatorOptimizer<G: Generator, E: Evaluator> {
    /// Workflow name
    name: String,
    /// Generator
    generator: G,
    /// Evaluator
    evaluator: E,
    /// Configuration
    config: EvaluatorOptimizerConfig,
}

impl<G: Generator, E: Evaluator> std::fmt::Debug for EvaluatorOptimizer<G, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluatorOptimizer")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish()
    }
}

impl<G: Generator, E: Evaluator> EvaluatorOptimizer<G, E> {
    /// Create a new evaluator-optimizer builder
    pub fn builder(generator: G, evaluator: E) -> EvaluatorOptimizerBuilder<G, E> {
        EvaluatorOptimizerBuilder::new(generator, evaluator)
    }

    /// Get the workflow name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute the generate-evaluate-refine loop
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, EvaluatorOptimizerTrace)> {
        let start = std::time::Instant::now();

        let mut attempts = Vec::new();
        let mut feedback: Option<String> = None;
        let mut best_output: Option<StepOutput> = None;
        let mut threshold_met = false;
        let mut total_iterations = 0;

        for iteration in 1..=self.config.max_iterations {
            total_iterations = iteration;
            // Generate
            let output = self
                .generator
                .generate(&input, feedback.as_deref(), provider)
                .await?;

            // Evaluate
            let evaluation = self.evaluator.evaluate(&input, &output, provider).await?;

            // Track attempt
            let attempt = Attempt {
                iteration,
                output: output.clone(),
                evaluation: evaluation.clone(),
            };

            if self.config.keep_attempts || attempts.is_empty() {
                attempts.push(attempt.clone());
            } else if let Some(last) = attempts.last_mut() {
                // Only keep best
                if evaluation.score > last.evaluation.score {
                    *last = attempt;
                }
            }

            // Update best
            if best_output.is_none() || evaluation.score > attempts.iter()
                .filter(|a| a.iteration < iteration)
                .map(|a| a.evaluation.score)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0)
            {
                best_output = Some(output.clone());
            }

            // Check if acceptable - only use configured threshold
            if evaluation.score >= self.config.quality_threshold {
                threshold_met = true;
                best_output = Some(output);
                break;
            }

            // Set feedback for next iteration
            feedback = Some(format!(
                "Score: {:.2}\nFeedback: {}\nIssues: {}",
                evaluation.score,
                evaluation.feedback,
                evaluation.issues.join(", ")
            ));
        }

        let final_output = best_output.unwrap_or_else(|| {
            attempts
                .iter()
                .max_by(|a, b| {
                    a.evaluation
                        .score
                        .partial_cmp(&b.evaluation.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|a| a.output.clone())
                .unwrap_or_else(|| StepOutput::new(serde_json::Value::Null))
        });

        let trace = EvaluatorOptimizerTrace {
            name: self.name.clone(),
            iterations: total_iterations,
            attempts,
            threshold_met,
            total_duration_ms: start.elapsed().as_millis() as u64,
        };

        Ok((final_output, trace))
    }
}

/// Builder for EvaluatorOptimizer
pub struct EvaluatorOptimizerBuilder<G: Generator, E: Evaluator> {
    name: String,
    generator: G,
    evaluator: E,
    config: EvaluatorOptimizerConfig,
}

impl<G: Generator, E: Evaluator> EvaluatorOptimizerBuilder<G, E> {
    /// Create a new builder
    pub fn new(generator: G, evaluator: E) -> Self {
        Self {
            name: "evaluator_optimizer".to_string(),
            generator,
            evaluator,
            config: EvaluatorOptimizerConfig::default(),
        }
    }

    /// Set the workflow name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set maximum iterations
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }

    /// Set quality threshold
    pub fn quality_threshold(mut self, threshold: f32) -> Self {
        self.config.quality_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Keep all attempts
    pub fn keep_attempts(mut self, keep: bool) -> Self {
        self.config.keep_attempts = keep;
        self
    }

    /// Build the evaluator-optimizer
    pub fn build(self) -> EvaluatorOptimizer<G, E> {
        EvaluatorOptimizer {
            name: self.name,
            generator: self.generator,
            evaluator: self.evaluator,
            config: self.config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LLMConfig, LLMRequest, LLMResponse, ModelInfo};

    struct MockProvider;

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> crate::error::Result<String> {
            Ok("mock".to_string())
        }

        async fn generate_request(&self, _request: &LLMRequest) -> crate::error::Result<LLMResponse> {
            Ok(LLMResponse {
                content: "mock response".to_string(),
                usage: None,
            })
        }

        fn model_info(&self) -> ModelInfo {
            ModelInfo {
                provider: "mock".to_string(),
                model_name: "test".to_string(),
            }
        }
    }

    struct CountingGenerator {
        count: std::sync::atomic::AtomicUsize,
    }

    impl CountingGenerator {
        fn new() -> Self {
            Self {
                count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Generator for CountingGenerator {
        async fn generate(
            &self,
            _input: &serde_json::Value,
            _feedback: Option<&str>,
            _provider: &dyn LLMProvider,
        ) -> WorkflowResult<StepOutput> {
            let count = self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(StepOutput::new(serde_json::json!({"attempt": count + 1})))
        }
    }

    struct ImprovingEvaluator {
        scores: Vec<f32>,
        index: std::sync::atomic::AtomicUsize,
    }

    impl ImprovingEvaluator {
        fn new(scores: Vec<f32>) -> Self {
            Self {
                scores,
                index: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Evaluator for ImprovingEvaluator {
        async fn evaluate(
            &self,
            _input: &serde_json::Value,
            _output: &StepOutput,
            _provider: &dyn LLMProvider,
        ) -> WorkflowResult<Evaluation> {
            let idx = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let score = self.scores.get(idx).copied().unwrap_or(1.0);
            Ok(Evaluation::new(score, format!("Attempt {} feedback", idx + 1)))
        }
    }

    #[test]
    fn test_evaluation_creation() {
        let eval = Evaluation::new(0.8, "Good work");
        assert_eq!(eval.score, 0.8);
        assert!(eval.acceptable);

        let eval = Evaluation::new(0.5, "Needs work");
        assert!(!eval.acceptable);
    }

    #[test]
    fn test_evaluation_clamping() {
        let eval = Evaluation::new(1.5, "Over 100%");
        assert_eq!(eval.score, 1.0);

        let eval = Evaluation::new(-0.5, "Negative");
        assert_eq!(eval.score, 0.0);
    }

    #[tokio::test]
    async fn test_evaluator_optimizer_early_success() {
        let generator = CountingGenerator::new();
        let evaluator = ImprovingEvaluator::new(vec![0.9]); // First attempt succeeds

        let eo = EvaluatorOptimizer::builder(generator, evaluator)
            .name("test")
            .max_iterations(5)
            .quality_threshold(0.8)
            .build();

        let provider = MockProvider;
        let (_, trace) = eo
            .execute(serde_json::json!({}), &provider)
            .await
            .unwrap();

        assert_eq!(trace.iterations, 1);
        assert!(trace.threshold_met);
    }

    #[tokio::test]
    async fn test_evaluator_optimizer_iteration() {
        let generator = CountingGenerator::new();
        let evaluator = ImprovingEvaluator::new(vec![0.3, 0.5, 0.7, 0.9]); // Improves over time

        let eo = EvaluatorOptimizer::builder(generator, evaluator)
            .name("iterating")
            .max_iterations(5)
            .quality_threshold(0.8)
            .keep_attempts(true)
            .build();

        let provider = MockProvider;
        let (_, trace) = eo
            .execute(serde_json::json!({}), &provider)
            .await
            .unwrap();

        assert_eq!(trace.iterations, 4); // Should take 4 attempts to reach 0.9
        assert!(trace.threshold_met);
        assert_eq!(trace.attempts.len(), 4);
    }

    #[tokio::test]
    async fn test_evaluator_optimizer_max_iterations() {
        let generator = CountingGenerator::new();
        let evaluator = ImprovingEvaluator::new(vec![0.3, 0.4, 0.5]); // Never reaches threshold

        let eo = EvaluatorOptimizer::builder(generator, evaluator)
            .max_iterations(3)
            .quality_threshold(0.9)
            .build();

        let provider = MockProvider;
        let (_, trace) = eo
            .execute(serde_json::json!({}), &provider)
            .await
            .unwrap();

        assert_eq!(trace.iterations, 3);
        assert!(!trace.threshold_met);
    }

    #[test]
    fn test_best_attempt() {
        let trace = EvaluatorOptimizerTrace {
            name: "test".to_string(),
            attempts: vec![
                Attempt {
                    iteration: 1,
                    output: StepOutput::new(serde_json::json!(1)),
                    evaluation: Evaluation::new(0.3, ""),
                },
                Attempt {
                    iteration: 2,
                    output: StepOutput::new(serde_json::json!(2)),
                    evaluation: Evaluation::new(0.8, ""),
                },
                Attempt {
                    iteration: 3,
                    output: StepOutput::new(serde_json::json!(3)),
                    evaluation: Evaluation::new(0.5, ""),
                },
            ],
            iterations: 3,
            threshold_met: false,
            total_duration_ms: 100,
        };

        let best = trace.best_attempt().unwrap();
        assert_eq!(best.iteration, 2);
        assert_eq!(best.evaluation.score, 0.8);
    }
}
