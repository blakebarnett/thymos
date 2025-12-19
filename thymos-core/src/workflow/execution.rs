//! Workflow execution types and error handling

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Error type for workflow operations
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Step execution failed
    #[error("Step '{step}' failed: {message}")]
    StepFailed { step: String, message: String },

    /// Gate halted execution
    #[error("Gate '{gate}' halted execution: {reason}")]
    GateHalted { gate: String, reason: String },

    /// LLM provider error
    #[error("LLM error: {0}")]
    LLMError(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Step output parsing failed
    #[error("Failed to parse step output: {0}")]
    ParseError(String),

    /// Workflow timeout
    #[error("Workflow timed out after {0:?}")]
    Timeout(Duration),
}

/// Result type for workflow operations
pub type WorkflowResult<T> = Result<T, WorkflowError>;

/// Trace of a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTrace {
    /// Step name
    pub step_name: String,

    /// Step input (serialized)
    pub input: serde_json::Value,

    /// Step output (serialized)
    pub output: serde_json::Value,

    /// Duration of step execution
    pub duration_ms: u64,

    /// Whether the step succeeded
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,

    /// Token usage if available
    pub token_usage: Option<TokenUsageTrace>,
}

/// Token usage trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageTrace {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

impl StepTrace {
    /// Create a successful step trace
    pub fn success(
        step_name: impl Into<String>,
        input: serde_json::Value,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name: step_name.into(),
            input,
            output,
            duration_ms,
            success: true,
            error: None,
            token_usage: None,
        }
    }

    /// Create a failed step trace
    pub fn failure(
        step_name: impl Into<String>,
        input: serde_json::Value,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name: step_name.into(),
            input,
            output: serde_json::Value::Null,
            duration_ms,
            success: false,
            error: Some(error.into()),
            token_usage: None,
        }
    }

    /// Add token usage to the trace
    pub fn with_token_usage(
        mut self,
        prompt_tokens: usize,
        completion_tokens: usize,
        total_tokens: usize,
    ) -> Self {
        self.token_usage = Some(TokenUsageTrace {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        });
        self
    }
}

/// Complete execution trace for a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Workflow name
    pub workflow_name: String,

    /// Step traces in execution order
    pub steps: Vec<StepTrace>,

    /// Total duration
    pub total_duration_ms: u64,

    /// Whether the workflow completed successfully
    pub success: bool,

    /// Final error if failed
    pub error: Option<String>,

    /// Index of the step that halted execution (if halted by gate)
    pub halted_at: Option<usize>,
}

impl ExecutionTrace {
    /// Create a new execution trace
    pub fn new(workflow_name: impl Into<String>) -> Self {
        Self {
            workflow_name: workflow_name.into(),
            steps: Vec::new(),
            total_duration_ms: 0,
            success: true,
            error: None,
            halted_at: None,
        }
    }

    /// Add a step trace
    pub fn add_step(&mut self, step: StepTrace) {
        self.total_duration_ms += step.duration_ms;
        if !step.success {
            self.success = false;
            self.error = step.error.clone();
        }
        self.steps.push(step);
    }

    /// Mark as halted at a specific step
    pub fn mark_halted(&mut self, step_index: usize, reason: impl Into<String>) {
        self.halted_at = Some(step_index);
        self.error = Some(reason.into());
    }

    /// Get total token usage across all steps
    pub fn total_token_usage(&self) -> Option<TokenUsageTrace> {
        let mut total = TokenUsageTrace {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        let mut has_usage = false;
        for step in &self.steps {
            if let Some(usage) = &step.token_usage {
                total.prompt_tokens += usage.prompt_tokens;
                total.completion_tokens += usage.completion_tokens;
                total.total_tokens += usage.total_tokens;
                has_usage = true;
            }
        }

        if has_usage {
            Some(total)
        } else {
            None
        }
    }

    /// Get the number of completed steps
    pub fn completed_steps(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_trace_success() {
        let trace = StepTrace::success(
            "test_step",
            serde_json::json!({"input": "value"}),
            serde_json::json!({"output": "result"}),
            100,
        );

        assert!(trace.success);
        assert!(trace.error.is_none());
        assert_eq!(trace.step_name, "test_step");
    }

    #[test]
    fn test_step_trace_failure() {
        let trace = StepTrace::failure(
            "test_step",
            serde_json::json!({}),
            "Something went wrong",
            50,
        );

        assert!(!trace.success);
        assert!(trace.error.is_some());
    }

    #[test]
    fn test_execution_trace() {
        let mut trace = ExecutionTrace::new("test_workflow");

        trace.add_step(StepTrace::success(
            "step1",
            serde_json::json!({}),
            serde_json::json!({}),
            100,
        ));

        trace.add_step(StepTrace::success(
            "step2",
            serde_json::json!({}),
            serde_json::json!({}),
            150,
        )
        .with_token_usage(100, 50, 150));

        assert!(trace.success);
        assert_eq!(trace.completed_steps(), 2);
        assert_eq!(trace.total_duration_ms, 250);

        let usage = trace.total_token_usage().unwrap();
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_execution_trace_failure() {
        let mut trace = ExecutionTrace::new("test_workflow");

        trace.add_step(StepTrace::success(
            "step1",
            serde_json::json!({}),
            serde_json::json!({}),
            100,
        ));

        trace.add_step(StepTrace::failure(
            "step2",
            serde_json::json!({}),
            "Error occurred",
            50,
        ));

        assert!(!trace.success);
        assert!(trace.error.is_some());
    }
}
