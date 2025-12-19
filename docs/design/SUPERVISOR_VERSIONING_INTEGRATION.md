# Supervisor Integration with Memory Versioning

**Date**: December 2024  
**Status**: Design  
**Purpose**: Design automatic supervisor-level integration with Locai memory versioning

## Executive Summary

With Locai memory versioning, the Thymos supervisor can make intelligent decisions about:
- **Automatic Branching**: Create branches for experiments, A/B tests, or risky changes
- **Worktree Management**: Use worktrees for parallel testing and resource optimization
- **Auto-Merge**: Automatically merge successful experiments back to main
- **Auto-Rollback**: Rollback failed experiments automatically
- **A/B Testing**: Automatically run A/B tests and select winners
- **Resource Scaling**: Use worktrees to scale agents horizontally

This enables **self-managing agents** that experiment, learn, and optimize automatically.

---

## Core Concepts

### Supervisor Versioning Manager

A new component that extends the supervisor with versioning capabilities:

```rust
/// Supervisor with versioning capabilities
pub struct VersioningSupervisor {
    /// Base supervisor
    supervisor: Arc<dyn AgentSupervisor>,
    
    /// Memory repository (manages branches/worktrees)
    memory_repo: Arc<MemoryRepository>,
    
    /// Versioning strategies
    strategies: Arc<RwLock<HashMap<String, Arc<dyn VersioningStrategy>>>>,
    
    /// Auto-decision engine
    decision_engine: Arc<AutoDecisionEngine>,
    
    /// Configuration
    config: VersioningSupervisorConfig,
}
```

### Auto-Decision Engine

Makes automatic decisions about when to branch, merge, rollback, etc.:

```rust
/// Automatic decision engine for versioning operations
pub struct AutoDecisionEngine {
    /// Decision rules
    rules: Vec<DecisionRule>,
    
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
    
    /// LLM for complex decisions (optional)
    llm: Option<Arc<dyn LLMProvider>>,
}

/// Decision rule
pub struct DecisionRule {
    pub name: String,
    pub condition: DecisionCondition,
    pub action: DecisionAction,
    pub priority: usize,
}

#[derive(Debug, Clone)]
pub enum DecisionCondition {
    /// Agent performance drops below threshold
    PerformanceDrop { threshold: f64, window: Duration },
    
    /// New strategy available
    NewStrategyAvailable,
    
    /// Resource constraints
    ResourceConstraint { cpu_threshold: f64, memory_threshold: f64 },
    
    /// Experiment timeout
    ExperimentTimeout { max_duration: Duration },
    
    /// Success criteria met
    SuccessCriteriaMet { criteria: SuccessCriteria },
    
    /// Failure criteria met
    FailureCriteriaMet { criteria: FailureCriteria },
    
    /// Custom condition
    Custom { evaluator: Arc<dyn Fn(&AgentMetrics) -> bool> },
}

#[derive(Debug, Clone)]
pub enum DecisionAction {
    /// Create branch for experiment
    CreateBranch { branch_name: String, description: String },
    
    /// Create worktree for parallel testing
    CreateWorktree { branch_name: String, count: usize },
    
    /// Merge branch to main
    MergeBranch { source_branch: String, strategy: MergeStrategy },
    
    /// Rollback to previous state
    Rollback { branch_name: String, target_commit: Option<String> },
    
    /// Delete branch
    DeleteBranch { branch_name: String },
    
    /// Scale worktrees
    ScaleWorktrees { branch_name: String, target_count: usize },
}
```

---

## Automatic Capabilities

### 1. Automatic Experiment Branching

Supervisor automatically creates branches when it detects opportunities for improvement:

```rust
impl VersioningSupervisor {
    /// Automatically create experiment branch when performance drops
    pub async fn auto_create_experiment_branch(
        &self,
        agent_id: &str,
        reason: &str,
    ) -> Result<String> {
        // Get current agent metrics
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        
        // Create branch name
        let branch_name = format!("experiment-{}-{}", agent_id, Utc::now().timestamp());
        
        // Create snapshot of current state
        let snapshot = self.memory_repo.create_snapshot(
            Some(&[agent_id.to_string()]),
            Some(&HashMap::from([
                ("reason".to_string(), Value::String(reason.to_string())),
                ("created_by".to_string(), Value::String("supervisor".to_string())),
                ("metrics".to_string(), serde_json::to_value(&metrics)?),
            ])),
        ).await?;
        
        // Create branch
        let branch_id = self.memory_repo.create_branch(
            &branch_name,
            Some("Auto-created experiment branch"),
            Some(&snapshot.snapshot_id),
        ).await?;
        
        info!("Auto-created experiment branch '{}' for agent {}", branch_name, agent_id);
        
        Ok(branch_id)
    }
}
```

**Use Case**: Agent performance drops → Supervisor automatically creates experiment branch to try new strategies.

### 2. Automatic A/B Testing with Worktrees

Supervisor automatically runs A/B tests using worktrees:

```rust
impl VersioningSupervisor {
    /// Automatically run A/B test
    pub async fn auto_ab_test(
        &self,
        agent_id: &str,
        variants: Vec<ExperimentVariant>,
    ) -> Result<ABTestResult> {
        // Create branches for each variant
        let mut branches = Vec::new();
        for variant in &variants {
            let branch_name = format!("ab-test-{}-{}", agent_id, variant.name);
            let branch_id = self.memory_repo.create_branch(
                &branch_name,
                Some(&format!("A/B test variant: {}", variant.description)),
                None,
            ).await?;
            
            // Apply variant changes
            self.apply_variant(&branch_id, variant).await?;
            
            branches.push((branch_name, branch_id));
        }
        
        // Create worktrees for parallel testing
        let mut worktrees = Vec::new();
        for (branch_name, _) in &branches {
            let worktree_id = self.memory_repo.create_worktree(branch_name, None).await?;
            let agent = self.memory_repo.get_worktree_agent(&worktree_id).await?;
            worktrees.push((worktree_id, agent));
        }
        
        // Run parallel tests
        let mut results = Vec::new();
        for (worktree_id, agent) in &worktrees {
            let result = self.run_test_scenario(agent).await?;
            results.push((worktree_id.clone(), result));
        }
        
        // Select winner
        let winner = self.select_winner(&results).await?;
        
        // Merge winner to main
        let (winner_branch, _) = branches.iter()
            .find(|(name, _)| name == &winner.branch_name)
            .unwrap();
        
        self.memory_repo.merge(
            winner_branch,
            "main",
            MergeStrategy::AutoMerge { llm: self.llm.clone() },
        ).await?;
        
        // Cleanup losing branches
        for (branch_name, _) in &branches {
            if branch_name != winner_branch {
                self.memory_repo.delete_branch(branch_name, true).await?;
            }
        }
        
        Ok(ABTestResult {
            winner: winner.branch_name.clone(),
            results,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExperimentVariant {
    pub name: String,
    pub description: String,
    pub changes: Vec<MemoryChange>,  // What to change
}

#[derive(Debug, Clone)]
pub struct ABTestResult {
    pub winner: String,
    pub results: Vec<(String, TestResult)>,
}
```

**Use Case**: Supervisor automatically tests 3 different strategies in parallel, selects winner, merges to main.

### 3. Automatic Rollback on Failure

Supervisor automatically rolls back when experiments fail:

```rust
impl VersioningSupervisor {
    /// Automatically rollback failed experiment
    pub async fn auto_rollback_on_failure(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        // Get agent metrics
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        
        // Check failure criteria
        if self.is_failure(&metrics).await? {
            warn!("Agent {} on branch {} failed, rolling back", agent_id, branch_name);
            
            // Get previous successful commit
            let previous_commit = self.find_last_successful_commit(agent_id).await?;
            
            // Rollback
            self.memory_repo.checkout_commit(
                &previous_commit,
                &mut self.get_agent(agent_id).await?,
                None,
            ).await?;
            
            // Delete failed branch
            self.memory_repo.delete_branch(branch_name, true).await?;
            
            info!("Rolled back agent {} to commit {}", agent_id, previous_commit);
        }
        
        Ok(())
    }
    
    async fn is_failure(&self, metrics: &AgentMetrics) -> Result<bool> {
        // Check various failure criteria
        if metrics.error_rate > 0.1 {
            return Ok(true);
        }
        
        if metrics.success_rate < 0.5 {
            return Ok(true);
        }
        
        if metrics.response_time > Duration::from_secs(10) {
            return Ok(true);
        }
        
        Ok(false)
    }
}
```

**Use Case**: Experiment causes errors → Supervisor automatically rolls back to last known good state.

### 4. Automatic Resource Scaling with Worktrees

Supervisor uses worktrees to scale agents horizontally:

```rust
impl VersioningSupervisor {
    /// Automatically scale agents using worktrees
    pub async fn auto_scale_with_worktrees(
        &self,
        agent_id: &str,
        target_load: f64,
    ) -> Result<()> {
        // Get current load
        let current_load = self.metrics.get_load(agent_id).await?;
        
        // Calculate needed worktrees
        let current_worktrees = self.memory_repo.list_worktrees_for_agent(agent_id).await?.len();
        let needed_worktrees = (target_load / current_load * current_worktrees as f64).ceil() as usize;
        
        if needed_worktrees > current_worktrees {
            // Scale up - create more worktrees
            let branch_name = self.get_agent_branch(agent_id).await?;
            for _ in current_worktrees..needed_worktrees {
                let worktree_id = self.memory_repo.create_worktree(&branch_name, None).await?;
                let agent = self.memory_repo.get_worktree_agent(&worktree_id).await?;
                
                // Start agent in worktree
                self.supervisor.start(
                    &format!("{}-worktree-{}", agent_id, worktree_id),
                    AgentMode::Active,
                    &RelevanceContext::new(),
                ).await?;
            }
            
            info!("Scaled up agent {} to {} worktrees", agent_id, needed_worktrees);
        } else if needed_worktrees < current_worktrees {
            // Scale down - remove worktrees
            let worktrees = self.memory_repo.list_worktrees_for_agent(agent_id).await?;
            for worktree in worktrees.iter().skip(needed_worktrees) {
                // Stop agent
                self.supervisor.stop(&worktree.agent_id, true).await?;
                
                // Remove worktree
                self.memory_repo.remove_worktree(&worktree.id, false).await?;
            }
            
            info!("Scaled down agent {} to {} worktrees", agent_id, needed_worktrees);
        }
        
        Ok(())
    }
}
```

**Use Case**: High load → Supervisor automatically creates worktrees to handle more requests in parallel.

### 5. Automatic Merge on Success

Supervisor automatically merges successful experiments:

```rust
impl VersioningSupervisor {
    /// Automatically merge successful experiment
    pub async fn auto_merge_on_success(
        &self,
        agent_id: &str,
        branch_name: &str,
    ) -> Result<()> {
        // Get agent metrics
        let metrics = self.metrics.get_agent_metrics(agent_id).await?;
        
        // Check success criteria
        if self.is_success(&metrics).await? {
            info!("Agent {} on branch {} succeeded, merging to main", agent_id, branch_name);
            
            // Merge to main
            self.memory_repo.merge(
                branch_name,
                "main",
                MergeStrategy::AutoMerge { 
                    llm: self.llm.clone() 
                },
            ).await?;
            
            // Delete experiment branch (merged)
            self.memory_repo.delete_branch(branch_name, true).await?;
            
            info!("Merged successful experiment {} to main", branch_name);
        }
        
        Ok(())
    }
    
    async fn is_success(&self, metrics: &AgentMetrics) -> Result<bool> {
        // Check success criteria
        if metrics.success_rate < 0.9 {
            return Ok(false);
        }
        
        if metrics.error_rate > 0.01 {
            return Ok(false);
        }
        
        if metrics.response_time > Duration::from_secs(5) {
            return Ok(false);
        }
        
        // Compare to baseline
        let baseline = self.metrics.get_baseline_metrics().await?;
        if metrics.success_rate < baseline.success_rate * 1.1 {
            return Ok(false);  // Not significantly better
        }
        
        Ok(true)
    }
}
```

**Use Case**: Experiment improves performance by 20% → Supervisor automatically merges to main.

### 6. Continuous Experimentation Loop

Supervisor runs continuous experimentation:

```rust
impl VersioningSupervisor {
    /// Continuous experimentation loop
    pub async fn continuous_experimentation_loop(
        &self,
        agent_id: &str,
    ) -> Result<()> {
        loop {
            // Get current performance
            let metrics = self.metrics.get_agent_metrics(agent_id).await?;
            
            // Check if we should experiment
            if self.should_experiment(&metrics).await? {
                // Generate experiment variants
                let variants = self.generate_experiment_variants(agent_id).await?;
                
                // Run A/B test
                let result = self.auto_ab_test(agent_id, variants).await?;
                
                info!("A/B test completed, winner: {}", result.winner);
            }
            
            // Check for failed experiments
            let active_branches = self.memory_repo.list_branches_for_agent(agent_id).await?;
            for branch in &active_branches {
                if branch.name != "main" {
                    self.auto_rollback_on_failure(agent_id, &branch.name).await?;
                    self.auto_merge_on_success(agent_id, &branch.name).await?;
                }
            }
            
            // Scale based on load
            self.auto_scale_with_worktrees(agent_id, 0.8).await?;
            
            // Wait before next iteration
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
    
    async fn should_experiment(&self, metrics: &AgentMetrics) -> Result<bool> {
        // Experiment if performance is stable but not improving
        if metrics.variance < 0.05 && metrics.trend == Trend::Stable {
            return Ok(true);
        }
        
        // Experiment if performance is declining
        if metrics.trend == Trend::Declining {
            return Ok(true);
        }
        
        Ok(false)
    }
    
    async fn generate_experiment_variants(
        &self,
        agent_id: &str,
    ) -> Result<Vec<ExperimentVariant>> {
        // Use LLM to generate experiment variants
        if let Some(llm) = &self.llm {
            let prompt = format!(
                "Generate 3 experiment variants to improve agent {} performance. \
                Current metrics: {:?}",
                agent_id,
                self.metrics.get_agent_metrics(agent_id).await?
            );
            
            let response = llm.generate(LLMRequest {
                messages: vec![Message::user(&prompt)],
                ..Default::default()
            }).await?;
            
            // Parse variants from LLM response
            self.parse_experiment_variants(&response.content).await
        } else {
            // Fallback: Generate simple variants
            Ok(vec![
                ExperimentVariant {
                    name: "more-aggressive".to_string(),
                    description: "More aggressive strategy".to_string(),
                    changes: vec![],
                },
                ExperimentVariant {
                    name: "more-cautious".to_string(),
                    description: "More cautious strategy".to_string(),
                    changes: vec![],
                },
            ])
        }
    }
}
```

**Use Case**: Supervisor continuously experiments, learns, and optimizes agents automatically.

---

## Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersioningSupervisorConfig {
    /// Enable automatic branching
    pub auto_branching_enabled: bool,
    
    /// Enable automatic A/B testing
    pub auto_ab_testing_enabled: bool,
    
    /// Enable automatic rollback
    pub auto_rollback_enabled: bool,
    
    /// Enable automatic merging
    pub auto_merge_enabled: bool,
    
    /// Enable automatic scaling
    pub auto_scaling_enabled: bool,
    
    /// Success criteria for auto-merge
    pub success_criteria: SuccessCriteria,
    
    /// Failure criteria for auto-rollback
    pub failure_criteria: FailureCriteria,
    
    /// Experiment timeout
    pub experiment_timeout: Duration,
    
    /// Maximum concurrent experiments per agent
    pub max_concurrent_experiments: usize,
    
    /// Worktree scaling configuration
    pub worktree_scaling: WorktreeScalingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriteria {
    pub min_success_rate: f64,
    pub max_error_rate: f64,
    pub max_response_time: Duration,
    pub min_improvement: f64,  // Minimum improvement over baseline
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCriteria {
    pub max_error_rate: f64,
    pub min_success_rate: f64,
    pub max_response_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeScalingConfig {
    pub enabled: bool,
    pub min_worktrees: usize,
    pub max_worktrees: usize,
    pub target_load: f64,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
}
```

**Example Configuration:**

```toml
[supervisor.versioning]
auto_branching_enabled = true
auto_ab_testing_enabled = true
auto_rollback_enabled = true
auto_merge_enabled = true
auto_scaling_enabled = true

[supervisor.versioning.success_criteria]
min_success_rate = 0.9
max_error_rate = 0.01
max_response_time = "5s"
min_improvement = 0.1

[supervisor.versioning.failure_criteria]
max_error_rate = 0.1
min_success_rate = 0.5
max_response_time = "10s"

[supervisor.versioning.worktree_scaling]
enabled = true
min_worktrees = 1
max_worktrees = 10
target_load = 0.8
scale_up_threshold = 0.9
scale_down_threshold = 0.5
```

---

## Use Cases

### Use-Case 1: Self-Optimizing Customer Support Agent

```rust
// Supervisor automatically optimizes support agent
let supervisor = VersioningSupervisor::new(config).await?;

// Enable continuous experimentation
supervisor.enable_continuous_experimentation("support_agent").await?;

// Supervisor will:
// 1. Monitor agent performance
// 2. Create experiment branches when performance plateaus
// 3. Test variants in parallel using worktrees
// 4. Merge winners automatically
// 5. Rollback failures automatically
// 6. Scale worktrees based on load
```

**Result**: Agent continuously improves without manual intervention.

### Use-Case 2: Multi-Tenant SaaS with A/B Testing

```rust
// Each tenant gets A/B tested automatically
for tenant_id in tenants {
    supervisor.auto_ab_test(
        &format!("agent_{}", tenant_id),
        vec![
            ExperimentVariant {
                name: "strategy_a".to_string(),
                description: "Aggressive upselling".to_string(),
                changes: vec![],
            },
            ExperimentVariant {
                name: "strategy_b".to_string(),
                description: "Conservative approach".to_string(),
                changes: vec![],
            },
        ],
    ).await?;
}

// Supervisor automatically:
// - Tests both strategies in parallel
// - Selects winner based on conversion rate
// - Merges winner to main
// - Scales worktrees for high-traffic tenants
```

**Result**: Each tenant gets optimized automatically.

### Use-Case 3: Cost-Optimized Experimentation

```rust
// Only experiment during low-traffic periods
supervisor.set_experimentation_schedule(
    Schedule::LowTrafficOnly {
        max_concurrent_experiments: 2,
    },
).await?;

// Supervisor will:
// - Only create experiments when traffic is low
// - Use worktrees to test multiple variants efficiently
// - Automatically clean up failed experiments
// - Scale down worktrees when not needed
```

**Result**: Experiments run efficiently without impacting production.

---

## Integration with Existing Supervisor

The versioning supervisor extends the existing supervisor:

```rust
// Existing supervisor functionality
let base_supervisor = ProcessSupervisor::new(base_config).await?;

// Add versioning capabilities
let versioning_supervisor = VersioningSupervisor::new(
    base_supervisor,
    memory_repo,
    versioning_config,
).await?;

// Use versioning supervisor instead of base supervisor
let lifecycle = AgentLifecycleManager::new(
    versioning_supervisor.clone(),
    relevance_evaluator,
    thresholds,
);

// Versioning supervisor implements AgentSupervisor trait
// So it's a drop-in replacement
```

---

## Metrics and Monitoring

```rust
/// Metrics for versioning operations
pub struct VersioningMetrics {
    /// Branches created
    pub branches_created: u64,
    
    /// Branches merged
    pub branches_merged: u64,
    
    /// Branches rolled back
    pub branches_rolled_back: u64,
    
    /// Worktrees created
    pub worktrees_created: u64,
    
    /// A/B tests run
    pub ab_tests_run: u64,
    
    /// Average experiment duration
    pub avg_experiment_duration: Duration,
    
    /// Success rate of experiments
    pub experiment_success_rate: f64,
    
    /// Average improvement from experiments
    pub avg_improvement: f64,
}
```

---

## Benefits

1. **Self-Optimizing Agents**: Agents improve automatically without manual intervention
2. **Risk-Free Experimentation**: Experiments run in isolated branches/worktrees
3. **Automatic Learning**: Supervisor learns what works and applies it
4. **Resource Efficiency**: Worktrees enable efficient parallel testing
5. **Production Safety**: Automatic rollback prevents bad changes from affecting production
6. **Scalability**: Automatic scaling with worktrees

---

## Implementation Plan

### Phase 1: Basic Integration
- [ ] VersioningSupervisor struct
- [ ] Basic branch creation
- [ ] Basic worktree management
- [ ] Integration with existing supervisor

### Phase 2: Auto-Decisions
- [ ] Auto-decision engine
- [ ] Success/failure criteria
- [ ] Automatic rollback
- [ ] Automatic merge

### Phase 3: Advanced Features
- [ ] A/B testing automation
- [ ] Worktree scaling
- [ ] Continuous experimentation loop
- [ ] LLM-powered variant generation

### Phase 4: Production Hardening
- [ ] Metrics and monitoring
- [ ] Error handling
- [ ] Performance optimization
- [ ] Documentation

---

## Conclusion

With Locai memory versioning, the supervisor can become **intelligent and autonomous**:

- **Automatically experiments** to improve agents
- **Automatically scales** using worktrees
- **Automatically learns** what works
- **Automatically optimizes** without manual intervention

This enables **self-managing agent systems** that continuously improve themselves.

**"The supervisor doesn't just manage agents - it makes them better."**



