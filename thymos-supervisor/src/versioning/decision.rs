//! Auto-decision engine for versioning operations

use crate::{Result, SupervisorError};
use std::sync::Arc;
use std::time::Duration;
use thymos_core::metrics::{AgentMetrics, MetricsCollector, PerformanceTrend};
use thymos_core::llm::LLMProvider;

/// Automatic decision engine for versioning operations
pub struct AutoDecisionEngine {
    /// Decision rules
    rules: Vec<DecisionRule>,
    
    /// Metrics collector
    metrics: Arc<MetricsCollector>,

    /// LLM for complex decisions (optional)
    #[allow(dead_code)]
    llm: Option<Arc<dyn LLMProvider>>,
}

/// Decision rule
#[derive(Debug, Clone)]
pub struct DecisionRule {
    pub name: String,
    pub condition: DecisionCondition,
    pub action: DecisionAction,
    pub priority: usize,
}

/// Decision condition
#[derive(Debug, Clone)]
pub enum DecisionCondition {
    /// Agent performance drops below threshold
    PerformanceDrop { threshold: f64, window: Duration },
    
    /// Performance trend is declining
    PerformanceDeclining,
    
    /// Error rate exceeds threshold
    ErrorRateExceeded { threshold: f64 },
    
    /// Success rate below threshold
    SuccessRateBelow { threshold: f64 },
    
    /// Latency exceeds threshold
    LatencyExceeded { threshold: Duration },
    
    /// Cost per request exceeds threshold
    CostExceeded { threshold: f64 },
    
    /// Resource usage exceeds threshold
    ResourceUsageExceeded { cpu_threshold: f64, memory_threshold: f64 },
    
    /// Experiment timeout
    ExperimentTimeout { max_duration: Duration },
    
    /// Success criteria met
    SuccessCriteriaMet { min_score: f64, min_improvement: f64 },
    
    /// Failure criteria met
    FailureCriteriaMet { max_error_rate: f64, min_success_rate: f64 },
    
    /// Custom condition (evaluated via closure)
    Custom { description: String },
}

/// Decision action
#[derive(Debug, Clone)]
pub enum DecisionAction {
    /// Create branch for experiment
    CreateBranch { branch_name: String, description: String },
    
    /// Create worktree for parallel testing
    CreateWorktree { branch_name: String, count: usize },
    
    /// Merge branch to main
    MergeBranch { source_branch: String },
    
    /// Rollback to previous state
    Rollback { branch_name: String, target_commit: Option<String> },
    
    /// Delete branch
    DeleteBranch { branch_name: String },
    
    /// Scale worktrees
    ScaleWorktrees { branch_name: String, target_count: usize },
}

impl AutoDecisionEngine {
    /// Create a new auto-decision engine
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self {
            rules: Vec::new(),
            metrics,
            llm: None,
        }
    }

    /// Create with LLM support
    pub fn with_llm(metrics: Arc<MetricsCollector>, llm: Arc<dyn LLMProvider>) -> Self {
        Self {
            rules: Vec::new(),
            metrics,
            llm: Some(llm),
        }
    }

    /// Add a decision rule
    pub fn add_rule(&mut self, rule: DecisionRule) {
        self.rules.push(rule);
        // Sort by priority (higher priority first)
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Evaluate all rules and return actions to take
    pub async fn evaluate(&self, agent_id: &str) -> Result<Vec<DecisionAction>> {
        let metrics = self.metrics.collect_metrics_by_id(agent_id).await
            .map_err(|e| SupervisorError::Supervisor(format!("Failed to collect metrics: {}", e)))?;

        let mut actions = Vec::new();

        for rule in &self.rules {
            if self.evaluate_condition(&rule.condition, &metrics).await? {
                actions.push(rule.action.clone());
            }
        }

        Ok(actions)
    }

    /// Check if agent should experiment (create branch)
    pub async fn should_experiment(&self, agent_id: &str) -> Result<bool> {
        let metrics = self.metrics.collect_metrics_by_id(agent_id).await
            .map_err(|e| SupervisorError::Supervisor(format!("Failed to collect metrics: {}", e)))?;

        // Experiment if performance is declining
        if metrics.trend == PerformanceTrend::Declining {
            return Ok(true);
        }

        // Experiment if performance is stable but not improving
        if metrics.trend == PerformanceTrend::Stable && metrics.variance < 0.05 {
            return Ok(true);
        }

        // Experiment if specific metrics are below threshold
        if metrics.task_performance.success_rate < 0.85 {
            return Ok(true);
        }

        if metrics.response_performance.p95_latency > Duration::from_secs(5) {
            return Ok(true);
        }

        if metrics.resource_performance.cost_per_request > 0.10 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if experiment should be rolled back
    pub async fn should_rollback(&self, agent_id: &str, _branch_name: &str) -> Result<bool> {
        let metrics = self.metrics.collect_metrics_by_id(agent_id).await
            .map_err(|e| SupervisorError::Supervisor(format!("Failed to collect metrics: {}", e)))?;

        // Rollback if error rate is too high
        if metrics.task_performance.error_rate > 0.1 {
            return Ok(true);
        }

        // Rollback if success rate is too low
        if metrics.task_performance.success_rate < 0.5 {
            return Ok(true);
        }

        // Rollback if latency is unacceptable
        if metrics.response_performance.p95_latency > Duration::from_secs(10) {
            return Ok(true);
        }

        // Rollback if cost is too high
        if metrics.resource_performance.cost_per_request > 0.20 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if experiment should be merged
    pub async fn should_merge(&self, agent_id: &str, _branch_name: &str) -> Result<bool> {
        let metrics = self.metrics.collect_metrics_by_id(agent_id).await
            .map_err(|e| SupervisorError::Supervisor(format!("Failed to collect metrics: {}", e)))?;

        // Get baseline metrics for comparison
        let history = self.metrics.get_history(agent_id).await
            .map_err(|e| SupervisorError::Supervisor(format!("Failed to get history: {}", e)))?;
        
        if history.len() < 2 {
            return Ok(false); // Need history to compare
        }

        let baseline = &history[history.len() - 2];

        // Merge if performance is significantly better
        if metrics.overall_score > baseline.overall_score * 1.1 {
            return Ok(true);
        }

        // Merge if specific metrics improved
        if metrics.task_performance.success_rate > baseline.task_performance.success_rate * 1.05 {
            return Ok(true);
        }

        let baseline_latency_ms = baseline.response_performance.avg_latency.as_millis() as f64;
        let current_latency_ms = metrics.response_performance.avg_latency.as_millis() as f64;
        if current_latency_ms < baseline_latency_ms * 0.9 {
            return Ok(true);
        }

        if metrics.resource_performance.cost_per_request < baseline.resource_performance.cost_per_request * 0.9 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Evaluate a condition against metrics
    async fn evaluate_condition(
        &self,
        condition: &DecisionCondition,
        metrics: &AgentMetrics,
    ) -> Result<bool> {
        match condition {
            DecisionCondition::PerformanceDrop { threshold, .. } => {
                Ok(metrics.overall_score < *threshold)
            }
            DecisionCondition::PerformanceDeclining => {
                Ok(metrics.trend == PerformanceTrend::Declining)
            }
            DecisionCondition::ErrorRateExceeded { threshold } => {
                Ok(metrics.task_performance.error_rate > *threshold)
            }
            DecisionCondition::SuccessRateBelow { threshold } => {
                Ok(metrics.task_performance.success_rate < *threshold)
            }
            DecisionCondition::LatencyExceeded { threshold } => {
                Ok(metrics.response_performance.p95_latency > *threshold)
            }
            DecisionCondition::CostExceeded { threshold } => {
                Ok(metrics.resource_performance.cost_per_request > *threshold)
            }
            DecisionCondition::ResourceUsageExceeded { cpu_threshold, memory_threshold } => {
                Ok(metrics.resource_performance.cpu_usage > *cpu_threshold
                    || metrics.resource_performance.memory_usage_percent > *memory_threshold)
            }
            DecisionCondition::SuccessCriteriaMet { min_score, .. } => {
                // Would need baseline comparison - simplified for now
                Ok(metrics.overall_score >= *min_score)
            }
            DecisionCondition::FailureCriteriaMet { max_error_rate, min_success_rate } => {
                Ok(metrics.task_performance.error_rate > *max_error_rate
                    || metrics.task_performance.success_rate < *min_success_rate)
            }
            DecisionCondition::ExperimentTimeout { .. } => {
                // Would need to track experiment start time
                Ok(false) // Placeholder
            }
            DecisionCondition::Custom { .. } => {
                // Custom conditions would need custom evaluators
                Ok(false) // Placeholder
            }
        }
    }
}

impl Default for AutoDecisionEngine {
    fn default() -> Self {
        // This would require a MetricsCollector, so we can't provide a real default
        // Users should use new() or with_llm()
        panic!("AutoDecisionEngine::default() not supported. Use AutoDecisionEngine::new() instead.")
    }
}

