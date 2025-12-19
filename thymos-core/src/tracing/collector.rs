//! Trace Collection

use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime};

use crate::llm::ModelInfo;
use crate::workflow::{ExecutionTrace, StepTrace};

use super::aggregator::{TraceAggregator, TraceSummary};

/// A complete trace report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReport {
    /// Report ID
    pub id: String,
    /// Workflow name
    pub workflow_name: String,
    /// When the trace started
    pub started_at: SystemTime,
    /// When the trace completed
    pub completed_at: Option<SystemTime>,
    /// All step traces
    pub steps: Vec<StepTrace>,
    /// Nested execution traces
    pub nested_traces: Vec<ExecutionTrace>,
    /// Aggregated summary
    pub summary: TraceSummary,
    /// Custom tags
    pub tags: std::collections::HashMap<String, String>,
    /// Whether the workflow succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Collects traces during workflow execution
pub struct TraceCollector {
    /// Workflow name
    workflow_name: String,
    /// Unique trace ID
    trace_id: String,
    /// Start time
    started_at: Instant,
    /// System time when started
    started_at_system: SystemTime,
    /// Collected step traces
    steps: Vec<StepTrace>,
    /// Nested execution traces
    nested_traces: Vec<ExecutionTrace>,
    /// Custom tags
    tags: std::collections::HashMap<String, String>,
    /// Model info for cost calculation
    model_info: Option<ModelInfo>,
}

impl TraceCollector {
    /// Create a new trace collector
    pub fn new(workflow_name: impl Into<String>) -> Self {
        Self {
            workflow_name: workflow_name.into(),
            trace_id: generate_trace_id(),
            started_at: Instant::now(),
            started_at_system: SystemTime::now(),
            steps: Vec::new(),
            nested_traces: Vec::new(),
            tags: std::collections::HashMap::new(),
            model_info: None,
        }
    }

    /// Create with a specific trace ID
    pub fn with_id(workflow_name: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            workflow_name: workflow_name.into(),
            trace_id: trace_id.into(),
            started_at: Instant::now(),
            started_at_system: SystemTime::now(),
            steps: Vec::new(),
            nested_traces: Vec::new(),
            tags: std::collections::HashMap::new(),
            model_info: None,
        }
    }

    /// Set model info for cost calculation
    pub fn set_model(&mut self, model_info: ModelInfo) {
        self.model_info = Some(model_info);
    }

    /// Get the trace ID
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    /// Add a tag
    pub fn add_tag(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.tags.insert(key.into(), value.into());
    }

    /// Add a step trace
    pub fn add_step(&mut self, step: StepTrace) {
        self.steps.push(step);
    }

    /// Add a nested execution trace
    pub fn add_nested(&mut self, trace: ExecutionTrace) {
        self.nested_traces.push(trace);
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    /// Get step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Finalize and generate the report
    pub fn finalize(self) -> TraceReport {
        self.finalize_with_result(true, None)
    }

    /// Finalize with success/error status
    pub fn finalize_with_result(self, success: bool, error: Option<String>) -> TraceReport {
        let aggregator = if let Some(model_info) = self.model_info {
            TraceAggregator::with_model(model_info)
        } else {
            TraceAggregator::new()
        };

        // Create a temporary execution trace for aggregation
        let mut exec_trace = ExecutionTrace::new(&self.workflow_name);
        for step in &self.steps {
            exec_trace.add_step(step.clone());
        }

        let mut aggregated = aggregator.aggregate_trace(&exec_trace);

        // Add nested trace summaries
        for nested in &self.nested_traces {
            let nested_agg = aggregator.aggregate_trace(nested);
            aggregated.summary.merge(&nested_agg.summary);
        }

        TraceReport {
            id: self.trace_id,
            workflow_name: self.workflow_name,
            started_at: self.started_at_system,
            completed_at: Some(SystemTime::now()),
            steps: self.steps,
            nested_traces: self.nested_traces,
            summary: aggregated.summary,
            tags: self.tags,
            success,
            error,
        }
    }
}

/// Generate a unique trace ID
fn generate_trace_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    format!("trace-{:x}-{:04x}", timestamp, count % 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_creation() {
        let collector = TraceCollector::new("test-workflow");
        assert!(!collector.trace_id().is_empty());
        assert_eq!(collector.step_count(), 0);
    }

    #[test]
    fn test_collector_with_id() {
        let collector = TraceCollector::with_id("test", "custom-id");
        assert_eq!(collector.trace_id(), "custom-id");
    }

    #[test]
    fn test_add_step() {
        let mut collector = TraceCollector::new("test");
        collector.add_step(StepTrace::success(
            "step1",
            serde_json::json!({}),
            serde_json::json!({}),
            100,
        ));

        assert_eq!(collector.step_count(), 1);
    }

    #[test]
    fn test_add_tags() {
        let mut collector = TraceCollector::new("test");
        collector.add_tag("env", "production");
        collector.add_tag("user_id", "123");

        let report = collector.finalize();
        assert_eq!(report.tags.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_finalize() {
        let mut collector = TraceCollector::new("test-workflow");
        collector.add_step(
            StepTrace::success("step1", serde_json::json!({}), serde_json::json!({}), 100)
                .with_token_usage(100, 50, 150),
        );

        let report = collector.finalize();

        assert_eq!(report.workflow_name, "test-workflow");
        assert!(report.success);
        assert!(report.completed_at.is_some());
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.summary.prompt_tokens, 100);
    }

    #[test]
    fn test_finalize_with_error() {
        let collector = TraceCollector::new("test");
        let report = collector.finalize_with_result(false, Some("Something failed".to_string()));

        assert!(!report.success);
        assert_eq!(report.error, Some("Something failed".to_string()));
    }

    #[test]
    fn test_unique_trace_ids() {
        let id1 = TraceCollector::new("test").trace_id().to_string();
        let id2 = TraceCollector::new("test").trace_id().to_string();

        assert_ne!(id1, id2);
    }
}
