//! Agent performance metrics collection
//!
//! This module provides comprehensive performance metrics collection for agents,
//! including task performance, response performance, resource usage, quality metrics,
//! and memory performance.

pub mod collector;
#[allow(clippy::module_inception)]
pub mod metrics;
pub mod storage;
pub mod resource;
pub mod cost;

#[cfg(test)]
mod tests;

pub use collector::MetricsCollector;
pub use metrics::{
    AgentMetrics, MemoryCriteria, MemoryPerformanceMetrics, PerformanceCriteria, PerformanceTrend,
    PerformanceWeights, QualityCriteria, QualityPerformanceMetrics, ResourceLimits,
    ResourcePerformanceMetrics, ResponsePerformanceMetrics, TaskCriteria, TaskPerformanceMetrics,
};
pub use storage::{MetricsStorage, InMemoryMetricsStorage};
pub use resource::{ResourceMonitor, StubResourceMonitor};
pub use cost::LLMCostCalculator;

