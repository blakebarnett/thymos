//! Core metric types and structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Task performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPerformanceMetrics {
    /// Success rate (0.0-1.0) - percentage of tasks completed successfully
    pub success_rate: f64,

    /// Error rate (0.0-1.0) - percentage of tasks that failed
    pub error_rate: f64,

    /// Average task completion time
    #[serde(with = "humantime_serde")]
    pub avg_completion_time: Duration,

    /// Median task completion time (less affected by outliers)
    #[serde(with = "humantime_serde")]
    pub median_completion_time: Duration,

    /// P95 task completion time (95th percentile)
    #[serde(with = "humantime_serde")]
    pub p95_completion_time: Duration,

    /// P99 task completion time (99th percentile)
    #[serde(with = "humantime_serde")]
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

impl Default for TaskPerformanceMetrics {
    fn default() -> Self {
        Self {
            success_rate: 1.0,
            error_rate: 0.0,
            avg_completion_time: Duration::ZERO,
            median_completion_time: Duration::ZERO,
            p95_completion_time: Duration::ZERO,
            p99_completion_time: Duration::ZERO,
            total_tasks: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            in_progress_tasks: 0,
            timeout_rate: 0.0,
            cancellation_rate: 0.0,
        }
    }
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

/// Response performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePerformanceMetrics {
    /// Average response latency
    #[serde(with = "humantime_serde")]
    pub avg_latency: Duration,

    /// Median response latency
    #[serde(with = "humantime_serde")]
    pub median_latency: Duration,

    /// P95 response latency
    #[serde(with = "humantime_serde")]
    pub p95_latency: Duration,

    /// P99 response latency
    #[serde(with = "humantime_serde")]
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

impl Default for ResponsePerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_latency: Duration::ZERO,
            median_latency: Duration::ZERO,
            p95_latency: Duration::ZERO,
            p99_latency: Duration::ZERO,
            throughput: 0.0,
            timeout_rate: 0.0,
            current_rps: 0.0,
            peak_rps: 0.0,
            avg_response_size: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
        }
    }
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

impl Default for ResourcePerformanceMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0,
            memory_usage_percent: 0.0,
            peak_memory_usage: 0,
            llm_calls: 0,
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            estimated_cost: 0.0,
            cost_per_request: 0.0,
            avg_tokens_per_request: 0.0,
            llm_error_rate: 0.0,
            rate_limit_hits: 0,
        }
    }
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

/// Quality performance metrics (domain-specific)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityPerformanceMetrics {
    /// User satisfaction score (0.0-1.0, if available)
    pub user_satisfaction: Option<f64>,

    /// User engagement (clicks, interactions, etc.)
    pub user_engagement: Option<f64>,

    /// Output quality score (0.0-1.0, if measurable)
    pub output_quality: Option<f64>,

    /// Business metrics (domain-specific)
    pub business_metrics: HashMap<String, f64>,
}

impl QualityPerformanceMetrics {
    /// Check if quality is acceptable
    pub fn is_quality_acceptable(&self, criteria: &QualityCriteria) -> bool {
        if self
            .user_satisfaction
            .is_some_and(|s| s < criteria.min_user_satisfaction.unwrap_or(0.0))
        {
            return false;
        }

        if self
            .output_quality
            .is_some_and(|q| q < criteria.min_output_quality.unwrap_or(0.0))
        {
            return false;
        }

        // Check business metrics
        for (metric_name, min_value) in &criteria.min_business_metrics {
            if self
                .business_metrics
                .get(metric_name)
                .is_some_and(|&v| v < *min_value)
            {
                return false;
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

/// Memory performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPerformanceMetrics {
    /// Total memories stored
    pub total_memories: usize,

    /// Memory search latency (average)
    #[serde(with = "humantime_serde")]
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

impl Default for MemoryPerformanceMetrics {
    fn default() -> Self {
        Self {
            total_memories: 0,
            avg_search_latency: Duration::ZERO,
            search_success_rate: 1.0,
            consolidation_effectiveness: 1.0,
            concept_extraction_accuracy: None,
            memory_hit_rate: 0.0,
            avg_memories_per_search: 0.0,
            storage_efficiency: 1.0,
        }
    }
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

/// Performance trend indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

    /// Calculate response performance score (0.0-1.0)
    fn response_performance_score(&self) -> f64 {
        if self.response_performance.total_requests == 0 {
            return 1.0;
        }
        let success_rate = self.response_performance.successful_requests as f64
            / self.response_performance.total_requests as f64;
        let latency_score = if self.response_performance.p95_latency.as_secs() < 5 {
            1.0
        } else if self.response_performance.p95_latency.as_secs() < 10 {
            0.8
        } else {
            0.5
        };
        (success_rate + latency_score) / 2.0
    }

    /// Calculate resource performance score (0.0-1.0)
    fn resource_performance_score(&self) -> f64 {
        let cpu_score = 1.0 - self.resource_performance.cpu_usage.min(1.0);
        let memory_score = 1.0 - self.resource_performance.memory_usage_percent.min(1.0);
        let cost_score = if self.resource_performance.cost_per_request < 0.10 {
            1.0
        } else if self.resource_performance.cost_per_request < 0.20 {
            0.8
        } else {
            0.5
        };
        (cpu_score + memory_score + cost_score) / 3.0
    }

    /// Calculate quality performance score (0.0-1.0)
    fn quality_performance_score(&self) -> f64 {
        let mut scores = Vec::new();
        if let Some(satisfaction) = self.quality_performance.user_satisfaction {
            scores.push(satisfaction);
        }
        if let Some(quality) = self.quality_performance.output_quality {
            scores.push(quality);
        }
        if scores.is_empty() {
            return 0.5; // Neutral if no quality metrics available
        }
        scores.iter().sum::<f64>() / scores.len() as f64
    }

    /// Calculate memory performance score (0.0-1.0)
    fn memory_performance_score(&self) -> f64 {
        let search_score = self.memory_performance.search_success_rate;
        let hit_rate_score = self.memory_performance.memory_hit_rate;
        let latency_score = if self.memory_performance.avg_search_latency.as_millis() < 100 {
            1.0
        } else if self.memory_performance.avg_search_latency.as_millis() < 500 {
            0.8
        } else {
            0.5
        };
        (search_score + hit_rate_score + latency_score) / 3.0
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

/// Weights for calculating overall performance score
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

/// Combined performance criteria
#[derive(Debug, Clone)]
pub struct PerformanceCriteria {
    pub task: TaskCriteria,
    pub max_latency: Duration,
    pub resource: ResourceLimits,
    pub quality: QualityCriteria,
    pub memory: MemoryCriteria,
    pub min_overall_score: f64,
}



