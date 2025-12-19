# Agent Performance Metrics

**Date**: December 2024  
**Status**: Design  
**Purpose**: Define what "performance" means for agent monitoring and automatic decision-making

## Executive Summary

When the supervisor "monitors performance," it tracks multiple dimensions of agent behavior:

1. **Task Performance**: Success rate, error rate, completion time
2. **Response Performance**: Latency, throughput, timeout rate
3. **Resource Performance**: CPU, memory, LLM costs, token usage
4. **Quality Performance**: Output quality, user satisfaction, business metrics
5. **Memory Performance**: Memory efficiency, search performance, consolidation effectiveness

The supervisor uses these metrics to make automatic decisions about branching, merging, rollback, and scaling.

---

## Core Performance Metrics

### 1. Task Performance Metrics

```rust
/// Task performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPerformanceMetrics {
    /// Success rate (0.0-1.0) - percentage of tasks completed successfully
    pub success_rate: f64,
    
    /// Error rate (0.0-1.0) - percentage of tasks that failed
    pub error_rate: f64,
    
    /// Average task completion time
    pub avg_completion_time: Duration,
    
    /// Median task completion time (less affected by outliers)
    pub median_completion_time: Duration,
    
    /// P95 task completion time (95th percentile)
    pub p95_completion_time: Duration,
    
    /// P99 task completion time (99th percentile)
    pub p99_completion_time: Duration,
    
    /// Total tasks completed
    pub total_tasks: u64,
    
    /// Successful tasks
    pub successful_tasks: u64,
    
    /// Failed tasks
    pub failed_tasks: u64,
    
    /// Tasks in progress
    pub in_progress_tasks: u64,
    
    /// Task timeout rate (0.0-1.0)
    pub timeout_rate: f64,
    
    /// Task cancellation rate (0.0-1.0)
    pub cancellation_rate: f64,
}

impl TaskPerformanceMetrics {
    /// Calculate success rate from counts
    pub fn calculate_success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.successful_tasks as f64 / self.total_tasks as f64
    }
    
    /// Check if performance is acceptable
    pub fn is_acceptable(&self, criteria: &TaskCriteria) -> bool {
        self.success_rate >= criteria.min_success_rate
            && self.error_rate <= criteria.max_error_rate
            && self.avg_completion_time <= criteria.max_completion_time
            && self.timeout_rate <= criteria.max_timeout_rate
    }
}

#[derive(Debug, Clone)]
pub struct TaskCriteria {
    pub min_success_rate: f64,
    pub max_error_rate: f64,
    pub max_completion_time: Duration,
    pub max_timeout_rate: f64,
}
```

**What it measures**: How well the agent completes its tasks.

**Use cases**:
- Success rate < 0.9 → Create experiment branch
- Error rate > 0.1 → Rollback
- Completion time increasing → Investigate

### 2. Response Performance Metrics

```rust
/// Response performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePerformanceMetrics {
    /// Average response latency
    pub avg_latency: Duration,
    
    /// Median response latency
    pub median_latency: Duration,
    
    /// P95 response latency
    pub p95_latency: Duration,
    
    /// P99 response latency
    pub p99_latency: Duration,
    
    /// Throughput (requests per second)
    pub throughput: f64,
    
    /// Timeout rate (0.0-1.0)
    pub timeout_rate: f64,
    
    /// Requests per second (current)
    pub current_rps: f64,
    
    /// Peak requests per second
    pub peak_rps: f64,
    
    /// Average response size (bytes)
    pub avg_response_size: usize,
    
    /// Total requests handled
    pub total_requests: u64,
    
    /// Successful requests
    pub successful_requests: u64,
    
    /// Failed requests
    pub failed_requests: u64,
}

impl ResponsePerformanceMetrics {
    /// Check if latency is acceptable
    pub fn is_latency_acceptable(&self, max_latency: Duration) -> bool {
        self.p95_latency <= max_latency
    }
    
    /// Check if throughput is sufficient
    pub fn is_throughput_sufficient(&self, min_throughput: f64) -> bool {
        self.throughput >= min_throughput
    }
}
```

**What it measures**: How fast and efficiently the agent responds.

**Use cases**:
- Latency > 5s → Create experiment branch for optimization
- Throughput dropping → Scale with worktrees
- Timeout rate > 0.05 → Rollback

### 3. Resource Performance Metrics

```rust
/// Resource performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePerformanceMetrics {
    /// CPU usage (0.0-1.0)
    pub cpu_usage: f64,
    
    /// Memory usage (bytes)
    pub memory_usage: usize,
    
    /// Memory usage percentage (0.0-1.0)
    pub memory_usage_percent: f64,
    
    /// Peak memory usage (bytes)
    pub peak_memory_usage: usize,
    
    /// LLM API calls made
    pub llm_calls: u64,
    
    /// Total tokens used (input + output)
    pub total_tokens: u64,
    
    /// Input tokens
    pub input_tokens: u64,
    
    /// Output tokens
    pub output_tokens: u64,
    
    /// Estimated cost (USD)
    pub estimated_cost: f64,
    
    /// Cost per request (USD)
    pub cost_per_request: f64,
    
    /// Average tokens per request
    pub avg_tokens_per_request: f64,
    
    /// LLM API error rate (0.0-1.0)
    pub llm_error_rate: f64,
    
    /// Rate limit hits
    pub rate_limit_hits: u64,
}

impl ResourcePerformanceMetrics {
    /// Check if resource usage is acceptable
    pub fn is_resource_usage_acceptable(&self, limits: &ResourceLimits) -> bool {
        self.cpu_usage <= limits.max_cpu_usage
            && self.memory_usage_percent <= limits.max_memory_usage
            && self.estimated_cost <= limits.max_cost_per_hour
    }
    
    /// Calculate cost efficiency (tasks per dollar)
    pub fn cost_efficiency(&self, tasks_completed: u64) -> f64 {
        if self.estimated_cost == 0.0 {
            return f64::INFINITY;
        }
        tasks_completed as f64 / self.estimated_cost
    }
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_cpu_usage: f64,
    pub max_memory_usage: f64,
    pub max_cost_per_hour: f64,
}
```

**What it measures**: How efficiently the agent uses resources (CPU, memory, money).

**Use cases**:
- Cost per request increasing → Create experiment branch for optimization
- Memory usage > 80% → Scale down or optimize
- Rate limit hits → Implement backoff or use worktrees

### 4. Quality Performance Metrics

```rust
/// Quality performance metrics (domain-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPerformanceMetrics {
    /// User satisfaction score (0.0-1.0, if available)
    pub user_satisfaction: Option<f64>,
    
    /// User engagement (clicks, interactions, etc.)
    pub user_engagement: Option<f64>,
    
    /// Output quality score (0.0-1.0, if measurable)
    pub output_quality: Option<f64>,
    
    /// Business metrics (domain-specific)
    pub business_metrics: HashMap<String, f64>,
    
    /// Examples:
    /// - conversion_rate: 0.15
    /// - revenue_per_user: 25.50
    /// - customer_retention: 0.85
    /// - support_ticket_resolution_rate: 0.92
}

impl QualityPerformanceMetrics {
    /// Check if quality is acceptable
    pub fn is_quality_acceptable(&self, criteria: &QualityCriteria) -> bool {
        if let Some(satisfaction) = self.user_satisfaction {
            if satisfaction < criteria.min_user_satisfaction {
                return false;
            }
        }
        
        if let Some(quality) = self.output_quality {
            if quality < criteria.min_output_quality {
                return false;
            }
        }
        
        // Check business metrics
        for (metric_name, min_value) in &criteria.min_business_metrics {
            if let Some(&value) = self.business_metrics.get(metric_name) {
                if value < *min_value {
                    return false;
                }
            }
        }
        
        true
    }
}

#[derive(Debug, Clone)]
pub struct QualityCriteria {
    pub min_user_satisfaction: Option<f64>,
    pub min_output_quality: Option<f64>,
    pub min_business_metrics: HashMap<String, f64>,
}
```

**What it measures**: How well the agent meets user needs and business goals.

**Use cases**:
- User satisfaction dropping → Create experiment branch
- Conversion rate decreasing → Rollback
- Support resolution rate improving → Merge to main

### 5. Memory Performance Metrics

```rust
/// Memory performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPerformanceMetrics {
    /// Total memories stored
    pub total_memories: usize,
    
    /// Memory search latency (average)
    pub avg_search_latency: Duration,
    
    /// Memory search success rate (0.0-1.0)
    pub search_success_rate: f64,
    
    /// Memory consolidation effectiveness (0.0-1.0)
    pub consolidation_effectiveness: f64,
    
    /// Concept extraction accuracy (0.0-1.0, if measurable)
    pub concept_extraction_accuracy: Option<f64>,
    
    /// Memory hit rate (0.0-1.0) - percentage of queries that find relevant memories
    pub memory_hit_rate: f64,
    
    /// Average memories per search result
    pub avg_memories_per_search: f64,
    
    /// Memory storage efficiency (compression ratio)
    pub storage_efficiency: f64,
}

impl MemoryPerformanceMetrics {
    /// Check if memory performance is acceptable
    pub fn is_acceptable(&self, criteria: &MemoryCriteria) -> bool {
        self.avg_search_latency <= criteria.max_search_latency
            && self.search_success_rate >= criteria.min_search_success_rate
            && self.memory_hit_rate >= criteria.min_memory_hit_rate
    }
}

#[derive(Debug, Clone)]
pub struct MemoryCriteria {
    pub max_search_latency: Duration,
    pub min_search_success_rate: f64,
    pub min_memory_hit_rate: f64,
}
```

**What it measures**: How efficiently the agent uses its memory system.

**Use cases**:
- Search latency increasing → Optimize memory indexing
- Memory hit rate dropping → Improve memory consolidation
- Concept extraction accuracy decreasing → Rollback

---

## Combined Agent Metrics

```rust
/// Complete agent performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    /// Agent ID
    pub agent_id: String,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Task performance
    pub task_performance: TaskPerformanceMetrics,
    
    /// Response performance
    pub response_performance: ResponsePerformanceMetrics,
    
    /// Resource performance
    pub resource_performance: ResourcePerformanceMetrics,
    
    /// Quality performance
    pub quality_performance: QualityPerformanceMetrics,
    
    /// Memory performance
    pub memory_performance: MemoryPerformanceMetrics,
    
    /// Overall performance score (0.0-1.0)
    pub overall_score: f64,
    
    /// Performance trend
    pub trend: PerformanceTrend,
    
    /// Performance variance (stability indicator)
    pub variance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Declining,
    Unknown,
}

impl AgentMetrics {
    /// Calculate overall performance score
    pub fn calculate_overall_score(&self, weights: &PerformanceWeights) -> f64 {
        let task_score = self.task_performance.success_rate * weights.task_weight;
        let response_score = self.response_performance_score() * weights.response_weight;
        let resource_score = self.resource_performance_score() * weights.resource_weight;
        let quality_score = self.quality_performance_score() * weights.quality_weight;
        let memory_score = self.memory_performance_score() * weights.memory_weight;
        
        task_score + response_score + resource_score + quality_score + memory_score
    }
    
    /// Determine performance trend
    pub fn determine_trend(&self, previous: &AgentMetrics) -> PerformanceTrend {
        let current_score = self.overall_score;
        let previous_score = previous.overall_score;
        
        let diff = current_score - previous_score;
        
        if diff > 0.05 {
            PerformanceTrend::Improving
        } else if diff < -0.05 {
            PerformanceTrend::Declining
        } else {
            PerformanceTrend::Stable
        }
    }
    
    /// Check if performance is acceptable
    pub fn is_acceptable(&self, criteria: &PerformanceCriteria) -> bool {
        self.task_performance.is_acceptable(&criteria.task)
            && self.response_performance.is_latency_acceptable(criteria.max_latency)
            && self.resource_performance.is_resource_usage_acceptable(&criteria.resource)
            && self.quality_performance.is_quality_acceptable(&criteria.quality)
            && self.memory_performance.is_acceptable(&criteria.memory)
            && self.overall_score >= criteria.min_overall_score
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceWeights {
    pub task_weight: f64,
    pub response_weight: f64,
    pub resource_weight: f64,
    pub quality_weight: f64,
    pub memory_weight: f64,
}

impl Default for PerformanceWeights {
    fn default() -> Self {
        Self {
            task_weight: 0.3,
            response_weight: 0.2,
            resource_weight: 0.2,
            quality_weight: 0.2,
            memory_weight: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceCriteria {
    pub task: TaskCriteria,
    pub max_latency: Duration,
    pub resource: ResourceLimits,
    pub quality: QualityCriteria,
    pub memory: MemoryCriteria,
    pub min_overall_score: f64,
}
```

---

## Metrics Collection

### Metrics Collector

```rust
/// Metrics collector for agents
pub struct MetricsCollector {
    /// Storage for metrics
    storage: Arc<dyn MetricsStorage>,
    
    /// Collection interval
    collection_interval: Duration,
    
    /// Metrics history (for trend analysis)
    history: Arc<RwLock<VecDeque<AgentMetrics>>>,
}

impl MetricsCollector {
    /// Collect metrics for an agent
    pub async fn collect_metrics(
        &self,
        agent_id: &str,
    ) -> Result<AgentMetrics> {
        // Collect task metrics
        let task_metrics = self.collect_task_metrics(agent_id).await?;
        
        // Collect response metrics
        let response_metrics = self.collect_response_metrics(agent_id).await?;
        
        // Collect resource metrics
        let resource_metrics = self.collect_resource_metrics(agent_id).await?;
        
        // Collect quality metrics
        let quality_metrics = self.collect_quality_metrics(agent_id).await?;
        
        // Collect memory metrics
        let memory_metrics = self.collect_memory_metrics(agent_id).await?;
        
        // Calculate overall score
        let overall_score = self.calculate_overall_score(
            &task_metrics,
            &response_metrics,
            &resource_metrics,
            &quality_metrics,
            &memory_metrics,
        );
        
        // Determine trend
        let trend = self.determine_trend(agent_id, &overall_score).await?;
        
        // Calculate variance
        let variance = self.calculate_variance(agent_id).await?;
        
        let metrics = AgentMetrics {
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            task_performance: task_metrics,
            response_performance: response_metrics,
            resource_performance: resource_metrics,
            quality_performance: quality_metrics,
            memory_performance: memory_metrics,
            overall_score,
            trend,
            variance,
        };
        
        // Store metrics
        self.storage.store_metrics(&metrics).await?;
        
        // Update history
        {
            let mut history = self.history.write().await;
            history.push_back(metrics.clone());
            if history.len() > 1000 {
                history.pop_front();
            }
        }
        
        Ok(metrics)
    }
    
    /// Collect task metrics from agent
    async fn collect_task_metrics(&self, agent_id: &str) -> Result<TaskPerformanceMetrics> {
        // Query agent for task statistics
        // This would be implemented via agent API or monitoring hooks
        todo!()
    }
    
    /// Collect response metrics from agent
    async fn collect_response_metrics(&self, agent_id: &str) -> Result<ResponsePerformanceMetrics> {
        // Query agent for response statistics
        todo!()
    }
    
    /// Collect resource metrics from system
    async fn collect_resource_metrics(&self, agent_id: &str) -> Result<ResourcePerformanceMetrics> {
        // Query system for CPU/memory usage
        // Query LLM provider for token/cost statistics
        todo!()
    }
    
    /// Collect quality metrics (domain-specific)
    async fn collect_quality_metrics(&self, agent_id: &str) -> Result<QualityPerformanceMetrics> {
        // Query domain-specific quality sources
        // User satisfaction, business metrics, etc.
        todo!()
    }
    
    /// Collect memory metrics from Locai
    async fn collect_memory_metrics(&self, agent_id: &str) -> Result<MemoryPerformanceMetrics> {
        // Query Locai for memory statistics
        todo!()
    }
}
```

---

## Supervisor Decision-Making

### How Supervisor Uses Metrics

```rust
impl VersioningSupervisor {
    /// Check if agent should experiment based on metrics
    pub async fn should_experiment(
        &self,
        agent_id: &str,
    ) -> Result<bool> {
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        
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
    pub async fn should_rollback(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<bool> {
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        
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
    pub async fn should_merge(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<bool> {
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        let baseline = self.metrics.get_baseline_metrics(agent_id).await?;
        
        // Merge if performance is significantly better
        if metrics.overall_score > baseline.overall_score * 1.1 {
            return Ok(true);
        }
        
        // Merge if specific metrics improved
        if metrics.task_performance.success_rate > baseline.task_performance.success_rate * 1.05 {
            return Ok(true);
        }
        
        if metrics.response_performance.avg_latency < baseline.response_performance.avg_latency * 0.9 {
            return Ok(true);
        }
        
        if metrics.resource_performance.cost_per_request < baseline.resource_performance.cost_per_request * 0.9 {
            return Ok(true);
        }
        
        Ok(false)
    }
}
```

---

## Example: Customer Support Agent

For a customer support agent, "performance" might mean:

```rust
let criteria = PerformanceCriteria {
    task: TaskCriteria {
        min_success_rate: 0.90,  // 90% of tickets resolved successfully
        max_error_rate: 0.05,    // Less than 5% errors
        max_completion_time: Duration::from_secs(300),  // 5 minutes
        max_timeout_rate: 0.01,  // Less than 1% timeouts
    },
    max_latency: Duration::from_secs(2),  // Respond within 2 seconds
    resource: ResourceLimits {
        max_cpu_usage: 0.8,
        max_memory_usage: 0.8,
        max_cost_per_hour: 10.0,  // $10/hour max
    },
    quality: QualityCriteria {
        min_user_satisfaction: Some(0.85),  // 85% user satisfaction
        min_output_quality: Some(0.80),     // 80% output quality
        min_business_metrics: HashMap::from([
            ("ticket_resolution_rate".to_string(), 0.90),  // 90% resolution
            ("first_contact_resolution".to_string(), 0.70),  // 70% FCR
        ]),
    },
    memory: MemoryCriteria {
        max_search_latency: Duration::from_millis(100),
        min_search_success_rate: 0.95,
        min_memory_hit_rate: 0.80,
    },
    min_overall_score: 0.80,
};
```

**Performance means**:
- Successfully resolves 90% of support tickets
- Responds within 2 seconds
- Costs less than $10/hour
- Users are 85% satisfied
- Resolves 90% of tickets
- Memory searches are fast and accurate

---

## Summary

**"Performance" means**:

1. **Task Performance**: Can the agent complete its tasks successfully?
2. **Response Performance**: Is the agent fast and responsive?
3. **Resource Performance**: Is the agent using resources efficiently?
4. **Quality Performance**: Does the agent meet user needs and business goals?
5. **Memory Performance**: Is the agent using memory effectively?

The supervisor monitors all these dimensions and makes automatic decisions based on:
- **Trends**: Is performance improving, stable, or declining?
- **Thresholds**: Are metrics above/below acceptable levels?
- **Comparisons**: Is this better/worse than baseline or other experiments?

This enables intelligent, data-driven automatic decision-making.



