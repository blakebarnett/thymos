//! Thymos Supervisor - Optional process supervisor for production multi-agent deployments
//!
//! Provides automatic relevance-based process lifecycle management, bridging Thymos's
//! intelligent agent lifecycle with production-grade process orchestration.
//!
//! ## When to Use
//!
//! Use `thymos-supervisor` when you need:
//! - **Cost optimization** - Don't run agents when idle
//! - **Resource constraints** - Can't run all agents at once
//! - **Multi-tenant systems** - One agent per customer/tenant
//! - **Dynamic workloads** - Agents that come and go based on context
//! - **Production deployment** - Process isolation and fault tolerance
//!
//! Don't use for simple single-agent applications - use in-process agents from `thymos-core` instead.

mod supervisor;
mod process;
mod lifecycle;
mod error;
mod versioning;

pub use supervisor::{AgentSupervisor, AgentHandle, HealthStatus};
pub use process::{ProcessSupervisor, SupervisorConfig};
pub use lifecycle::AgentLifecycleManager;
pub use error::{Result, SupervisorError};
pub use versioning::{VersioningSupervisor, VersioningSupervisorConfig, AutoDecisionEngine, DecisionAction, DecisionCondition, DecisionRule};

use serde::{Deserialize, Serialize};

/// Agent mode (active or passive)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    /// Actively responding to interactions
    Active,
    /// Listening to events, updating state, but not responding
    Passive,
}

impl std::fmt::Display for AgentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentMode::Active => write!(f, "active"),
            AgentMode::Passive => write!(f, "passive"),
        }
    }
}

/// Report of reconciliation actions
#[derive(Debug, Default, Clone)]
pub struct ReconciliationReport {
    /// Agents that were started
    pub started: Vec<String>,
    /// Agents that were stopped
    pub stopped: Vec<String>,
    /// Agents that were upgraded (Listening → Active)
    pub upgraded: Vec<String>,
    /// Agents that were downgraded (Active → Listening)
    pub downgraded: Vec<String>,
}

