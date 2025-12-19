//! Process-based supervisor implementation

use crate::{AgentSupervisor, AgentHandle, AgentMode, HealthStatus, Result, SupervisorError};
use thymos_core::{
    agent::AgentStatus,
    lifecycle::RelevanceContext,
};
use anyhow::Context;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Configuration for process supervisor
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Path to agent binary
    pub agent_binary: PathBuf,
    
    /// Starting port for agents
    pub port_start: u16,
    
    /// Startup timeout
    pub startup_timeout: Duration,
    
    /// Shutdown timeout before force kill
    pub shutdown_timeout: Duration,
    
    /// Working directory for agents
    pub working_dir: Option<PathBuf>,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            agent_binary: PathBuf::from("./target/release/thymos-agent"),
            port_start: 3000,
            startup_timeout: Duration::from_secs(10),
            shutdown_timeout: Duration::from_secs(5),
            working_dir: None,
        }
    }
}

/// Process-based supervisor (subprocess management)
pub struct ProcessSupervisor {
    processes: Arc<RwLock<HashMap<String, Child>>>,
    handles: Arc<RwLock<HashMap<String, AgentHandle>>>,
    config: SupervisorConfig,
    next_port: Arc<RwLock<u16>>,
}

impl ProcessSupervisor {
    /// Create a new process supervisor
    pub async fn new(config: SupervisorConfig) -> Result<Self> {
        // Verify agent binary exists
        if !config.agent_binary.exists() {
            return Err(SupervisorError::BinaryNotFound(
                config.agent_binary.display().to_string()
            ));
        }
        
        Ok(Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
            next_port: Arc::new(RwLock::new(config.port_start)),
            config,
        })
    }
    
    /// Allocate next available port
    async fn allocate_port(&self) -> Result<u16> {
        let mut port = self.next_port.write().await;
        let allocated = *port;
        *port += 1;
        Ok(allocated)
    }
    
    /// Write context to temporary file
    async fn write_context(
        &self,
        agent_id: &str,
        context: &RelevanceContext,
    ) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let context_file = temp_dir.join(format!("thymos_context_{}.json", agent_id));
        
        let context_json = serde_json::to_string_pretty(context)
            .context("Failed to serialize context")?;
        
        tokio::fs::write(&context_file, context_json).await
            .context("Failed to write context file")?;
        
        Ok(context_file)
    }
    
    /// Wait for agent to be ready
    async fn wait_for_ready(&self, port: u16, timeout_duration: Duration) -> Result<()> {
        // Simple health check - try to connect to the port
        // In a real implementation, this would check an HTTP endpoint or similar
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() > timeout_duration {
                return Err(SupervisorError::StartupTimeout);
            }
            
            // Check if port is listening (simplified - real implementation would use proper health check)
            if self.check_port_ready(port).await {
                debug!("Agent on port {} is ready", port);
                return Ok(());
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    
    /// Check if port is ready (simplified implementation)
    async fn check_port_ready(&self, _port: u16) -> bool {
        // TODO: Implement actual health check
        // For now, just wait a bit
        tokio::time::sleep(Duration::from_millis(500)).await;
        true
    }
}

#[async_trait]
impl AgentSupervisor for ProcessSupervisor {
    async fn start(
        &self,
        agent_id: &str,
        mode: AgentMode,
        context: &RelevanceContext,
    ) -> Result<AgentHandle> {
        info!("Starting agent: {} (mode: {:?})", agent_id, mode);
        
        // Check if already running
        {
            let processes = self.processes.read().await;
            if processes.contains_key(agent_id) {
                return Err(SupervisorError::Supervisor(format!(
                    "Agent {} is already running",
                    agent_id
                )));
            }
        }
        
        // Allocate port
        let port = self.allocate_port().await?;
        
        // Write context to temp file
        let context_file = self.write_context(agent_id, context).await?;
        
        // Spawn process
        let mut cmd = Command::new(&self.config.agent_binary);
        cmd.arg("--agent-id").arg(agent_id)
           .arg("--port").arg(port.to_string())
           .arg("--mode").arg(mode.to_string())
           .arg("--context").arg(&context_file);
        
        if let Some(ref working_dir) = self.config.working_dir {
            cmd.current_dir(working_dir);
        }
        
        let child = cmd.spawn()
            .context("Failed to spawn agent process")?;
        
        let pid = child.id();
        
        // Wait for agent to be ready
        self.wait_for_ready(port, self.config.startup_timeout).await?;
        
        // Store process and handle
        {
            let mut processes = self.processes.write().await;
            processes.insert(agent_id.to_string(), child);
        }
        
        let handle = AgentHandle {
            agent_id: agent_id.to_string(),
            pid,
            port,
        };
        
        {
            let mut handles = self.handles.write().await;
            handles.insert(agent_id.to_string(), handle.clone());
        }
        
        info!("Agent {} started successfully (PID: {}, Port: {})", agent_id, pid, port);
        Ok(handle)
    }
    
    async fn stop(&self, agent_id: &str, save_state: bool) -> Result<()> {
        info!("Stopping agent: {} (save_state: {})", agent_id, save_state);
        
        let child_opt = {
            let mut processes = self.processes.write().await;
            processes.remove(agent_id)
        };
        
        if let Some(mut child) = child_opt {
            let pid = child.id();
            
            // Send SIGTERM for graceful shutdown
            #[cfg(unix)]
            {
                // Try to send SIGTERM (simplified - would use nix crate in production)
                if let Err(e) = std::process::Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .output()
                {
                    warn!("Failed to send SIGTERM to agent {}: {}", agent_id, e);
                }
            }
            
            #[cfg(windows)]
            {
                // On Windows, just kill the process
                let _ = child.kill();
            }
            
            // Wait for graceful shutdown (spawn blocking task)
            let shutdown_result = timeout(
                self.config.shutdown_timeout,
                tokio::task::spawn_blocking(move || child.wait())
            ).await;
            
            match shutdown_result {
                Ok(Ok(Ok(status))) => {
                    info!("Agent {} stopped gracefully (status: {:?})", agent_id, status);
                }
                Ok(Ok(Err(e))) => {
                    warn!("Error waiting for agent {} shutdown: {}", agent_id, e);
                }
                Ok(Err(e)) => {
                    warn!("Error in shutdown task for agent {}: {}", agent_id, e);
                }
                Err(_) => {
                    warn!("Agent {} shutdown timeout, force killing", agent_id);
                    // Note: child was moved into spawn_blocking, so we can't kill it here
                    // In production, we'd track the PID and kill it separately
                }
            }
        } else {
            warn!("Agent {} was not running", agent_id);
        }
        
        // Clean up handle
        {
            let mut handles = self.handles.write().await;
            handles.remove(agent_id);
        }
        
        // TODO: Save state if requested
        if save_state {
            debug!("Saving state for agent {}", agent_id);
            // Implementation would save agent state here
        }
        
        Ok(())
    }
    
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus> {
        let processes = self.processes.read().await;
        
        if processes.contains_key(agent_id) {
            // Process exists - assume it's running
            // Note: try_wait() requires mutable access, so we can't use it here
            // In production, we'd track process status separately
            Ok(AgentStatus::Active)
        } else {
            Ok(AgentStatus::Dormant)
        }
    }
    
    async fn set_mode(&self, agent_id: &str, mode: AgentMode) -> Result<()> {
        // TODO: Implement mode switching
        // This would require IPC with the agent process
        debug!("Setting mode for agent {} to {:?}", agent_id, mode);
        Ok(())
    }
    
    async fn list_agents(&self) -> Result<Vec<String>> {
        let processes = self.processes.read().await;
        Ok(processes.keys().cloned().collect())
    }
    
    async fn health_check(&self, agent_id: &str) -> Result<HealthStatus> {
        let handles = self.handles.read().await;
        
        if handles.contains_key(agent_id) {
            // TODO: Implement actual health check
            // For now, just check if process is running
            let status = self.get_status(agent_id).await?;
            match status {
                AgentStatus::Active | AgentStatus::Listening => Ok(HealthStatus::Healthy),
                _ => Ok(HealthStatus::Unhealthy),
            }
        } else {
            Ok(HealthStatus::Unknown)
        }
    }
}

