//! Resource monitoring for system metrics

use crate::error::Result;
use async_trait::async_trait;

/// Trait for resource monitoring
#[async_trait]
pub trait ResourceMonitor: Send + Sync {
    /// Get current CPU usage (0.0-1.0)
    async fn get_cpu_usage(&self) -> Result<f64>;

    /// Get current memory usage in bytes
    async fn get_memory_usage(&self) -> Result<usize>;

    /// Get peak memory usage in bytes
    async fn get_peak_memory_usage(&self) -> Result<usize>;

    /// Get memory usage percentage (0.0-1.0)
    async fn get_memory_usage_percent(&self) -> Result<f64>;
}

// Note: SysinfoResourceMonitor can be added later as an optional feature
// For now, use StubResourceMonitor or implement a custom ResourceMonitor

/// Stub resource monitor (returns zeros when sysinfo is not available)
pub struct StubResourceMonitor;

#[async_trait]
impl ResourceMonitor for StubResourceMonitor {
    async fn get_cpu_usage(&self) -> Result<f64> {
        Ok(0.0)
    }

    async fn get_memory_usage(&self) -> Result<usize> {
        Ok(0)
    }

    async fn get_peak_memory_usage(&self) -> Result<usize> {
        Ok(0)
    }

    async fn get_memory_usage_percent(&self) -> Result<f64> {
        Ok(0.0)
    }
}

