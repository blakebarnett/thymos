//! Orchestrator-Workers Workflow Pattern
//!
//! Central orchestrator delegates tasks to specialized workers and synthesizes results.

use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::LLMProvider;

use super::chain::Chain;
use super::execution::{WorkflowError, WorkflowResult};
use super::planner::{Plan, Planner, SubTask};
use super::step::{Step, StepOutput};

/// A worker that can execute specific tasks
pub struct Worker {
    /// Worker name
    pub name: String,
    /// Capabilities this worker provides
    pub capabilities: Vec<String>,
    /// Handler (step or chain)
    pub handler: WorkerHandler,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("name", &self.name)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

/// Handler type for a worker
pub enum WorkerHandler {
    Step(Step),
    Chain(Chain),
}

impl std::fmt::Debug for WorkerHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerHandler::Step(s) => f.debug_tuple("Step").field(s).finish(),
            WorkerHandler::Chain(c) => f.debug_tuple("Chain").field(c).finish(),
        }
    }
}

impl Worker {
    /// Create a new worker with a step handler
    pub fn step(name: impl Into<String>, capabilities: Vec<String>, step: Step) -> Self {
        Self {
            name: name.into(),
            capabilities,
            handler: WorkerHandler::Step(step),
        }
    }

    /// Create a new worker with a chain handler
    pub fn chain(name: impl Into<String>, capabilities: Vec<String>, chain: Chain) -> Self {
        Self {
            name: name.into(),
            capabilities,
            handler: WorkerHandler::Chain(chain),
        }
    }

    /// Check if worker has a capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// Execute the worker
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<StepOutput> {
        match &self.handler {
            WorkerHandler::Step(step) => {
                let (output, _) = step.execute(input, provider).await?;
                Ok(output)
            }
            WorkerHandler::Chain(chain) => {
                let (output, _) = chain.execute(input, provider).await?;
                Ok(output)
            }
        }
    }
}

/// Result of a single task execution
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub output: StepOutput,
    pub worker_name: String,
    pub duration_ms: u64,
}

/// Orchestrator execution trace
#[derive(Debug, Clone)]
pub struct OrchestratorTrace {
    /// Orchestrator name
    pub name: String,
    /// The plan that was executed
    pub plan: Plan,
    /// Results from each task
    pub task_results: Vec<TaskResult>,
    /// Tasks that failed
    pub failures: Vec<(String, String)>,
    /// Total duration
    pub total_duration_ms: u64,
    /// Whether synthesis was successful
    pub success: bool,
}

/// Orchestrator workflow
pub struct Orchestrator<P: Planner> {
    /// Orchestrator name
    name: String,
    /// Planner for task decomposition
    planner: P,
    /// Available workers
    workers: Vec<Worker>,
    /// Default worker for unmatched capabilities
    default_worker: Option<Worker>,
    /// Enable replanning on failures
    enable_replanning: bool,
    /// Maximum replan attempts
    max_replan_attempts: usize,
}

impl<P: Planner> std::fmt::Debug for Orchestrator<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("name", &self.name)
            .field("worker_count", &self.workers.len())
            .field("enable_replanning", &self.enable_replanning)
            .finish()
    }
}

impl<P: Planner> Orchestrator<P> {
    /// Create a new orchestrator builder
    pub fn builder(planner: P) -> OrchestratorBuilder<P> {
        OrchestratorBuilder::new(planner)
    }

    /// Get the orchestrator name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get available capabilities
    pub fn available_capabilities(&self) -> Vec<String> {
        let mut caps: Vec<String> = self
            .workers
            .iter()
            .flat_map(|w| w.capabilities.clone())
            .collect();
        caps.sort();
        caps.dedup();
        caps
    }

    /// Find a worker for a capability
    fn find_worker(&self, capability: Option<&str>) -> Option<&Worker> {
        if let Some(cap) = capability {
            self.workers
                .iter()
                .find(|w| w.has_capability(cap))
                .or(self.default_worker.as_ref())
        } else {
            self.default_worker.as_ref().or(self.workers.first())
        }
    }

    /// Execute the orchestrator workflow
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, OrchestratorTrace)> {
        let start = std::time::Instant::now();

        // Create the plan
        let capabilities = self.available_capabilities();
        let mut plan = self.planner.plan(&input, &capabilities).await?;

        let mut task_results: Vec<TaskResult> = Vec::new();
        let mut failures: Vec<(String, String)> = Vec::new();
        let mut replan_count = 0;

        loop {
            // Execute tasks in dependency order
            let ordered_tasks = plan.tasks_in_order();
            let mut completed: HashMap<String, serde_json::Value> = HashMap::new();

            for task in ordered_tasks {
                // Prepare input with completed task results
                let mut task_input = task.input.clone();
                if task_input.is_null() {
                    task_input = input.clone();
                }

                // Add dependency results to input
                for dep_id in &task.dependencies {
                    if let Some(dep_result) = completed.get(dep_id) {
                        if let serde_json::Value::Object(ref mut obj) = task_input {
                            obj.insert(format!("dep_{}", dep_id), dep_result.clone());
                        }
                    }
                }

                // Find worker
                let worker = self
                    .find_worker(task.required_capability.as_deref())
                    .ok_or_else(|| WorkflowError::InvalidConfig(format!(
                        "No worker for capability: {:?}",
                        task.required_capability
                    )))?;

                // Execute task
                let task_start = std::time::Instant::now();
                match worker.execute(task_input.clone(), provider).await {
                    Ok(output) => {
                        completed.insert(task.id.clone(), output.data.clone());
                        task_results.push(TaskResult {
                            task_id: task.id.clone(),
                            output,
                            worker_name: worker.name.clone(),
                            duration_ms: task_start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        failures.push((task.id.clone(), e.to_string()));
                    }
                }
            }

            // Check if replanning is needed
            if !failures.is_empty() && self.enable_replanning && replan_count < self.max_replan_attempts {
                let completed_results: Vec<_> = task_results
                    .iter()
                    .map(|r| (r.task_id.clone(), r.output.data.clone()))
                    .collect();

                if let Ok(Some(new_plan)) = self.planner.replan(&plan, &completed_results, &failures).await {
                    plan = new_plan;
                    replan_count += 1;
                    failures.clear();
                    continue;
                }
            }

            break;
        }

        // Synthesize results
        let synthesized = self.synthesize(&plan, &task_results)?;

        let trace = OrchestratorTrace {
            name: self.name.clone(),
            plan,
            task_results,
            failures,
            total_duration_ms: start.elapsed().as_millis() as u64,
            success: true,
        };

        Ok((synthesized, trace))
    }

    fn synthesize(&self, plan: &Plan, results: &[TaskResult]) -> WorkflowResult<StepOutput> {
        match plan.synthesis_strategy.as_str() {
            "merge" => {
                let mut merged = serde_json::Map::new();
                for result in results {
                    merged.insert(result.task_id.clone(), result.output.data.clone());
                }
                Ok(StepOutput::new(serde_json::Value::Object(merged)))
            }
            "concatenate" => {
                let texts: Vec<String> = results
                    .iter()
                    .filter_map(|r| r.output.data.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(StepOutput::new(serde_json::json!(texts.join("\n\n"))))
            }
            "array" => {
                let values: Vec<_> = results.iter().map(|r| r.output.data.clone()).collect();
                Ok(StepOutput::new(serde_json::Value::Array(values)))
            }
            _ => {
                // Default to merge
                let mut merged = serde_json::Map::new();
                for result in results {
                    merged.insert(result.task_id.clone(), result.output.data.clone());
                }
                Ok(StepOutput::new(serde_json::Value::Object(merged)))
            }
        }
    }
}

/// Builder for Orchestrator
pub struct OrchestratorBuilder<P: Planner> {
    name: String,
    planner: P,
    workers: Vec<Worker>,
    default_worker: Option<Worker>,
    enable_replanning: bool,
    max_replan_attempts: usize,
}

impl<P: Planner> OrchestratorBuilder<P> {
    /// Create a new orchestrator builder
    pub fn new(planner: P) -> Self {
        Self {
            name: "orchestrator".to_string(),
            planner,
            workers: Vec::new(),
            default_worker: None,
            enable_replanning: false,
            max_replan_attempts: 3,
        }
    }

    /// Set the orchestrator name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a worker
    pub fn worker(mut self, worker: Worker) -> Self {
        self.workers.push(worker);
        self
    }

    /// Set the default worker
    pub fn default_worker(mut self, worker: Worker) -> Self {
        self.default_worker = Some(worker);
        self
    }

    /// Enable replanning on failures
    pub fn enable_replanning(mut self, enable: bool) -> Self {
        self.enable_replanning = enable;
        self
    }

    /// Set maximum replan attempts
    pub fn max_replan_attempts(mut self, max: usize) -> Self {
        self.max_replan_attempts = max;
        self
    }

    /// Build the orchestrator
    pub fn build(self) -> Orchestrator<P> {
        Orchestrator {
            name: self.name,
            planner: self.planner,
            workers: self.workers,
            default_worker: self.default_worker,
            enable_replanning: self.enable_replanning,
            max_replan_attempts: self.max_replan_attempts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LLMConfig, LLMRequest, LLMResponse, ModelInfo};
    use crate::workflow::planner::StaticPlanner;
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
    fn test_worker_creation() {
        let worker = Worker::step(
            "researcher",
            vec!["research".to_string(), "analysis".to_string()],
            Step::transform("research", |_| Ok(serde_json::json!("researched"))).build(),
        );

        assert!(worker.has_capability("research"));
        assert!(worker.has_capability("analysis"));
        assert!(!worker.has_capability("coding"));
    }

    #[test]
    fn test_orchestrator_builder() {
        let plan = Plan::new("Test plan")
            .add_task(SubTask::new("t1", "Task 1").with_capability("research"));

        let planner = StaticPlanner::new(plan);

        let orchestrator = Orchestrator::builder(planner)
            .name("test-orchestrator")
            .worker(Worker::step(
                "researcher",
                vec!["research".to_string()],
                Step::transform("r", |_| Ok(serde_json::json!("done"))).build(),
            ))
            .enable_replanning(true)
            .max_replan_attempts(5)
            .build();

        assert_eq!(orchestrator.name(), "test-orchestrator");
        assert!(orchestrator.available_capabilities().contains(&"research".to_string()));
    }

    #[tokio::test]
    async fn test_orchestrator_execution() {
        let plan = Plan::new("Simple plan")
            .add_task(SubTask::new("t1", "First task").with_capability("process"))
            .add_task(SubTask::new("t2", "Second task").with_capability("process").depends_on("t1"))
            .with_synthesis("merge");

        let planner = StaticPlanner::new(plan);

        let orchestrator = Orchestrator::builder(planner)
            .name("test")
            .worker(Worker::step(
                "processor",
                vec!["process".to_string()],
                Step::transform("proc", |input| {
                    Ok(serde_json::json!({"processed": input}))
                })
                .build(),
            ))
            .build();

        let provider = MockProvider;
        let (output, trace) = orchestrator
            .execute(serde_json::json!({"data": "test"}), &provider)
            .await
            .unwrap();

        assert!(trace.success);
        assert_eq!(trace.task_results.len(), 2);
        assert!(output.data.get("t1").is_some());
        assert!(output.data.get("t2").is_some());
    }

    #[tokio::test]
    async fn test_orchestrator_with_default_worker() {
        let plan = Plan::new("Plan with unknown capability")
            .add_task(SubTask::new("t1", "Unknown task").with_capability("unknown"));

        let planner = StaticPlanner::new(plan);

        let orchestrator = Orchestrator::builder(planner)
            .default_worker(Worker::step(
                "default",
                vec![],
                Step::transform("default", |_| Ok(serde_json::json!("handled by default"))).build(),
            ))
            .build();

        let provider = MockProvider;
        let (output, trace) = orchestrator
            .execute(serde_json::json!({}), &provider)
            .await
            .unwrap();

        assert!(trace.success);
        assert_eq!(trace.task_results[0].worker_name, "default");
    }
}
