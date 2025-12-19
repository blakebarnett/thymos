//! Parallel Workflow Pattern
//!
//! Executes multiple branches concurrently with result aggregation.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::llm::LLMProvider;

use super::aggregator::{Aggregator, AllSuccess};
use super::chain::Chain;
use super::execution::{ExecutionTrace, StepTrace, WorkflowError, WorkflowResult};
use super::step::{Step, StepOutput};

/// Branch in a parallel workflow
pub enum Branch {
    /// Single step branch
    Step(Step),
    /// Chain branch
    Chain(Chain),
}

impl std::fmt::Debug for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Step(step) => f.debug_tuple("Step").field(step).finish(),
            Branch::Chain(chain) => f.debug_tuple("Chain").field(chain).finish(),
        }
    }
}

impl Branch {
    /// Execute the branch
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, Option<ExecutionTrace>)> {
        match self {
            Branch::Step(step) => {
                let (output, trace) = step.execute(input, provider).await?;
                Ok((output, Some(ExecutionTrace {
                    workflow_name: step.name.clone(),
                    steps: vec![trace],
                    total_duration_ms: 0,
                    success: true,
                    error: None,
                    halted_at: None,
                })))
            }
            Branch::Chain(chain) => {
                let (output, trace) = chain.execute(input, provider).await?;
                Ok((output, Some(trace)))
            }
        }
    }

    /// Get the branch name
    pub fn name(&self) -> &str {
        match self {
            Branch::Step(step) => &step.name,
            Branch::Chain(chain) => chain.name(),
        }
    }
}

/// Parallel workflow configuration
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Maximum concurrent branches
    pub max_concurrency: usize,
    /// Timeout per branch
    pub branch_timeout: Option<Duration>,
    /// Overall timeout
    pub total_timeout: Option<Duration>,
    /// Continue on partial failure
    pub continue_on_failure: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 10,
            branch_timeout: None,
            total_timeout: None,
            continue_on_failure: false,
        }
    }
}

/// Parallel workflow for concurrent execution
pub struct Parallel {
    /// Workflow name
    name: String,
    /// Branches to execute
    branches: Vec<Branch>,
    /// Aggregator for results
    aggregator: Box<dyn Aggregator>,
    /// Configuration
    config: ParallelConfig,
}

impl std::fmt::Debug for Parallel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parallel")
            .field("name", &self.name)
            .field("branch_count", &self.branches.len())
            .field("config", &self.config)
            .finish()
    }
}

impl Parallel {
    /// Create a new parallel builder
    pub fn builder() -> ParallelBuilder {
        ParallelBuilder::new()
    }

    /// Get the workflow name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of branches
    pub fn branch_count(&self) -> usize {
        self.branches.len()
    }

    /// Execute all branches in parallel
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, ParallelExecutionTrace)> {
        let start = std::time::Instant::now();
        
        // Create semaphore for concurrency limiting
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));
        
        // Spawn all branches
        let mut handles = Vec::new();
        
        for branch in &self.branches {
            let input_clone = input.clone();
            let branch_name = branch.name().to_string();
            let branch_timeout = self.config.branch_timeout;
            let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
                WorkflowError::StepFailed {
                    step: branch_name.clone(),
                    message: "Failed to acquire semaphore".to_string(),
                }
            })?;

            // Execute branch
            let branch_start = std::time::Instant::now();
            let result = match branch_timeout {
                Some(timeout) => {
                    match tokio::time::timeout(timeout, branch.execute(input_clone, provider)).await {
                        Ok(result) => result,
                        Err(_) => Err(WorkflowError::Timeout(timeout)),
                    }
                }
                None => branch.execute(input_clone, provider).await,
            };
            
            let duration_ms = branch_start.elapsed().as_millis() as u64;
            drop(permit);

            handles.push((branch_name, result, duration_ms));
        }

        // Collect results
        let mut branch_traces = Vec::new();
        let mut results = Vec::new();

        for (name, result, duration_ms) in handles {
            let success = result.is_ok();
            let error = result.as_ref().err().map(|e| e.to_string());
            
            branch_traces.push(BranchTrace {
                name: name.clone(),
                duration_ms,
                success,
                error,
            });

            results.push(result.map(|(output, _)| output));
        }

        // Aggregate results
        let aggregated = self.aggregator.aggregate(results)?;

        let total_duration_ms = start.elapsed().as_millis() as u64;

        let trace = ParallelExecutionTrace {
            workflow_name: self.name.clone(),
            branches: branch_traces,
            total_duration_ms,
            success: true,
        };

        Ok((aggregated, trace))
    }
}

/// Trace for a single branch
#[derive(Debug, Clone)]
pub struct BranchTrace {
    /// Branch name
    pub name: String,
    /// Execution duration
    pub duration_ms: u64,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Parallel execution trace
#[derive(Debug, Clone)]
pub struct ParallelExecutionTrace {
    /// Workflow name
    pub workflow_name: String,
    /// Branch traces
    pub branches: Vec<BranchTrace>,
    /// Total duration
    pub total_duration_ms: u64,
    /// Overall success
    pub success: bool,
}

impl ParallelExecutionTrace {
    /// Get count of successful branches
    pub fn successful_branches(&self) -> usize {
        self.branches.iter().filter(|b| b.success).count()
    }

    /// Get count of failed branches
    pub fn failed_branches(&self) -> usize {
        self.branches.iter().filter(|b| !b.success).count()
    }
}

/// Builder for Parallel workflows
pub struct ParallelBuilder {
    name: String,
    branches: Vec<Branch>,
    aggregator: Option<Box<dyn Aggregator>>,
    config: ParallelConfig,
}

impl ParallelBuilder {
    /// Create a new parallel builder
    pub fn new() -> Self {
        Self {
            name: "parallel".to_string(),
            branches: Vec::new(),
            aggregator: None,
            config: ParallelConfig::default(),
        }
    }

    /// Set the workflow name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a step branch
    pub fn step(mut self, step: Step) -> Self {
        self.branches.push(Branch::Step(step));
        self
    }

    /// Add a chain branch
    pub fn chain(mut self, chain: Chain) -> Self {
        self.branches.push(Branch::Chain(chain));
        self
    }

    /// Add a branch
    pub fn branch(mut self, branch: Branch) -> Self {
        self.branches.push(branch);
        self
    }

    /// Set the aggregator
    pub fn aggregator<A: Aggregator + 'static>(mut self, aggregator: A) -> Self {
        self.aggregator = Some(Box::new(aggregator));
        self
    }

    /// Set maximum concurrency
    pub fn max_concurrency(mut self, max: usize) -> Self {
        self.config.max_concurrency = max;
        self
    }

    /// Set branch timeout
    pub fn branch_timeout(mut self, timeout: Duration) -> Self {
        self.config.branch_timeout = Some(timeout);
        self
    }

    /// Set total timeout
    pub fn total_timeout(mut self, timeout: Duration) -> Self {
        self.config.total_timeout = Some(timeout);
        self
    }

    /// Continue on partial failure
    pub fn continue_on_failure(mut self, continue_: bool) -> Self {
        self.config.continue_on_failure = continue_;
        self
    }

    /// Build the parallel workflow
    pub fn build(self) -> Parallel {
        Parallel {
            name: self.name,
            branches: self.branches,
            aggregator: self.aggregator.unwrap_or_else(|| Box::new(AllSuccess)),
            config: self.config,
        }
    }
}

impl Default for ParallelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LLMConfig, LLMRequest, LLMResponse, ModelInfo};
    use crate::workflow::aggregator::{FirstSuccess, Merge};
    use async_trait::async_trait;

    struct MockProvider;

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> crate::error::Result<String> {
            Ok("mock".to_string())
        }

        async fn generate_request(&self, _request: &LLMRequest) -> crate::error::Result<LLMResponse> {
            Ok(LLMResponse {
                content: "mock".to_string(),
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

    #[test]
    fn test_parallel_builder() {
        let parallel = Parallel::builder()
            .name("test-parallel")
            .step(Step::transform("a", |_| Ok(serde_json::json!(1))).build())
            .step(Step::transform("b", |_| Ok(serde_json::json!(2))).build())
            .max_concurrency(5)
            .build();

        assert_eq!(parallel.name(), "test-parallel");
        assert_eq!(parallel.branch_count(), 2);
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let parallel = Parallel::builder()
            .name("test")
            .step(Step::transform("a", |_| Ok(serde_json::json!(1))).build())
            .step(Step::transform("b", |_| Ok(serde_json::json!(2))).build())
            .step(Step::transform("c", |_| Ok(serde_json::json!(3))).build())
            .build();

        let provider = MockProvider;
        let (output, trace) = parallel
            .execute(serde_json::json!("input"), &provider)
            .await
            .unwrap();

        // AllSuccess returns array
        assert_eq!(output.data, serde_json::json!([1, 2, 3]));
        assert_eq!(trace.successful_branches(), 3);
        assert_eq!(trace.failed_branches(), 0);
    }

    #[tokio::test]
    async fn test_parallel_with_first_success() {
        let parallel = Parallel::builder()
            .step(Step::transform("a", |_| Ok(serde_json::json!("first"))).build())
            .step(Step::transform("b", |_| Ok(serde_json::json!("second"))).build())
            .aggregator(FirstSuccess)
            .build();

        let provider = MockProvider;
        let (output, _) = parallel
            .execute(serde_json::json!("input"), &provider)
            .await
            .unwrap();

        // FirstSuccess returns the first one
        assert_eq!(output.data, serde_json::json!("first"));
    }

    #[tokio::test]
    async fn test_parallel_with_merge() {
        let parallel = Parallel::builder()
            .step(Step::transform("summary", |_| Ok(serde_json::json!("short version"))).build())
            .step(Step::transform("analysis", |_| Ok(serde_json::json!("detailed analysis"))).build())
            .aggregator(Merge::with_keys(&["summary", "analysis"]))
            .build();

        let provider = MockProvider;
        let (output, _) = parallel
            .execute(serde_json::json!("input"), &provider)
            .await
            .unwrap();

        assert_eq!(output.data["summary"], "short version");
        assert_eq!(output.data["analysis"], "detailed analysis");
    }

    #[test]
    fn test_parallel_config() {
        let parallel = Parallel::builder()
            .max_concurrency(3)
            .branch_timeout(Duration::from_secs(10))
            .total_timeout(Duration::from_secs(60))
            .continue_on_failure(true)
            .build();

        assert_eq!(parallel.config.max_concurrency, 3);
        assert_eq!(parallel.config.branch_timeout, Some(Duration::from_secs(10)));
        assert!(parallel.config.continue_on_failure);
    }
}
