//! Workflow Patterns for LLM Agent Execution
//!
//! This module provides structured execution patterns for LLM agents:
//!
//! - **Chain**: Sequential steps where each output feeds the next input
//! - **Router**: Directs input to specialized handlers based on classification
//! - **Parallel**: Executes multiple tasks concurrently
//! - **Orchestrator**: Coordinates specialized workers for complex tasks
//! - **Evaluator-Optimizer**: Iteratively refines output based on evaluation
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::workflow::{Chain, Step, Router, RuleClassifier};
//!
//! // Chain example
//! let chain = Chain::builder()
//!     .step(Step::llm("summarize", summarize_prompt))
//!     .step(Step::llm("translate", translate_prompt))
//!     .build();
//!
//! let result = chain.execute(input, &provider).await?;
//!
//! // Router example
//! let router = Router::builder()
//!     .step_route("greeting", Step::llm("greet", "{{input}}").build())
//!     .step_route("question", Step::llm("answer", "{{input}}").build())
//!     .fallback_step(Step::llm("default", "{{input}}").build())
//!     .build();
//!
//! let classifier = RuleClassifier::new("other")
//!     .add_contains_rule("greeting", "hello");
//!
//! let (output, trace) = router.execute(input, &classifier, &provider).await?;
//! ```
//!
//! # References
//!
//! - [Anthropic Building Effective Agents](https://www.anthropic.com/research/building-effective-agents)
//! - Phase 2 of `docs/design/LLM_NATIVE_AGENT_DESIGN.md`

mod aggregator;
mod chain;
mod classifier;
mod evaluator_optimizer;
mod execution;
mod gate;
mod orchestrator;
mod parallel;
mod planner;
mod router;
mod step;

pub use aggregator::{Aggregator, AllSuccess, BestResult, FirstSuccess, Merge, Voting};
pub use chain::{Chain, ChainBuilder, ChainConfig};
pub use classifier::{Classification, Classifier, KeywordClassifier, LLMClassifier, RuleClassifier};
pub use evaluator_optimizer::{
    Attempt, Evaluation, Evaluator, EvaluatorOptimizer, EvaluatorOptimizerBuilder,
    EvaluatorOptimizerConfig, EvaluatorOptimizerTrace, Generator, LLMEvaluator, LLMGenerator,
    ThresholdEvaluator,
};
pub use execution::{ExecutionTrace, StepTrace, TokenUsageTrace, WorkflowError, WorkflowResult};
pub use gate::{Gate, GateCondition};
pub use orchestrator::{Orchestrator, OrchestratorBuilder, OrchestratorTrace, TaskResult, Worker, WorkerHandler};
pub use parallel::{Branch, BranchTrace, Parallel, ParallelBuilder, ParallelConfig, ParallelExecutionTrace};
pub use planner::{LLMPlanner, Plan, Planner, StaticPlanner, SubTask};
pub use router::{Route, RouteHandler, Router, RouterBuilder, RouterExecutionTrace};
pub use step::{Step, StepBuilder, StepOutput, StepType};
