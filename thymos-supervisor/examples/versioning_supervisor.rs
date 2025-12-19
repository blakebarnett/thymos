//! Example: Versioning Supervisor with Automatic Branching and Rollback
//!
//! This example demonstrates how to use the VersioningSupervisor to:
//! - Automatically create experiment branches when performance drops
//! - Monitor agent performance and make decisions
//! - Automatically rollback failed experiments
//! - Automatically merge successful experiments

use std::sync::Arc;
use std::time::Duration;
use thymos_core::config::MemoryConfig;
use thymos_core::memory::versioning::MemoryRepository;
use thymos_core::metrics::{InMemoryMetricsStorage, MetricsCollector, StubResourceMonitor};
use thymos_supervisor::{
    AgentSupervisor, AutoDecisionEngine, DecisionAction, DecisionCondition, DecisionRule,
    ProcessSupervisor, SupervisorConfig, VersioningSupervisor, VersioningSupervisorConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Versioning Supervisor Example ===\n");

    // 1. Create base supervisor
    println!("1. Creating base supervisor...");
    let base_supervisor: Arc<dyn AgentSupervisor> = Arc::new(
        ProcessSupervisor::new(SupervisorConfig::default()).await
            .map_err(|e| format!("Failed to create supervisor: {}", e))?,
    );
    println!("   ✓ Base supervisor created\n");

    // 2. Create memory repository
    println!("2. Creating memory repository...");
    let temp_dir = tempfile::TempDir::new()?;
    let mut memory_config = MemoryConfig::default();
    memory_config.mode = thymos_core::config::MemoryMode::Embedded {
        data_dir: temp_dir.path().to_path_buf(),
    };
    
    // Create agent first to get Locai instance
    let temp_agent = thymos_core::agent::Agent::builder()
        .id("temp-for-repo")
        .with_memory_config(memory_config.clone())
        .build()
        .await?;
    
    // Extract Locai from agent's memory system
    // Note: This is a workaround - in practice you'd have direct access to Locai
    let memory_system = temp_agent.memory();
    let locai = match memory_system {
        thymos_core::memory::MemorySystem::Single { locai, .. } => locai.clone(),
        _ => return Err("Hybrid memory mode not supported in this example".into()),
    };
    
    let memory_repo = Arc::new(
        MemoryRepository::new(locai)
            .await
            .map_err(|e| format!("Failed to create repository: {}", e))?,
    );
    println!("   ✓ Memory repository created\n");

    // 3. Create metrics collector
    println!("3. Creating metrics collector...");
    let storage = Arc::new(InMemoryMetricsStorage::new());
    let resource_monitor = Arc::new(StubResourceMonitor);
    let metrics = Arc::new(MetricsCollector::new(storage.clone(), resource_monitor));
    println!("   ✓ Metrics collector created\n");

    // 4. Record some sample metrics to demonstrate functionality
    println!("4. Recording sample metrics...");
    let agent_id = "example-agent";
    
    // Record some tasks
    metrics.record_task_start(agent_id).await?;
    metrics.record_task_complete(agent_id, true, false, false).await?;
    metrics.record_task_start(agent_id).await?;
    metrics.record_task_complete(agent_id, false, false, false).await?;
    
    // Record some responses
    metrics.record_response(agent_id, Duration::from_millis(150), true, 1024, false).await?;
    metrics.record_response(agent_id, Duration::from_millis(200), true, 2048, false).await?;
    
    // Record some LLM calls
    let model_info = thymos_core::llm::ModelInfo {
        provider: "openai".to_string(),
        model_name: "gpt-3.5-turbo".to_string(),
    };
    metrics.record_llm_call(agent_id, 1000, 500, model_info.clone(), false, false).await?;
    
    // Create agent for metrics collection
    let agent = thymos_core::agent::Agent::builder()
        .id(agent_id)
        .with_memory_config(memory_config.clone())
        .build()
        .await?;
    let _initial_metrics = metrics.collect_metrics(&agent).await?;
    println!("   ✓ Sample metrics recorded\n");

    // 5. Create versioning supervisor
    println!("5. Creating versioning supervisor...");
    let config = VersioningSupervisorConfig {
        auto_branching_enabled: true,
        auto_rollback_enabled: true,
        auto_merge_enabled: true,
        ab_testing_enabled: false,
        ..Default::default()
    };
    
    let versioning_supervisor = VersioningSupervisor::new(
        base_supervisor,
        memory_repo.clone(),
        metrics.clone(),
        config,
    );
    println!("   ✓ Versioning supervisor created\n");

    // 6. Demonstrate decision engine
    println!("6. Demonstrating decision engine...");
    let mut decision_engine = AutoDecisionEngine::new(metrics.clone());
    
    // Add a decision rule
    decision_engine.add_rule(DecisionRule {
        name: "performance-drop".to_string(),
        condition: DecisionCondition::PerformanceDeclining,
        action: DecisionAction::CreateBranch {
            branch_name: "experiment-performance".to_string(),
            description: "Performance declining".to_string(),
        },
        priority: 10,
    });
    
    // Evaluate rules
    let actions = decision_engine.evaluate(agent_id).await?;
    println!("   ✓ Decision engine evaluated");
    println!("   → Generated {} actions", actions.len());
    for (i, action) in actions.iter().enumerate() {
        println!("     Action {}: {:?}", i + 1, action);
    }
    println!();

    // 7. Demonstrate automatic experiment branch creation
    println!("7. Checking if should create experiment branch...");
    let should_experiment = decision_engine.should_experiment(agent_id).await?;
    if should_experiment {
        println!("   ✓ Should create experiment branch");
        match versioning_supervisor
            .auto_create_experiment_branch(agent_id, "Performance metrics indicate experimentation needed")
            .await
        {
            Ok(branch_name) => {
                println!("   ✓ Created experiment branch: {}", branch_name);
            }
            Err(e) => {
                println!("   ⚠ Could not create branch: {}", e);
            }
        }
    } else {
        println!("   → No experiment needed at this time");
    }
    println!();

    // 8. Demonstrate monitoring
    println!("8. Demonstrating agent monitoring...");
    match versioning_supervisor.monitor_agent(agent_id).await {
        Ok(actions) => {
            println!("   ✓ Monitoring completed");
            println!("   → Generated {} decision actions", actions.len());
            for (i, action) in actions.iter().enumerate() {
                println!("     Action {}: {:?}", i + 1, action);
            }
        }
        Err(e) => {
            println!("   ⚠ Monitoring failed: {}", e);
        }
    }
    println!();

    // 9. Demonstrate rollback check
    println!("9. Checking if rollback is needed...");
    let branch_name = "experiment-example-agent-12345";
    let should_rollback = decision_engine.should_rollback(agent_id, branch_name).await?;
    if should_rollback {
        println!("   ⚠ Rollback recommended");
        match versioning_supervisor.auto_rollback_on_failure(agent_id, branch_name).await {
            Ok(()) => println!("   ✓ Rollback completed"),
            Err(e) => println!("   ⚠ Rollback failed: {}", e),
        }
    } else {
        println!("   ✓ No rollback needed");
    }
    println!();

    // 10. Demonstrate merge check
    println!("10. Checking if merge is needed...");
    let should_merge = decision_engine.should_merge(agent_id, branch_name).await?;
    if should_merge {
        println!("   ✓ Merge recommended");
        match versioning_supervisor.auto_merge_on_success(agent_id, branch_name).await {
            Ok(()) => println!("   ✓ Merge completed"),
            Err(e) => println!("   ⚠ Merge failed: {}", e),
        }
    } else {
        println!("   → Not ready to merge yet");
    }
    println!();

    // 11. Show active experiments
    println!("11. Checking active experiments...");
    let experiments = versioning_supervisor.get_active_experiments().await;
    println!("   ✓ Active experiments: {}", experiments.len());
    for (agent_id, branch_name) in experiments {
        println!("     - Agent '{}' on branch '{}'", agent_id, branch_name);
    }
    println!();

    // 12. Show metrics
    println!("12. Current agent metrics:");
    if let Some(current_metrics) = metrics.get_agent_metrics(agent_id).await? {
        println!("   ✓ Overall score: {:.2}", current_metrics.overall_score);
        println!("   ✓ Trend: {:?}", current_metrics.trend);
        println!("   ✓ Task success rate: {:.2}", current_metrics.task_performance.success_rate);
        println!("   ✓ Response latency (p95): {:?}", current_metrics.response_performance.p95_latency);
        println!("   ✓ Cost per request: ${:.4}", current_metrics.resource_performance.cost_per_request);
    } else {
        println!("   → No metrics available yet");
    }
    println!();

    println!("=== Example Complete ===");
    println!("\nThe versioning supervisor is now fully functional!");
    println!("Next steps:");
    println!("1. Add more decision rules for custom behavior");
    println!("2. Configure merge strategies for conflict resolution");
    println!("3. Enable A/B testing for parallel experiments");
    println!("4. Integrate with actual agent workloads");

    Ok(())
}
