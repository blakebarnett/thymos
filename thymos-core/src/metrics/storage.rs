//! Metrics storage abstraction

use crate::error::Result;
use crate::metrics::AgentMetrics;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Trait for metrics storage backends
#[async_trait]
pub trait MetricsStorage: Send + Sync {
    /// Store metrics for an agent
    async fn store_metrics(&self, metrics: &AgentMetrics) -> Result<()>;

    /// Get latest metrics for an agent
    async fn get_latest_metrics(&self, agent_id: &str) -> Result<Option<AgentMetrics>>;

    /// Get metrics history for an agent
    async fn get_metrics_history(
        &self,
        agent_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AgentMetrics>>;

    /// Get metrics for a time range
    async fn get_metrics_range(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentMetrics>>;

    /// Get baseline metrics (typically from main branch)
    async fn get_baseline_metrics(&self, agent_id: &str) -> Result<Option<AgentMetrics>>;
}

/// In-memory metrics storage implementation
pub struct InMemoryMetricsStorage {
    /// Metrics by agent ID
    metrics: Arc<RwLock<HashMap<String, Vec<AgentMetrics>>>>,
    /// Baseline metrics by agent ID
    baselines: Arc<RwLock<HashMap<String, AgentMetrics>>>,
}

impl InMemoryMetricsStorage {
    /// Create a new in-memory metrics storage
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            baselines: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryMetricsStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetricsStorage for InMemoryMetricsStorage {
    async fn store_metrics(&self, metrics: &AgentMetrics) -> Result<()> {
        let mut storage = self.metrics.write().await;
        let agent_metrics = storage.entry(metrics.agent_id.clone()).or_insert_with(Vec::new);
        agent_metrics.push(metrics.clone());
        
        // Keep only last 1000 metrics per agent
        if agent_metrics.len() > 1000 {
            agent_metrics.remove(0);
        }
        
        Ok(())
    }

    async fn get_latest_metrics(&self, agent_id: &str) -> Result<Option<AgentMetrics>> {
        let storage = self.metrics.read().await;
        Ok(storage
            .get(agent_id)
            .and_then(|metrics| metrics.last().cloned()))
    }

    async fn get_metrics_history(
        &self,
        agent_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AgentMetrics>> {
        let storage = self.metrics.read().await;
        let mut metrics = storage
            .get(agent_id)
            .cloned()
            .unwrap_or_default();
        
        // Reverse to get most recent first
        metrics.reverse();
        
        if let Some(limit) = limit {
            metrics.truncate(limit);
        }
        
        Ok(metrics)
    }

    async fn get_metrics_range(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentMetrics>> {
        let storage = self.metrics.read().await;
        let metrics = storage
            .get(agent_id)
            .cloned()
            .unwrap_or_default();
        
        Ok(metrics
            .into_iter()
            .filter(|m| m.timestamp >= start && m.timestamp <= end)
            .collect())
    }

    async fn get_baseline_metrics(&self, agent_id: &str) -> Result<Option<AgentMetrics>> {
        let baselines = self.baselines.read().await;
        Ok(baselines.get(agent_id).cloned())
    }
}

impl InMemoryMetricsStorage {
    /// Set baseline metrics for an agent
    pub async fn set_baseline_metrics(&self, agent_id: &str, metrics: AgentMetrics) -> Result<()> {
        let mut baselines = self.baselines.write().await;
        baselines.insert(agent_id.to_string(), metrics);
        Ok(())
    }
}



