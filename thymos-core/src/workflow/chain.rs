//! Chain Workflow Pattern
//!
//! Sequential execution where each step's output feeds the next step's input.

use crate::llm::LLMProvider;

use super::execution::{ExecutionTrace, WorkflowError, WorkflowResult};
use super::gate::Gate;
use super::step::{Step, StepOutput};

/// Configuration for chain execution
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// Whether to fail fast on step failure
    pub fail_fast: bool,
    /// Default value to use when a step fails (if not fail_fast)
    pub default_on_failure: Option<serde_json::Value>,
    /// Maximum number of steps to execute (None = unlimited)
    pub max_steps: Option<usize>,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            fail_fast: true,
            default_on_failure: None,
            max_steps: None,
        }
    }
}

/// Chain element - either a step or a gate
pub enum ChainElement {
    Step(Step),
    Gate(Gate),
}

/// Chain workflow for sequential LLM operations
///
/// Each step receives the output of the previous step as input.
/// Gates can be inserted between steps to conditionally halt execution.
pub struct Chain {
    /// Chain name
    name: String,
    /// Elements in the chain (steps and gates)
    elements: Vec<ChainElement>,
    /// Configuration
    config: ChainConfig,
}

impl std::fmt::Debug for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chain")
            .field("name", &self.name)
            .field("element_count", &self.elements.len())
            .field("config", &self.config)
            .finish()
    }
}

impl Chain {
    /// Create a new chain builder
    pub fn builder() -> ChainBuilder {
        ChainBuilder::new()
    }

    /// Get the chain name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Execute the chain with the given input
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, ExecutionTrace)> {
        let mut trace = ExecutionTrace::new(&self.name);
        let mut current_value = input;
        let mut final_output = StepOutput::new(serde_json::Value::Null);
        let mut step_count = 0;

        for (index, element) in self.elements.iter().enumerate() {
            // Check max steps limit
            if let Some(max) = self.config.max_steps {
                if step_count >= max {
                    break;
                }
            }

            match element {
                ChainElement::Step(step) => {
                    match step.execute(current_value.clone(), provider).await {
                        Ok((output, step_trace)) => {
                            current_value = output.data.clone();
                            final_output = output;
                            trace.add_step(step_trace);
                            step_count += 1;
                        }
                        Err(e) => {
                            if self.config.fail_fast {
                                trace.success = false;
                                trace.error = Some(e.to_string());
                                return Err(e);
                            } else if let Some(default) = &self.config.default_on_failure {
                                current_value = default.clone();
                                final_output = StepOutput::new(default.clone());
                            } else {
                                // Continue with null
                                current_value = serde_json::Value::Null;
                                final_output = StepOutput::new(serde_json::Value::Null);
                            }
                        }
                    }
                }
                ChainElement::Gate(gate) => {
                    if !gate.evaluate(&current_value) {
                        let reason = gate
                            .halt_message
                            .clone()
                            .unwrap_or_else(|| format!("Gate '{}' condition not met", gate.name));

                        trace.mark_halted(index, &reason);

                        return Err(WorkflowError::GateHalted {
                            gate: gate.name.clone(),
                            reason,
                        });
                    }
                }
            }
        }

        Ok((final_output, trace))
    }

    /// Execute the chain and return only the final output
    pub async fn run(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<serde_json::Value> {
        let (output, _) = self.execute(input, provider).await?;
        Ok(output.data)
    }
}

/// Builder for creating Chain workflows
pub struct ChainBuilder {
    name: String,
    elements: Vec<ChainElement>,
    config: ChainConfig,
}

impl ChainBuilder {
    /// Create a new chain builder
    pub fn new() -> Self {
        Self {
            name: "chain".to_string(),
            elements: Vec::new(),
            config: ChainConfig::default(),
        }
    }

    /// Set the chain name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a step to the chain
    pub fn step(mut self, step: Step) -> Self {
        self.elements.push(ChainElement::Step(step));
        self
    }

    /// Add a gate to the chain
    pub fn gate(mut self, gate: Gate) -> Self {
        self.elements.push(ChainElement::Gate(gate));
        self
    }

    /// Set fail-fast mode
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.config.fail_fast = fail_fast;
        self
    }

    /// Set default value on failure
    pub fn default_on_failure(mut self, value: serde_json::Value) -> Self {
        self.config.default_on_failure = Some(value);
        self.config.fail_fast = false;
        self
    }

    /// Set maximum steps to execute
    pub fn max_steps(mut self, max: usize) -> Self {
        self.config.max_steps = Some(max);
        self
    }

    /// Build the chain
    pub fn build(self) -> Chain {
        Chain {
            name: self.name,
            elements: self.elements,
            config: self.config,
        }
    }
}

impl Default for ChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LLMConfig, LLMRequest, LLMResponse, ModelInfo, TokenUsage};
    use async_trait::async_trait;

    struct MockProvider {
        responses: std::sync::Mutex<Vec<String>>,
    }

    impl MockProvider {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> crate::error::Result<String> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok("default response".to_string())
            } else {
                Ok(responses.remove(0))
            }
        }

        async fn generate_request(&self, _request: &LLMRequest) -> crate::error::Result<LLMResponse> {
            let mut responses = self.responses.lock().unwrap();
            let content = if responses.is_empty() {
                "default response".to_string()
            } else {
                responses.remove(0)
            };

            Ok(LLMResponse {
                content,
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            })
        }

        fn model_info(&self) -> ModelInfo {
            ModelInfo {
                provider: "mock".to_string(),
                model_name: "test".to_string(),
            }
        }
    }

    #[test]
    fn test_chain_builder() {
        let chain = Chain::builder()
            .name("test-chain")
            .step(Step::llm("step1", "Process: {{input}}").build())
            .step(Step::llm("step2", "Transform: {{input}}").build())
            .build();

        assert_eq!(chain.name(), "test-chain");
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn test_chain_with_gate() {
        let chain = Chain::builder()
            .name("gated-chain")
            .step(Step::llm("step1", "{{input}}").build())
            .gate(Gate::contains("has_continue", "continue"))
            .step(Step::llm("step2", "{{input}}").build())
            .build();

        assert_eq!(chain.len(), 3);
    }

    #[tokio::test]
    async fn test_chain_execution() {
        let provider = MockProvider::new(vec![
            "first result".to_string(),
            "second result".to_string(),
        ]);

        let chain = Chain::builder()
            .name("test")
            .step(Step::llm("step1", "{{input}}").build())
            .step(Step::llm("step2", "{{input}}").build())
            .build();

        let (output, trace) = chain
            .execute(serde_json::json!("start"), &provider)
            .await
            .unwrap();

        assert_eq!(output.data, serde_json::json!("second result"));
        assert!(trace.success);
        assert_eq!(trace.completed_steps(), 2);
    }

    #[tokio::test]
    async fn test_chain_with_transform() {
        let provider = MockProvider::new(vec![]);

        let chain = Chain::builder()
            .name("transform-chain")
            .step(
                Step::transform("double", |v| {
                    let n = v.as_i64().unwrap_or(0);
                    Ok(serde_json::json!(n * 2))
                })
                .build(),
            )
            .step(
                Step::transform("add_ten", |v| {
                    let n = v.as_i64().unwrap_or(0);
                    Ok(serde_json::json!(n + 10))
                })
                .build(),
            )
            .build();

        let result = chain.run(serde_json::json!(5), &provider).await.unwrap();

        // 5 * 2 = 10, 10 + 10 = 20
        assert_eq!(result, serde_json::json!(20));
    }

    #[tokio::test]
    async fn test_chain_gate_halt() {
        let provider = MockProvider::new(vec!["no match here".to_string()]);

        let chain = Chain::builder()
            .name("gated")
            .step(Step::llm("step1", "{{input}}").build())
            .gate(
                Gate::contains("requires_continue", "continue")
                    .with_halt_message("Output must contain 'continue'"),
            )
            .step(Step::llm("step2", "{{input}}").build())
            .build();

        let result = chain.execute(serde_json::json!("start"), &provider).await;

        assert!(result.is_err());
        match result {
            Err(WorkflowError::GateHalted { gate, .. }) => {
                assert_eq!(gate, "requires_continue");
            }
            _ => panic!("Expected GateHalted error"),
        }
    }

    #[tokio::test]
    async fn test_chain_token_tracking() {
        let provider = MockProvider::new(vec!["result".to_string()]);

        let chain = Chain::builder()
            .name("tracked")
            .step(Step::llm("step1", "{{input}}").build())
            .build();

        let (_, trace) = chain
            .execute(serde_json::json!("input"), &provider)
            .await
            .unwrap();

        let usage = trace.total_token_usage().unwrap();
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_chain_config() {
        let chain = Chain::builder()
            .name("configured")
            .fail_fast(false)
            .default_on_failure(serde_json::json!({"error": true}))
            .max_steps(5)
            .build();

        assert!(!chain.config.fail_fast);
        assert!(chain.config.default_on_failure.is_some());
        assert_eq!(chain.config.max_steps, Some(5));
    }
}
