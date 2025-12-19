//! Planners for Orchestrator-Workers Pattern
//!
//! Planners decompose complex tasks into subtasks for workers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};

use super::execution::{WorkflowError, WorkflowResult};

/// A subtask to be delegated to a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    /// Task identifier
    pub id: String,
    /// Task description
    pub description: String,
    /// Required worker capability
    pub required_capability: Option<String>,
    /// Input data for the task
    pub input: serde_json::Value,
    /// Dependencies on other task IDs
    pub dependencies: Vec<String>,
    /// Priority (higher = more important)
    pub priority: i32,
}

impl SubTask {
    /// Create a new subtask
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            required_capability: None,
            input: serde_json::Value::Null,
            dependencies: Vec::new(),
            priority: 0,
        }
    }

    /// Set the required capability
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capability = Some(capability.into());
        self
    }

    /// Set the input data
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    /// Add a dependency
    pub fn depends_on(mut self, task_id: impl Into<String>) -> Self {
        self.dependencies.push(task_id.into());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Execution plan produced by a planner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Plan description
    pub description: String,
    /// Subtasks to execute
    pub tasks: Vec<SubTask>,
    /// Strategy for combining results
    pub synthesis_strategy: String,
}

impl Plan {
    /// Create a new plan
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            tasks: Vec::new(),
            synthesis_strategy: "merge".to_string(),
        }
    }

    /// Add a task
    pub fn add_task(mut self, task: SubTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Set synthesis strategy
    pub fn with_synthesis(mut self, strategy: impl Into<String>) -> Self {
        self.synthesis_strategy = strategy.into();
        self
    }

    /// Get tasks in dependency order
    pub fn tasks_in_order(&self) -> Vec<&SubTask> {
        let mut result = Vec::new();
        let mut completed: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut remaining: Vec<&SubTask> = self.tasks.iter().collect();

        while !remaining.is_empty() {
            let ready: Vec<_> = remaining
                .iter()
                .filter(|t| t.dependencies.iter().all(|d| completed.contains(d.as_str())))
                .cloned()
                .collect();

            if ready.is_empty() && !remaining.is_empty() {
                // Circular dependency or missing dependency - just return in order
                result.extend(remaining);
                break;
            }

            for task in &ready {
                completed.insert(&task.id);
                result.push(*task);
            }

            remaining.retain(|t| !completed.contains(t.id.as_str()));
        }

        result
    }
}

/// Trait for decomposing tasks into subtasks
#[async_trait]
pub trait Planner: Send + Sync {
    /// Create a plan for the given input
    async fn plan(
        &self,
        input: &serde_json::Value,
        available_capabilities: &[String],
    ) -> WorkflowResult<Plan>;

    /// Optionally refine a plan based on partial results
    async fn replan(
        &self,
        _original_plan: &Plan,
        _completed_results: &[(String, serde_json::Value)],
        _failures: &[(String, String)],
    ) -> WorkflowResult<Option<Plan>> {
        Ok(None) // Default: no replanning
    }
}

/// Simple static planner with pre-defined tasks
pub struct StaticPlanner {
    plan: Plan,
}

impl StaticPlanner {
    /// Create a static planner with a pre-defined plan
    pub fn new(plan: Plan) -> Self {
        Self { plan }
    }
}

#[async_trait]
impl Planner for StaticPlanner {
    async fn plan(
        &self,
        _input: &serde_json::Value,
        _available_capabilities: &[String],
    ) -> WorkflowResult<Plan> {
        Ok(self.plan.clone())
    }
}

/// LLM-based planner that uses an LLM to decompose tasks
pub struct LLMPlanner {
    system_prompt: String,
}

impl LLMPlanner {
    /// Create a new LLM planner
    pub fn new() -> Self {
        Self {
            system_prompt: Self::default_system_prompt(),
        }
    }

    /// Create with a custom system prompt
    pub fn with_system_prompt(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
        }
    }

    fn default_system_prompt() -> String {
        r#"You are a task decomposition planner. Given a complex task and available worker capabilities, break it down into subtasks.

Output your plan as JSON with this format:
{
  "description": "Brief plan description",
  "tasks": [
    {
      "id": "task1",
      "description": "What this task does",
      "required_capability": "capability_name",
      "dependencies": []
    }
  ],
  "synthesis_strategy": "merge"
}

Available synthesis strategies: merge, concatenate, summarize"#.to_string()
    }

    /// Plan using an LLM provider
    pub async fn plan_with_provider(
        &self,
        input: &serde_json::Value,
        available_capabilities: &[String],
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<Plan> {
        let input_str = serde_json::to_string_pretty(input).unwrap_or_default();
        let caps_str = available_capabilities.join(", ");

        let user_prompt = format!(
            "Task: {}\n\nAvailable worker capabilities: {}\n\nCreate a plan to accomplish this task.",
            input_str, caps_str
        );

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
            max_tokens: Some(1000),
            stop_sequences: Vec::new(),
        };

        let response = provider
            .generate_request(&request)
            .await
            .map_err(|e| WorkflowError::LLMError(e.to_string()))?;

        // Parse the JSON response
        let plan: Plan = serde_json::from_str(&response.content)
            .map_err(|e| WorkflowError::ParseError(format!("Failed to parse plan: {}", e)))?;

        Ok(plan)
    }
}

impl Default for LLMPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtask_creation() {
        let task = SubTask::new("task1", "Do something")
            .with_capability("research")
            .with_input(serde_json::json!({"query": "test"}))
            .depends_on("task0")
            .with_priority(5);

        assert_eq!(task.id, "task1");
        assert_eq!(task.required_capability, Some("research".to_string()));
        assert_eq!(task.dependencies, vec!["task0"]);
        assert_eq!(task.priority, 5);
    }

    #[test]
    fn test_plan_creation() {
        let plan = Plan::new("Test plan")
            .add_task(SubTask::new("t1", "First task"))
            .add_task(SubTask::new("t2", "Second task").depends_on("t1"))
            .with_synthesis("summarize");

        assert_eq!(plan.tasks.len(), 2);
        assert_eq!(plan.synthesis_strategy, "summarize");
    }

    #[test]
    fn test_tasks_in_order() {
        let plan = Plan::new("Ordered plan")
            .add_task(SubTask::new("t3", "Third").depends_on("t2"))
            .add_task(SubTask::new("t1", "First"))
            .add_task(SubTask::new("t2", "Second").depends_on("t1"));

        let ordered = plan.tasks_in_order();

        // t1 should come first (no deps), then t2, then t3
        assert_eq!(ordered[0].id, "t1");
        assert_eq!(ordered[1].id, "t2");
        assert_eq!(ordered[2].id, "t3");
    }

    #[tokio::test]
    async fn test_static_planner() {
        let plan = Plan::new("Static plan")
            .add_task(SubTask::new("t1", "Task 1"))
            .add_task(SubTask::new("t2", "Task 2"));

        let planner = StaticPlanner::new(plan);
        let result = planner.plan(&serde_json::json!({}), &[]).await.unwrap();

        assert_eq!(result.tasks.len(), 2);
    }
}
