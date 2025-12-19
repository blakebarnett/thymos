//! Metrics collector implementation

use crate::agent::Agent;
use crate::error::Result;
use crate::metrics::{
    AgentMetrics, MemoryPerformanceMetrics, PerformanceTrend, PerformanceWeights,
    QualityPerformanceMetrics, ResourcePerformanceMetrics, ResponsePerformanceMetrics,
    TaskPerformanceMetrics,
};
use crate::metrics::cost::LLMCostCalculator;
use crate::metrics::resource::ResourceMonitor;
use crate::metrics::storage::MetricsStorage;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Internal tracking state for metrics collection
#[derive(Debug, Clone)]
struct MetricsState {
    /// Task tracking
    tasks: Vec<TaskRecord>,
    /// Response tracking
    responses: Vec<ResponseRecord>,
    /// LLM call tracking
    llm_calls: Vec<LLMCallRecord>,
    /// Memory operation tracking
    memory_operations: Vec<MemoryOperationRecord>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TaskRecord {
    start_time: chrono::DateTime<Utc>,
    end_time: Option<chrono::DateTime<Utc>>,
    success: Option<bool>,
    timeout: bool,
    cancelled: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ResponseRecord {
    start_time: chrono::DateTime<Utc>,
    latency: Duration,
    success: bool,
    size: usize,
    timeout: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LLMCallRecord {
    timestamp: chrono::DateTime<Utc>,
    input_tokens: usize,
    output_tokens: usize,
    cost: f64,
    error: bool,
    rate_limited: bool,
    model_info: crate::llm::ModelInfo,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MemoryOperationRecord {
    timestamp: chrono::DateTime<Utc>,
    operation_type: String,
    latency: Duration,
    success: bool,
    memories_found: Option<usize>,
}

/// Metrics collector for agents
pub struct MetricsCollector {
    /// Storage backend
    storage: Arc<dyn MetricsStorage>,
    /// Resource monitor
    resource_monitor: Arc<dyn ResourceMonitor>,
    /// Cost calculator
    cost_calculator: Arc<RwLock<LLMCostCalculator>>,
    /// Internal state per agent
    state: Arc<RwLock<HashMap<String, MetricsState>>>,
    /// Performance weights
    weights: PerformanceWeights,
    /// History for trend analysis
    history: Arc<RwLock<HashMap<String, Vec<AgentMetrics>>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(
        storage: Arc<dyn MetricsStorage>,
        resource_monitor: Arc<dyn ResourceMonitor>,
    ) -> Self {
        Self {
            storage,
            resource_monitor,
            cost_calculator: Arc::new(RwLock::new(LLMCostCalculator::new())),
            state: Arc::new(RwLock::new(HashMap::new())),
            weights: PerformanceWeights::default(),
            history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get latest metrics for an agent from storage
    pub async fn get_agent_metrics(&self, agent_id: &str) -> Result<Option<AgentMetrics>> {
        self.storage.get_latest_metrics(agent_id).await
    }

    /// Collect metrics for an agent
    pub async fn collect_metrics(&self, agent: &Agent) -> Result<AgentMetrics> {
        let agent_id = agent.id().to_string();

        // Collect task metrics
        let task_metrics = self.collect_task_metrics(&agent_id).await?;

        // Collect response metrics
        let response_metrics = self.collect_response_metrics(&agent_id).await?;

        // Collect resource metrics
        let resource_metrics = self.collect_resource_metrics(&agent_id).await?;

        // Collect quality metrics
        let quality_metrics = self.collect_quality_metrics(&agent_id).await?;

        // Collect memory metrics
        let memory_metrics = self.collect_memory_metrics_by_id(&agent_id).await?;

        // Calculate overall score
        let overall_score = self.calculate_overall_score(
            &task_metrics,
            &response_metrics,
            &resource_metrics,
            &quality_metrics,
            &memory_metrics,
        );

        // Determine trend
        let trend = self.determine_trend(&agent_id, overall_score).await?;

        // Calculate variance
        let variance = self.calculate_variance(&agent_id).await?;

        let metrics = AgentMetrics {
            agent_id: agent_id.clone(),
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
            let agent_history = history.entry(agent_id).or_insert_with(Vec::new);
            agent_history.push(metrics.clone());
            if agent_history.len() > 1000 {
                agent_history.remove(0);
            }
        }

        Ok(metrics)
    }

    /// Collect metrics by agent ID (uses stored metrics if available, otherwise creates default)
    ///
    /// This method is useful when you don't have an Agent instance but need metrics
    /// for decision-making. It will use the latest stored metrics and combine with
    /// any current state tracking.
    pub async fn collect_metrics_by_id(&self, agent_id: &str) -> Result<AgentMetrics> {
        // Try to get latest stored metrics
        if let Some(stored_metrics) = self.storage.get_latest_metrics(agent_id).await? {
            // Update with current state if available
            let state = self.state.read().await;
            if state.get(agent_id).is_some() {
                // We have current state - collect fresh metrics
                drop(state);
                
                // Collect task metrics from state
                let task_metrics = self.collect_task_metrics(agent_id).await?;
                let response_metrics = self.collect_response_metrics(agent_id).await?;
                let resource_metrics = self.collect_resource_metrics(agent_id).await?;
                let quality_metrics = self.collect_quality_metrics(agent_id).await?;
                
                // For memory metrics, we'd need the agent, so use defaults
                let memory_metrics = MemoryPerformanceMetrics::default();
                
                // Calculate overall score
                let overall_score = self.calculate_overall_score(
                    &task_metrics,
                    &response_metrics,
                    &resource_metrics,
                    &quality_metrics,
                    &memory_metrics,
                );
                
                // Determine trend
                let trend = self.determine_trend(agent_id, overall_score).await?;
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
                
                // Store the new metrics
                self.storage.store_metrics(&metrics).await?;
                
                return Ok(metrics);
            }
            
            // No current state, return stored metrics
            return Ok(stored_metrics);
        }
        
        // No stored metrics and no state - return default metrics
        Ok(AgentMetrics {
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            task_performance: TaskPerformanceMetrics::default(),
            response_performance: ResponsePerformanceMetrics::default(),
            resource_performance: ResourcePerformanceMetrics::default(),
            quality_performance: QualityPerformanceMetrics::default(),
            memory_performance: MemoryPerformanceMetrics::default(),
            overall_score: 1.0,
            trend: PerformanceTrend::Unknown,
            variance: 0.0,
        })
    }

    /// Record a task start
    pub async fn record_task_start(&self, agent_id: &str) -> Result<()> {
        let mut state = self.state.write().await;
        let agent_state = state.entry(agent_id.to_string()).or_insert_with(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        agent_state.tasks.push(TaskRecord {
            start_time: Utc::now(),
            end_time: None,
            success: None,
            timeout: false,
            cancelled: false,
        });

        Ok(())
    }

    /// Record a task completion
    pub async fn record_task_complete(
        &self,
        agent_id: &str,
        success: bool,
        timeout: bool,
        cancelled: bool,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        if let Some(task) = state
            .get_mut(agent_id)
            .and_then(|s| s.tasks.last_mut())
        {
            task.end_time = Some(Utc::now());
            task.success = Some(success);
            task.timeout = timeout;
            task.cancelled = cancelled;
        }
        Ok(())
    }

    /// Record a response
    pub async fn record_response(
        &self,
        agent_id: &str,
        latency: Duration,
        success: bool,
        size: usize,
        timeout: bool,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        let agent_state = state.entry(agent_id.to_string()).or_insert_with(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        agent_state.responses.push(ResponseRecord {
            start_time: Utc::now(),
            latency,
            success,
            size,
            timeout,
        });

        // Keep only last 1000 responses
        if agent_state.responses.len() > 1000 {
            agent_state.responses.remove(0);
        }

        Ok(())
    }

    /// Record an LLM call
    pub async fn record_llm_call(
        &self,
        agent_id: &str,
        input_tokens: usize,
        output_tokens: usize,
        model_info: crate::llm::ModelInfo,
        error: bool,
        rate_limited: bool,
    ) -> Result<()> {
        let cost_calculator = self.cost_calculator.read().await;
        let usage = crate::llm::TokenUsage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
        };
        let cost = cost_calculator.calculate_cost(&model_info, &usage);
        drop(cost_calculator);

        let mut state = self.state.write().await;
        let agent_state = state.entry(agent_id.to_string()).or_insert_with(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        agent_state.llm_calls.push(LLMCallRecord {
            timestamp: Utc::now(),
            input_tokens,
            output_tokens,
            cost,
            error,
            rate_limited,
            model_info,
        });

        // Keep only last 1000 LLM calls
        if agent_state.llm_calls.len() > 1000 {
            agent_state.llm_calls.remove(0);
        }

        Ok(())
    }

    /// Record a memory operation
    pub async fn record_memory_operation(
        &self,
        agent_id: &str,
        operation_type: String,
        latency: Duration,
        success: bool,
        memories_found: Option<usize>,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        let agent_state = state.entry(agent_id.to_string()).or_insert_with(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        agent_state.memory_operations.push(MemoryOperationRecord {
            timestamp: Utc::now(),
            operation_type,
            latency,
            success,
            memories_found,
        });

        // Keep only last 1000 memory operations
        if agent_state.memory_operations.len() > 1000 {
            agent_state.memory_operations.remove(0);
        }

        Ok(())
    }

    /// Collect task metrics
    async fn collect_task_metrics(&self, agent_id: &str) -> Result<TaskPerformanceMetrics> {
        let state = self.state.read().await;
        let agent_state = state.get(agent_id).cloned().unwrap_or_else(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        let completed_tasks: Vec<_> = agent_state
            .tasks
            .iter()
            .filter(|t| t.end_time.is_some())
            .collect();

        let total_tasks = completed_tasks.len() as u64;
        let successful_tasks = completed_tasks
            .iter()
            .filter(|t| t.success == Some(true))
            .count() as u64;
        let failed_tasks = completed_tasks
            .iter()
            .filter(|t| t.success == Some(false))
            .count() as u64;
        let in_progress_tasks = agent_state
            .tasks
            .iter()
            .filter(|t| t.end_time.is_none())
            .count() as u64;

        let timeout_count = completed_tasks.iter().filter(|t| t.timeout).count() as u64;
        let cancellation_count = completed_tasks.iter().filter(|t| t.cancelled).count() as u64;

        let success_rate = if total_tasks > 0 {
            successful_tasks as f64 / total_tasks as f64
        } else {
            1.0
        };
        let error_rate = if total_tasks > 0 {
            failed_tasks as f64 / total_tasks as f64
        } else {
            0.0
        };
        let timeout_rate = if total_tasks > 0 {
            timeout_count as f64 / total_tasks as f64
        } else {
            0.0
        };
        let cancellation_rate = if total_tasks > 0 {
            cancellation_count as f64 / total_tasks as f64
        } else {
            0.0
        };

        // Calculate completion times
        let mut completion_times: Vec<Duration> = completed_tasks
            .iter()
            .filter_map(|t| {
                t.end_time
                    .and_then(|end| (end - t.start_time).to_std().ok())
            })
            .collect();

        completion_times.sort();

        let avg_completion_time = if !completion_times.is_empty() {
            completion_times.iter().sum::<Duration>() / completion_times.len() as u32
        } else {
            Duration::ZERO
        };

        let median_completion_time = if !completion_times.is_empty() {
            let mid = completion_times.len() / 2;
            if completion_times.len().is_multiple_of(2) {
                (completion_times[mid - 1] + completion_times[mid]) / 2
            } else {
                completion_times[mid]
            }
        } else {
            Duration::ZERO
        };

        let p95_completion_time = if !completion_times.is_empty() {
            let idx = (completion_times.len() as f64 * 0.95) as usize;
            completion_times[idx.min(completion_times.len() - 1)]
        } else {
            Duration::ZERO
        };

        let p99_completion_time = if !completion_times.is_empty() {
            let idx = (completion_times.len() as f64 * 0.99) as usize;
            completion_times[idx.min(completion_times.len() - 1)]
        } else {
            Duration::ZERO
        };

        Ok(TaskPerformanceMetrics {
            success_rate,
            error_rate,
            avg_completion_time,
            median_completion_time,
            p95_completion_time,
            p99_completion_time,
            total_tasks,
            successful_tasks,
            failed_tasks,
            in_progress_tasks,
            timeout_rate,
            cancellation_rate,
        })
    }

    /// Collect response metrics
    async fn collect_response_metrics(&self, agent_id: &str) -> Result<ResponsePerformanceMetrics> {
        let state = self.state.read().await;
        let agent_state = state.get(agent_id).cloned().unwrap_or_else(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        let responses = agent_state.responses;
        let total_requests = responses.len() as u64;
        let successful_requests = responses.iter().filter(|r| r.success).count() as u64;
        let failed_requests = responses.iter().filter(|r| !r.success).count() as u64;

        if responses.is_empty() {
            return Ok(ResponsePerformanceMetrics::default());
        }

        let mut latencies: Vec<Duration> = responses.iter().map(|r| r.latency).collect();
        latencies.sort();

        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let median_latency = if latencies.len().is_multiple_of(2) {
            let mid = latencies.len() / 2;
            (latencies[mid - 1] + latencies[mid]) / 2
        } else {
            latencies[latencies.len() / 2]
        };

        let p95_latency = {
            let idx = (latencies.len() as f64 * 0.95) as usize;
            latencies[idx.min(latencies.len() - 1)]
        };

        let p99_latency = {
            let idx = (latencies.len() as f64 * 0.99) as usize;
            latencies[idx.min(latencies.len() - 1)]
        };

        let timeout_count = responses.iter().filter(|r| r.timeout).count() as u64;
        let timeout_rate = if total_requests > 0 {
            timeout_count as f64 / total_requests as f64
        } else {
            0.0
        };

        let avg_response_size = if !responses.is_empty() {
            responses.iter().map(|r| r.size).sum::<usize>() / responses.len()
        } else {
            0
        };

        // Calculate throughput (simplified - would need time window in real implementation)
        let throughput = if !responses.is_empty() && avg_latency > Duration::ZERO {
            1.0 / avg_latency.as_secs_f64()
        } else {
            0.0
        };

        Ok(ResponsePerformanceMetrics {
            avg_latency,
            median_latency,
            p95_latency,
            p99_latency,
            throughput,
            timeout_rate,
            current_rps: throughput,
            peak_rps: throughput, // Simplified - would track peak over time
            avg_response_size,
            total_requests,
            successful_requests,
            failed_requests,
        })
    }

    /// Collect resource metrics
    async fn collect_resource_metrics(&self, agent_id: &str) -> Result<ResourcePerformanceMetrics> {
        let cpu_usage = self.resource_monitor.get_cpu_usage().await.unwrap_or(0.0);
        let memory_usage = self.resource_monitor.get_memory_usage().await.unwrap_or(0);
        let peak_memory_usage = self
            .resource_monitor
            .get_peak_memory_usage()
            .await
            .unwrap_or(0);
        let memory_usage_percent = self
            .resource_monitor
            .get_memory_usage_percent()
            .await
            .unwrap_or(0.0);

        let state = self.state.read().await;
        let agent_state = state.get(agent_id).cloned().unwrap_or_else(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        let llm_calls = agent_state.llm_calls;
        let llm_calls_count = llm_calls.len() as u64;
        let total_tokens = llm_calls.iter().map(|c| c.input_tokens + c.output_tokens).sum::<usize>() as u64;
        let input_tokens = llm_calls.iter().map(|c| c.input_tokens).sum::<usize>() as u64;
        let output_tokens = llm_calls.iter().map(|c| c.output_tokens).sum::<usize>() as u64;
        let estimated_cost = llm_calls.iter().map(|c| c.cost).sum::<f64>();
        let error_count = llm_calls.iter().filter(|c| c.error).count() as u64;
        let rate_limit_hits = llm_calls.iter().filter(|c| c.rate_limited).count() as u64;

        let llm_error_rate = if llm_calls_count > 0 {
            error_count as f64 / llm_calls_count as f64
        } else {
            0.0
        };

        let responses = agent_state.responses;
        let total_requests = responses.len() as u64;
        let cost_per_request = if total_requests > 0 {
            estimated_cost / total_requests as f64
        } else {
            0.0
        };

        let avg_tokens_per_request = if total_requests > 0 {
            total_tokens as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(ResourcePerformanceMetrics {
            cpu_usage,
            memory_usage,
            memory_usage_percent,
            peak_memory_usage,
            llm_calls: llm_calls_count,
            total_tokens,
            input_tokens,
            output_tokens,
            estimated_cost,
            cost_per_request,
            avg_tokens_per_request,
            llm_error_rate,
            rate_limit_hits,
        })
    }

    /// Collect quality metrics (domain-specific, returns defaults for now)
    async fn collect_quality_metrics(&self, _agent_id: &str) -> Result<QualityPerformanceMetrics> {
        // Quality metrics are domain-specific and would be provided by the application
        Ok(QualityPerformanceMetrics::default())
    }

    /// Collect memory metrics (with agent)
    #[allow(dead_code)]
    async fn collect_memory_metrics(
        &self,
        agent_id: &str,
        _agent: &Agent,
    ) -> Result<MemoryPerformanceMetrics> {
        self.collect_memory_metrics_by_id(agent_id).await
    }

    /// Collect memory metrics by agent ID (without agent instance)
    async fn collect_memory_metrics_by_id(
        &self,
        agent_id: &str,
    ) -> Result<MemoryPerformanceMetrics> {
        let state = self.state.read().await;
        let agent_state = state.get(agent_id).cloned().unwrap_or_else(|| MetricsState {
            tasks: Vec::new(),
            responses: Vec::new(),
            llm_calls: Vec::new(),
            memory_operations: Vec::new(),
        });

        // Get memory count - placeholder (would need memory system access)
        let total_memories = 0;

        let memory_ops = agent_state.memory_operations;
        let search_ops: Vec<_> = memory_ops
            .iter()
            .filter(|op| op.operation_type == "search")
            .collect();

        let avg_search_latency = if !search_ops.is_empty() {
            let total: Duration = search_ops.iter().map(|op| op.latency).sum();
            total / search_ops.len() as u32
        } else {
            Duration::ZERO
        };

        let successful_searches = search_ops.iter().filter(|op| op.success).count();
        let search_success_rate = if !search_ops.is_empty() {
            successful_searches as f64 / search_ops.len() as f64
        } else {
            1.0
        };

        let memories_found_sum: usize = search_ops
            .iter()
            .filter_map(|op| op.memories_found)
            .sum();
        let avg_memories_per_search = if !search_ops.is_empty() {
            memories_found_sum as f64 / search_ops.len() as f64
        } else {
            0.0
        };

        let memory_hit_rate = if !search_ops.is_empty() {
            let hits = search_ops.iter().filter(|op| {
                op.memories_found.map(|n| n > 0).unwrap_or(false)
            }).count();
            hits as f64 / search_ops.len() as f64
        } else {
            0.0
        };

        Ok(MemoryPerformanceMetrics {
            total_memories,
            avg_search_latency,
            search_success_rate,
            consolidation_effectiveness: 1.0,
            concept_extraction_accuracy: None,
            memory_hit_rate,
            avg_memories_per_search,
            storage_efficiency: 1.0,
        })
    }

    /// Calculate overall performance score
    fn calculate_overall_score(
        &self,
        task: &TaskPerformanceMetrics,
        response: &ResponsePerformanceMetrics,
        resource: &ResourcePerformanceMetrics,
        quality: &QualityPerformanceMetrics,
        memory: &MemoryPerformanceMetrics,
    ) -> f64 {
        let task_score = task.success_rate * self.weights.task_weight;
        let response_score = if response.total_requests > 0 {
            let success_rate = response.successful_requests as f64 / response.total_requests as f64;
            let latency_score = if response.p95_latency.as_secs() < 5 {
                1.0
            } else if response.p95_latency.as_secs() < 10 {
                0.8
            } else {
                0.5
            };
            (success_rate + latency_score) / 2.0 * self.weights.response_weight
        } else {
            1.0 * self.weights.response_weight
        };
        let resource_score = {
            let cpu_score = 1.0 - resource.cpu_usage.min(1.0);
            let memory_score = 1.0 - resource.memory_usage_percent.min(1.0);
            let cost_score = if resource.cost_per_request < 0.10 {
                1.0
            } else if resource.cost_per_request < 0.20 {
                0.8
            } else {
                0.5
            };
            (cpu_score + memory_score + cost_score) / 3.0 * self.weights.resource_weight
        };
        let quality_score = {
            let mut scores = Vec::new();
            if let Some(satisfaction) = quality.user_satisfaction {
                scores.push(satisfaction);
            }
            if let Some(q) = quality.output_quality {
                scores.push(q);
            }
            (if scores.is_empty() {
                0.5
            } else {
                scores.iter().sum::<f64>() / scores.len() as f64
            }) * self.weights.quality_weight
        };
        let memory_score = {
            let search_score = memory.search_success_rate;
            let hit_rate_score = memory.memory_hit_rate;
            let latency_score = if memory.avg_search_latency.as_millis() < 100 {
                1.0
            } else if memory.avg_search_latency.as_millis() < 500 {
                0.8
            } else {
                0.5
            };
            (search_score + hit_rate_score + latency_score) / 3.0 * self.weights.memory_weight
        };

        task_score + response_score + resource_score + quality_score + memory_score
    }

    /// Determine performance trend
    async fn determine_trend(&self, agent_id: &str, current_score: f64) -> Result<PerformanceTrend> {
        let history = self.history.read().await;
        if let Some(previous) = history.get(agent_id).and_then(|h| h.last()) {
            let diff = current_score - previous.overall_score;
            if diff > 0.05 {
                return Ok(PerformanceTrend::Improving);
            } else if diff < -0.05 {
                return Ok(PerformanceTrend::Declining);
            } else {
                return Ok(PerformanceTrend::Stable);
            }
        }
        Ok(PerformanceTrend::Unknown)
    }

    /// Calculate performance variance
    async fn calculate_variance(&self, agent_id: &str) -> Result<f64> {
        let history = self.history.read().await;
        if let Some(agent_history) = history.get(agent_id) {
            if agent_history.len() < 2 {
                return Ok(0.0);
            }

            let scores: Vec<f64> = agent_history.iter().map(|m| m.overall_score).collect();
            let mean = scores.iter().sum::<f64>() / scores.len() as f64;
            let variance = scores
                .iter()
                .map(|s| (s - mean).powi(2))
                .sum::<f64>()
                / scores.len() as f64;

            Ok(variance)
        } else {
            Ok(0.0)
        }
    }

    /// Get metrics history for an agent
    pub async fn get_history(&self, agent_id: &str) -> Result<Vec<AgentMetrics>> {
        let history = self.history.read().await;
        Ok(history.get(agent_id).cloned().unwrap_or_default())
    }

    /// Set performance weights
    pub fn set_weights(&mut self, weights: PerformanceWeights) {
        self.weights = weights;
    }
}

