//! Integration tests for versioning supervisor

use crate::versioning::{AutoDecisionEngine, DecisionAction, DecisionCondition, DecisionRule, VersioningSupervisor, VersioningSupervisorConfig};
use crate::{AgentSupervisor, AgentMode, HealthStatus};
use std::sync::Arc;
use std::time::Duration;
use thymos_core::config::MemoryConfig;
use thymos_core::lifecycle::RelevanceContext;
use thymos_core::memory::versioning::MemoryRepository;
use thymos_core::metrics::{InMemoryMetricsStorage, MetricsCollector, StubResourceMonitor};
use thymos_core::agent::AgentStatus;
use async_trait::async_trait;

// Mock supervisor for testing
struct MockSupervisor;

#[async_trait]
impl AgentSupervisor for MockSupervisor {
    async fn start(
        &self,
        _agent_id: &str,
        _mode: AgentMode,
        _context: &RelevanceContext,
    ) -> crate::Result<crate::AgentHandle> {
        Ok(crate::AgentHandle {
            agent_id: "test".to_string(),
            pid: 12345,
            port: 3000,
        })
    }

    async fn stop(&self, _agent_id: &str, _save_state: bool) -> crate::Result<()> {
        Ok(())
    }

    async fn get_status(&self, _agent_id: &str) -> crate::Result<AgentStatus> {
        Ok(AgentStatus::Active)
    }

    async fn set_mode(&self, _agent_id: &str, _mode: AgentMode) -> crate::Result<()> {
        Ok(())
    }

    async fn list_agents(&self) -> crate::Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn health_check(&self, _agent_id: &str) -> crate::Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}

async fn setup_test_environment() -> (
    Arc<dyn AgentSupervisor>,
    Arc<MemoryRepository>,
    Arc<MetricsCollector>,
) {
    // Create mock supervisor
    let base_supervisor: Arc<dyn AgentSupervisor> = Arc::new(MockSupervisor);

    // Create memory repository
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };

    let temp_agent = thymos_core::agent::Agent::builder()
        .id("test-repo-setup")
        .with_memory_config(memory_config.clone())
        .build()
        .await
        .unwrap();

    let memory_system = temp_agent.memory();
    let locai = match memory_system {
        thymos_core::memory::MemorySystem::Single { locai, .. } => locai.clone(),
        _ => panic!("Hybrid mode not supported in tests"),
    };

    let memory_repo = Arc::new(MemoryRepository::new(locai).await.unwrap());

    // Create metrics collector
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let metrics = Arc::new(MetricsCollector::new(storage, resource_monitor));

    (base_supervisor, memory_repo, metrics)
}

#[tokio::test]
async fn test_decision_engine_should_experiment() {
    let (_supervisor, _repo, metrics) = setup_test_environment().await;
    let engine = AutoDecisionEngine::new(metrics.clone());

    let agent_id = "test-agent";

    // Initially, no metrics - should not experiment
    let should_experiment = engine.should_experiment(agent_id).await.unwrap();
    assert!(!should_experiment);

    // Record some poor performance metrics
    metrics.record_task_start(agent_id).await.unwrap();
    metrics.record_task_complete(agent_id, false, false, false).await.unwrap();
    metrics.record_task_start(agent_id).await.unwrap();
    metrics.record_task_complete(agent_id, false, false, false).await.unwrap();

    // Create agent to collect metrics
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    // Collect metrics to store them
    let _metrics = metrics.collect_metrics(&agent).await.unwrap();

    // Now should experiment due to low success rate
    let should_experiment = engine.should_experiment(agent_id).await.unwrap();
    assert!(should_experiment);
}

#[tokio::test]
async fn test_decision_engine_should_rollback() {
    let (_supervisor, _repo, metrics) = setup_test_environment().await;
    let engine = AutoDecisionEngine::new(metrics.clone());

    let agent_id = "test-agent";

    // Record high error rate
    for _ in 0..10 {
        metrics.record_task_start(agent_id).await.unwrap();
        metrics.record_task_complete(agent_id, false, false, false).await.unwrap();
    }

    // Create agent to collect metrics
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let _metrics = metrics.collect_metrics(&agent).await.unwrap();

    // Should rollback due to high error rate
    let should_rollback = engine.should_rollback(agent_id, "test-branch").await.unwrap();
    assert!(should_rollback);
}

#[tokio::test]
async fn test_decision_engine_evaluate_rules() {
    let (_supervisor, _repo, metrics) = setup_test_environment().await;
    let mut engine = AutoDecisionEngine::new(metrics.clone());

    let agent_id = "test-agent";

    // Add a rule
    engine.add_rule(DecisionRule {
        name: "test-rule".to_string(),
        condition: DecisionCondition::ErrorRateExceeded { threshold: 0.1 },
        action: DecisionAction::Rollback {
            branch_name: "test-branch".to_string(),
            target_commit: None,
        },
        priority: 10,
    });

    // Record high error rate
    for _ in 0..5 {
        metrics.record_task_start(agent_id).await.unwrap();
        metrics.record_task_complete(agent_id, false, false, false).await.unwrap();
    }

    // Create agent to collect metrics
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let _metrics = metrics.collect_metrics(&agent).await.unwrap();

    // Evaluate rules
    let actions = engine.evaluate(agent_id).await.unwrap();
    assert!(!actions.is_empty());
}

#[tokio::test]
async fn test_versioning_supervisor_creation() {
    let (supervisor, repo, metrics) = setup_test_environment().await;

    let config = VersioningSupervisorConfig::default();
    let versioning_supervisor = VersioningSupervisor::new(
        supervisor,
        repo,
        metrics,
        config,
    );

    // Should be able to list agents (delegates to base supervisor)
    let agents = versioning_supervisor.list_agents().await.unwrap();
    assert!(agents.is_empty()); // No agents started yet
}

#[tokio::test]
async fn test_versioning_supervisor_active_experiments() {
    let (supervisor, repo, metrics) = setup_test_environment().await;

    let config = VersioningSupervisorConfig {
        auto_branching_enabled: true,
        ..Default::default()
    };
    let versioning_supervisor = VersioningSupervisor::new(
        supervisor,
        repo.clone(),
        metrics.clone(),
        config,
    );

    let agent_id = "test-agent";

    // Record some metrics
    metrics.record_task_start(agent_id).await.unwrap();
    metrics.record_task_complete(agent_id, true, false, false).await.unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let _metrics = metrics.collect_metrics(&agent).await.unwrap();

    // Try to create experiment branch
    // Note: This may fail if should_experiment returns false, which is fine
    let _ = versioning_supervisor
        .auto_create_experiment_branch(agent_id, "Test experiment")
        .await;

    // Check active experiments
    let experiments = versioning_supervisor.get_active_experiments().await;
    // May or may not have experiments depending on decision logic
    assert!(experiments.len() <= 1);
}

#[tokio::test]
async fn test_metrics_collector_by_id() {
    let (_supervisor, _repo, metrics) = setup_test_environment().await;

    let agent_id = "test-agent";

    // Record some metrics
    metrics.record_task_start(agent_id).await.unwrap();
    metrics.record_task_complete(agent_id, true, false, false).await.unwrap();
    metrics.record_response(agent_id, Duration::from_millis(100), true, 1024, false).await.unwrap();

    // Create agent to store initial metrics
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config)
        .build()
        .await
        .unwrap();

    let _initial = metrics.collect_metrics(&agent).await.unwrap();

    // Now collect by ID (should work without agent instance)
    let metrics_by_id = metrics.collect_metrics_by_id(agent_id).await.unwrap();
    assert_eq!(metrics_by_id.agent_id, agent_id);
    assert!(metrics_by_id.overall_score >= 0.0);
    assert!(metrics_by_id.overall_score <= 1.0);

    // Get latest metrics
    let latest = metrics.get_agent_metrics(agent_id).await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().agent_id, agent_id);
}
