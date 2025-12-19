//! Tests for metrics collection

use crate::metrics::{
    InMemoryMetricsStorage, LLMCostCalculator, MetricsCollector, MetricsStorage,
    PerformanceWeights, ResourceMonitor, StubResourceMonitor,
};
use crate::llm::{ModelInfo, TokenUsage};
use std::sync::Arc;
use std::time::Duration;

// Mock resource monitor for testing
struct MockResourceMonitor {
    cpu_usage: f64,
    memory_usage: usize,
    memory_usage_percent: f64,
    peak_memory_usage: usize,
}

#[async_trait::async_trait]
impl ResourceMonitor for MockResourceMonitor {
    async fn get_cpu_usage(&self) -> crate::error::Result<f64> {
        Ok(self.cpu_usage)
    }

    async fn get_memory_usage(&self) -> crate::error::Result<usize> {
        Ok(self.memory_usage)
    }

    async fn get_peak_memory_usage(&self) -> crate::error::Result<usize> {
        Ok(self.peak_memory_usage)
    }

    async fn get_memory_usage_percent(&self) -> crate::error::Result<f64> {
        Ok(self.memory_usage_percent)
    }
}

#[tokio::test]
async fn test_task_metrics_collection() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(MockResourceMonitor {
        cpu_usage: 0.5,
        memory_usage: 1024 * 1024,
        memory_usage_percent: 0.3,
        peak_memory_usage: 2 * 1024 * 1024,
    });
    let collector = MetricsCollector::new(storage, resource_monitor);

    // Record some tasks
    collector.record_task_start("test-agent").await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    collector
        .record_task_complete("test-agent", true, false, false)
        .await
        .unwrap();

    collector.record_task_start("test-agent").await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    collector
        .record_task_complete("test-agent", true, false, false)
        .await
        .unwrap();

    collector.record_task_start("test-agent").await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    collector
        .record_task_complete("test-agent", false, false, false)
        .await
        .unwrap();

    // Create a mock agent for collection
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    assert_eq!(metrics.task_performance.total_tasks, 3);
    assert_eq!(metrics.task_performance.successful_tasks, 2);
    assert_eq!(metrics.task_performance.failed_tasks, 1);
    assert!(metrics.task_performance.success_rate > 0.6);
    assert!(metrics.task_performance.avg_completion_time > Duration::ZERO);
}

#[tokio::test]
async fn test_response_metrics_collection() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let collector = MetricsCollector::new(storage, resource_monitor);

    // Record some responses
    collector
        .record_response("test-agent", Duration::from_millis(100), true, 1024, false)
        .await
        .unwrap();
    collector
        .record_response("test-agent", Duration::from_millis(200), true, 2048, false)
        .await
        .unwrap();
    collector
        .record_response("test-agent", Duration::from_millis(150), false, 512, true)
        .await
        .unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    assert_eq!(metrics.response_performance.total_requests, 3);
    assert_eq!(metrics.response_performance.successful_requests, 2);
    assert_eq!(metrics.response_performance.failed_requests, 1);
    assert!(metrics.response_performance.avg_latency > Duration::ZERO);
    assert!(metrics.response_performance.p95_latency >= metrics.response_performance.avg_latency);
}

#[tokio::test]
async fn test_llm_cost_calculation() {
    let mut calculator = LLMCostCalculator::new();

    let model_info = ModelInfo {
        provider: "openai".to_string(),
        model_name: "gpt-4".to_string(),
    };

    let usage = TokenUsage {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
    };

    let cost = calculator.calculate_cost(&model_info, &usage);
    // Should be approximately (1000/1M * 30) + (500/1M * 60) = 0.03 + 0.03 = 0.06
    assert!(cost > 0.0);
    assert!(cost < 1.0); // Should be reasonable

    // Test custom pricing
    calculator.set_pricing("custom", "model-1", 10.0, 20.0);
    let custom_model = ModelInfo {
        provider: "custom".to_string(),
        model_name: "model-1".to_string(),
    };
    let custom_cost = calculator.calculate_cost(&custom_model, &usage);
    assert!(custom_cost > 0.0);
}

#[tokio::test]
async fn test_llm_metrics_collection() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let collector = MetricsCollector::new(storage, resource_monitor);

    let model_info = ModelInfo {
        provider: "openai".to_string(),
        model_name: "gpt-4".to_string(),
    };

    // Record some LLM calls
    collector
        .record_llm_call("test-agent", 1000, 500, model_info.clone(), false, false)
        .await
        .unwrap();
    collector
        .record_llm_call("test-agent", 2000, 1000, model_info.clone(), false, false)
        .await
        .unwrap();
    collector
        .record_llm_call("test-agent", 500, 250, model_info.clone(), true, false)
        .await
        .unwrap();
    collector
        .record_llm_call("test-agent", 1000, 500, model_info.clone(), false, true)
        .await
        .unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    assert_eq!(metrics.resource_performance.llm_calls, 4);
    assert_eq!(metrics.resource_performance.input_tokens, 4500);
    assert_eq!(metrics.resource_performance.output_tokens, 2250);
    assert_eq!(metrics.resource_performance.rate_limit_hits, 1);
    assert!(metrics.resource_performance.estimated_cost > 0.0);
}

#[tokio::test]
async fn test_memory_metrics_collection() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let collector = MetricsCollector::new(storage, resource_monitor);

    // Record some memory operations
    collector
        .record_memory_operation(
            "test-agent",
            "search".to_string(),
            Duration::from_millis(50),
            true,
            Some(5),
        )
        .await
        .unwrap();
    collector
        .record_memory_operation(
            "test-agent",
            "search".to_string(),
            Duration::from_millis(100),
            true,
            Some(10),
        )
        .await
        .unwrap();
    collector
        .record_memory_operation(
            "test-agent",
            "search".to_string(),
            Duration::from_millis(75),
            false,
            None,
        )
        .await
        .unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    assert!(metrics.memory_performance.avg_search_latency > Duration::ZERO);
    assert!(metrics.memory_performance.search_success_rate < 1.0); // One failed search
    assert!(metrics.memory_performance.memory_hit_rate > 0.0);
}

#[tokio::test]
async fn test_metrics_storage() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let collector = MetricsCollector::new(storage.clone(), resource_monitor);

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    // Collect metrics multiple times
    let _metrics1 = collector.collect_metrics(&agent).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let metrics2 = collector.collect_metrics(&agent).await.unwrap();

    // Check storage
    let latest = storage.get_latest_metrics("test-agent").await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().agent_id, "test-agent");

    let history = storage
        .get_metrics_history("test-agent", None)
        .await
        .unwrap();
    assert!(history.len() >= 2);

    // Check trend
    assert_ne!(metrics2.trend, crate::metrics::PerformanceTrend::Unknown);
}

#[tokio::test]
async fn test_performance_score_calculation() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(MockResourceMonitor {
        cpu_usage: 0.3,
        memory_usage: 512 * 1024,
        memory_usage_percent: 0.2,
        peak_memory_usage: 1024 * 1024,
    });
    let collector = MetricsCollector::new(storage, resource_monitor);

    // Record successful operations
    collector.record_task_start("test-agent").await.unwrap();
    collector
        .record_task_complete("test-agent", true, false, false)
        .await
        .unwrap();

    collector
        .record_response("test-agent", Duration::from_millis(50), true, 1024, false)
        .await
        .unwrap();

    let model_info = ModelInfo {
        provider: "openai".to_string(),
        model_name: "gpt-3.5-turbo".to_string(),
    };
    collector
        .record_llm_call("test-agent", 100, 50, model_info, false, false)
        .await
        .unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    // Overall score should be reasonable
    assert!(metrics.overall_score >= 0.0);
    assert!(metrics.overall_score <= 1.0);
    assert!(metrics.overall_score > 0.5); // Should be decent with successful operations
}

#[tokio::test]
async fn test_metrics_history_trend() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let mut collector = MetricsCollector::new(storage, resource_monitor);

    // Set custom weights to make trends more visible
    let mut weights = PerformanceWeights::default();
    weights.task_weight = 1.0;
    weights.response_weight = 0.0;
    weights.resource_weight = 0.0;
    weights.quality_weight = 0.0;
    weights.memory_weight = 0.0;
    collector.set_weights(weights);

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    // First collection - all successful
    collector.record_task_start("test-agent").await.unwrap();
    collector
        .record_task_complete("test-agent", true, false, false)
        .await
        .unwrap();
    let _metrics1 = collector.collect_metrics(&agent).await.unwrap();

    // Second collection - still successful
    collector.record_task_start("test-agent").await.unwrap();
    collector
        .record_task_complete("test-agent", true, false, false)
        .await
        .unwrap();
    let metrics2 = collector.collect_metrics(&agent).await.unwrap();

    // Should be stable or improving
    assert!(
        metrics2.trend == crate::metrics::PerformanceTrend::Stable
            || metrics2.trend == crate::metrics::PerformanceTrend::Improving
    );

    // Check variance
    assert!(metrics2.variance >= 0.0);
}

#[tokio::test]
async fn test_baseline_metrics() {
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let collector = MetricsCollector::new(storage.clone(), resource_monitor);

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = crate::config::MemoryConfig::default();
    memory_config.mode = crate::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = crate::agent::Agent::builder()
        .id("test-agent")
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let metrics = collector.collect_metrics(&agent).await.unwrap();

    // Set as baseline (using the concrete type directly)
    let storage_concrete = storage.as_ref() as &InMemoryMetricsStorage;
    storage_concrete.set_baseline_metrics("test-agent", metrics.clone()).await.unwrap();

    // Get baseline
    let baseline = storage.get_baseline_metrics("test-agent").await.unwrap();
    assert!(baseline.is_some());
    assert_eq!(baseline.unwrap().agent_id, metrics.agent_id);
}

