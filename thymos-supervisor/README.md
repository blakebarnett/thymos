# Thymos Supervisor

**Optional process supervisor for production multi-agent deployments**

`thymos-supervisor` provides automatic relevance-based process lifecycle management, bridging Thymos's intelligent agent lifecycle with production-grade process orchestration.

## When to Use Thymos Supervisor

### ✅ Use When You Need:

1. **Cost Optimization** - Don't run agents when idle (multi-tenant SaaS, expensive LLMs)
2. **Resource Constraints** - Can't run all agents at once (limited CPU/memory)
3. **Multi-Tenant Systems** - One agent per customer/tenant (start/stop based on activity)
4. **Dynamic Workloads** - Agents that come and go based on events or context
5. **Production Deployment** - Need process isolation, health checks, and fault tolerance

### ❌ Don't Use For:

- **Simple single-agent applications** - Use in-process agents from `thymos-core`
- **Always-running agents** - Chatbots, assistants that are always active
- **Development/prototyping** - In-process agents are simpler and faster
- **Embedded systems** - Process overhead isn't worth it

## Quick Start

### Installation

```toml
[dependencies]
thymos-core = "0.1"
thymos-supervisor = "0.1"  # Optional - only if you need process management
```

### Basic Usage

```rust
use thymos_supervisor::{ProcessSupervisor, SupervisorConfig};
use thymos_core::{AgentLifecycleManager, RelevanceContext, RelevanceThresholds};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Create supervisor
    let config = SupervisorConfig {
        agent_binary: PathBuf::from("./target/release/my-agent"),
        port_start: 3000,
        startup_timeout: Duration::from_secs(10),
        shutdown_timeout: Duration::from_secs(5),
    };
    
    let supervisor = ProcessSupervisor::new(config).await?;
    
    // Create lifecycle manager with your relevance evaluator
    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        MyRelevanceEvaluator::new(),
        RelevanceThresholds::default(),
    );
    
    // Reconcile agents based on context
    let mut context = RelevanceContext::new();
    context.set("customer_active", true);
    context.set("last_activity_days", 2);
    
    let report = lifecycle.reconcile(&context).await?;
    println!("Started: {:?}, Stopped: {:?}", report.started, report.stopped);
    
    Ok(())
}
```

## Use Cases

### 1. Multi-Tenant SaaS (Cost Optimization)

```rust
// Start agent when customer is active, stop when inactive
struct CustomerRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for CustomerRelevanceEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        let customer_id = agent_id.strip_prefix("customer_").unwrap();
        let last_activity_days: u32 = context.get("last_activity_days").unwrap_or(999);
        let subscription_active: bool = context.get("subscription_active").unwrap_or(false);
        
        let score = if !subscription_active {
            0.0  // Archived - subscription cancelled
        } else if last_activity_days < 1 {
            1.0  // Active - customer used service today
        } else if last_activity_days < 7 {
            0.6  // Listening - recent activity
        } else if last_activity_days < 30 {
            0.2  // Dormant - inactive but keep ready
        } else {
            0.0  // Archived - inactive too long
        };
        
        Ok(RelevanceScore::new(score))
    }
}
```

**Benefit**: Only pay for compute when customers are active. Save costs on inactive customers.

### 2. Cost-Optimized LLM Agents

```rust
// Only run expensive agents when there's work to do
struct ExpensiveLLMRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for ExpensiveLLMRelevanceEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        let queue_length: usize = context.get("pending_requests").unwrap_or(0);
        let cost_per_hour: f64 = context.get("cost_per_hour").unwrap_or(10.0);
        
        let score = if queue_length > 10 {
            1.0  // Active - lots of work
        } else if queue_length > 0 {
            0.5  // Listening - some work
        } else {
            0.0  // Dormant - no work, save costs
        };
        
        Ok(RelevanceScore::new(score))
    }
}
```

**Benefit**: Only pay for expensive LLM compute when processing requests. Stop when idle.

### 3. Resource-Constrained Environments

```rust
// Prioritize most relevant agents when resources are limited
struct ResourceConstrainedEvaluator;

#[async_trait]
impl RelevanceEvaluator for ResourceConstrainedEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        let available_cpu: f64 = context.get("available_cpu").unwrap_or(1.0);
        let agent_priority: f64 = context.get("agent_priority").unwrap_or(0.5);
        
        // Only run if we have resources and agent is high priority
        let score = if available_cpu > 0.5 && agent_priority > 0.7 {
            1.0
        } else if available_cpu > 0.2 && agent_priority > 0.5 {
            0.5
        } else {
            0.0
        };
        
        Ok(RelevanceScore::new(score))
    }
}
```

**Benefit**: Automatically manage resource allocation. Start high-priority agents first.

### 4. Event-Driven Agents

```rust
// Start agents when specific events occur
struct EventDrivenEvaluator;

#[async_trait]
impl RelevanceEvaluator for EventDrivenEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        let pending_events: usize = context.get("pending_events").unwrap_or(0);
        let event_type: String = context.get("event_type").unwrap_or_default();
        
        // Only relevant if there are events of the right type
        let score = if pending_events > 0 && event_type == "order_fulfillment" {
            1.0
        } else {
            0.0
        };
        
        Ok(RelevanceScore::new(score))
    }
}
```

**Benefit**: Agents only run when needed. Automatic activation on events.

## Architecture

```
┌─────────────────────────────────────┐
│   AgentLifecycleManager             │
│   (from thymos-core)                │
│                                     │
│   - Evaluates relevance             │
│   - Decides state transitions       │
│   - Coordinates reconciliation      │
└──────────────┬──────────────────────┘
               │
               │ uses
               ▼
┌─────────────────────────────────────┐
│   AgentSupervisor (trait)          │
│   (from thymos-supervisor)          │
│                                     │
│   - start() / stop()                │
│   - get_status()                    │
│   - health_check()                  │
└──────────────┬──────────────────────┘
               │
               │ implements
               ▼
┌─────────────────────────────────────┐
│   ProcessSupervisor                 │
│   (from thymos-supervisor)           │
│                                     │
│   - Spawns processes                │
│   - Manages ports                    │
│   - Handles state persistence       │
│   - Health monitoring                │
└─────────────────────────────────────┘
```

## Key Features

### Automatic Relevance-Based Lifecycle

The supervisor integrates with Thymos's relevance evaluation to automatically:
- Start agents when they become relevant
- Stop agents when they become dormant
- Handle state persistence automatically
- Coordinate shared memory connections

### Process Isolation

- Fault isolation - one agent crash doesn't affect others
- Resource limits - per-agent memory/CPU limits
- Security boundaries - process-level isolation

### Production Ready

- Health checks and monitoring
- Graceful shutdown with state persistence
- Automatic restart on failure
- Port allocation and management

## Examples

See the `examples/` directory for complete examples:

- `multi_tenant.rs` - Multi-tenant SaaS cost optimization
- `cost_optimization.rs` - Expensive LLM agent cost management

## Relationship to Thymos Core

**`thymos-core`** provides:
- Agent lifecycle management (state transitions)
- Relevance evaluation
- Memory system
- In-process agents (default)

**`thymos-supervisor`** adds:
- Process management (spawning, monitoring)
- Automatic relevance → process lifecycle
- Production deployment features

You can use `thymos-core` without `thymos-supervisor` for simple agents. Add `thymos-supervisor` when you need production-grade process management.

## License

Same as Thymos (MIT OR Apache-2.0)



