//! Unified Tracing & Export
//!
//! Aggregates traces across workflow types with cost estimation and export.
//!
//! # Features
//!
//! - Trace aggregation across nested workflows
//! - Cost estimation using LLMCostCalculator
//! - JSON export for debugging
//! - OpenTelemetry-compatible spans
//! - Trace sampling for high-volume scenarios
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::tracing::{TraceCollector, TraceExporter};
//!
//! let mut collector = TraceCollector::new("my-workflow");
//! collector.add_step_trace(step_trace);
//! collector.add_nested_trace(nested_execution_trace);
//!
//! let report = collector.finalize();
//! let json = TraceExporter::to_json(&report)?;
//! ```

mod aggregator;
mod collector;
mod export;
mod sampling;

pub use aggregator::{AggregatedTrace, TraceAggregator, TraceSummary};
pub use collector::{TraceCollector, TraceReport};
pub use export::{TraceExporter, TraceFormat};
pub use sampling::{SamplingDecision, SamplingStrategy, TraceSampler};
