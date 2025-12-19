//! Supervisor trait and implementations

use crate::{AgentMode, Result};
use thymos_core::{
    agent::AgentStatus,
    lifecycle::RelevanceContext,
};
use async_trait::async_trait;

/// Supervisor for managing agent processes
#[async_trait]
pub trait AgentSupervisor: Send + Sync {
    /// Start an agent
    async fn start(
        &self,
        agent_id: &str,
        mode: AgentMode,
        context: &RelevanceContext,
    ) -> Result<AgentHandle>;
    
    /// Stop an agent gracefully
    async fn stop(&self, agent_id: &str, save_state: bool) -> Result<()>;
    
    /// Get current agent status
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus>;
    
    /// Set agent mode (active/passive)
    async fn set_mode(&self, agent_id: &str, mode: AgentMode) -> Result<()>;
    
    /// List all known agents
    async fn list_agents(&self) -> Result<Vec<String>>;
    
    /// Health check on agent
    async fn health_check(&self, agent_id: &str) -> Result<HealthStatus>;
}

/// Handle to a running agent process
#[derive(Debug, Clone)]
pub struct AgentHandle {
    /// Agent ID
    pub agent_id: String,
    /// Process ID
    pub pid: u32,
    /// Port the agent is listening on
    pub port: u16,
}

/// Health status of an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Agent is healthy and responding
    Healthy,
    /// Agent is unhealthy (not responding, errors)
    Unhealthy,
    /// Agent status is unknown
    Unknown,
}

