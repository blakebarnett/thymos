//! Trace Aggregation

use serde::{Deserialize, Serialize};

use crate::llm::ModelInfo;
use crate::metrics::LLMCostCalculator;
use crate::workflow::{ExecutionTrace, StepTrace, TokenUsageTrace};

/// Aggregated metrics from traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    /// Total steps executed
    pub total_steps: usize,
    /// Successful steps
    pub successful_steps: usize,
    /// Failed steps
    pub failed_steps: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Total prompt tokens
    pub prompt_tokens: usize,
    /// Total completion tokens
    pub completion_tokens: usize,
    /// Total tokens
    pub total_tokens: usize,
    /// Estimated cost in USD
    pub estimated_cost_usd: f64,
}

impl TraceSummary {
    /// Create an empty summary
    pub fn empty() -> Self {
        Self {
            total_steps: 0,
            successful_steps: 0,
            failed_steps: 0,
            total_duration_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
        }
    }

    /// Merge another summary into this one
    pub fn merge(&mut self, other: &TraceSummary) {
        self.total_steps += other.total_steps;
        self.successful_steps += other.successful_steps;
        self.failed_steps += other.failed_steps;
        self.total_duration_ms += other.total_duration_ms;
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.estimated_cost_usd += other.estimated_cost_usd;
    }
}

/// An aggregated trace from multiple sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedTrace {
    /// Trace name
    pub name: String,
    /// All step traces
    pub steps: Vec<StepTrace>,
    /// Nested execution traces
    pub nested: Vec<ExecutionTrace>,
    /// Aggregated summary
    pub summary: TraceSummary,
}

/// Aggregates traces and calculates metrics
pub struct TraceAggregator {
    cost_calculator: LLMCostCalculator,
    model_info: Option<ModelInfo>,
}

impl TraceAggregator {
    /// Create a new trace aggregator
    pub fn new() -> Self {
        Self {
            cost_calculator: LLMCostCalculator::new(),
            model_info: None,
        }
    }

    /// Create with a specific model for cost calculation
    pub fn with_model(model_info: ModelInfo) -> Self {
        Self {
            cost_calculator: LLMCostCalculator::new(),
            model_info: Some(model_info),
        }
    }

    /// Set the model info for cost calculation
    pub fn set_model(&mut self, model_info: ModelInfo) {
        self.model_info = Some(model_info);
    }

    /// Aggregate a single execution trace
    pub fn aggregate_trace(&self, trace: &ExecutionTrace) -> AggregatedTrace {
        let mut summary = TraceSummary::empty();

        for step in &trace.steps {
            summary.total_steps += 1;
            summary.total_duration_ms += step.duration_ms;

            if step.success {
                summary.successful_steps += 1;
            } else {
                summary.failed_steps += 1;
            }

            if let Some(ref usage) = step.token_usage {
                summary.prompt_tokens += usage.prompt_tokens;
                summary.completion_tokens += usage.completion_tokens;
                summary.total_tokens += usage.total_tokens;
            }
        }

        // Calculate cost if we have model info
        if let Some(ref model_info) = self.model_info {
            let usage = crate::llm::TokenUsage {
                prompt_tokens: summary.prompt_tokens,
                completion_tokens: summary.completion_tokens,
                total_tokens: summary.total_tokens,
            };
            summary.estimated_cost_usd = self.cost_calculator.calculate_cost(model_info, &usage);
        }

        AggregatedTrace {
            name: trace.workflow_name.clone(),
            steps: trace.steps.clone(),
            nested: Vec::new(),
            summary,
        }
    }

    /// Aggregate multiple traces
    pub fn aggregate_traces(&self, traces: &[ExecutionTrace]) -> AggregatedTrace {
        let mut combined = AggregatedTrace {
            name: "aggregated".to_string(),
            steps: Vec::new(),
            nested: traces.to_vec(),
            summary: TraceSummary::empty(),
        };

        for trace in traces {
            let aggregated = self.aggregate_trace(trace);
            combined.summary.merge(&aggregated.summary);
        }

        combined
    }

    /// Calculate summary from token usage
    pub fn summarize_usage(&self, usage: &TokenUsageTrace) -> TraceSummary {
        let mut summary = TraceSummary::empty();
        summary.prompt_tokens = usage.prompt_tokens;
        summary.completion_tokens = usage.completion_tokens;
        summary.total_tokens = usage.total_tokens;

        if let Some(ref model_info) = self.model_info {
            let llm_usage = crate::llm::TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            };
            summary.estimated_cost_usd = self.cost_calculator.calculate_cost(model_info, &llm_usage);
        }

        summary
    }
}

impl Default for TraceAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_trace() -> ExecutionTrace {
        let mut trace = ExecutionTrace::new("test");
        trace.add_step(
            StepTrace::success("step1", serde_json::json!({}), serde_json::json!({}), 100)
                .with_token_usage(100, 50, 150),
        );
        trace.add_step(
            StepTrace::success("step2", serde_json::json!({}), serde_json::json!({}), 200)
                .with_token_usage(150, 75, 225),
        );
        trace
    }

    #[test]
    fn test_aggregate_trace() {
        let trace = create_test_trace();
        let aggregator = TraceAggregator::new();

        let aggregated = aggregator.aggregate_trace(&trace);

        assert_eq!(aggregated.summary.total_steps, 2);
        assert_eq!(aggregated.summary.successful_steps, 2);
        assert_eq!(aggregated.summary.total_duration_ms, 300);
        assert_eq!(aggregated.summary.prompt_tokens, 250);
        assert_eq!(aggregated.summary.completion_tokens, 125);
    }

    #[test]
    fn test_aggregate_with_cost() {
        let trace = create_test_trace();
        let aggregator = TraceAggregator::with_model(ModelInfo {
            provider: "openai".to_string(),
            model_name: "gpt-4".to_string(),
        });

        let aggregated = aggregator.aggregate_trace(&trace);

        assert!(aggregated.summary.estimated_cost_usd > 0.0);
    }

    #[test]
    fn test_aggregate_multiple_traces() {
        let trace1 = create_test_trace();
        let trace2 = create_test_trace();
        let aggregator = TraceAggregator::new();

        let aggregated = aggregator.aggregate_traces(&[trace1, trace2]);

        assert_eq!(aggregated.summary.total_steps, 4);
        assert_eq!(aggregated.nested.len(), 2);
    }

    #[test]
    fn test_summary_merge() {
        let mut summary1 = TraceSummary::empty();
        summary1.total_steps = 5;
        summary1.prompt_tokens = 100;

        let mut summary2 = TraceSummary::empty();
        summary2.total_steps = 3;
        summary2.prompt_tokens = 50;

        summary1.merge(&summary2);

        assert_eq!(summary1.total_steps, 8);
        assert_eq!(summary1.prompt_tokens, 150);
    }
}
