//! Offline evaluation harness for agent workflows
//!
//! This module provides infrastructure for deterministic evaluation:
//! - Stub tools with predetermined responses
//! - Load deterministic memory snapshots
//! - Run workflows and validate outputs
//! - Compare against golden fixtures
//!
//! # Architecture
//!
//! The eval harness runs workflows in isolation without external dependencies
//! (no network, no real LLM calls). Tools are replaced with stubs that return
//! predetermined responses, enabling reproducible testing.
//!
//! # Example
//!
//! ```rust,no_run
//! use thymos_core::eval::{EvalHarness, Fixture, StubTool};
//!
//! // Load a fixture
//! let fixture = Fixture::load("fixtures/search_workflow.json")?;
//!
//! // Create harness with stubbed tools
//! let harness = EvalHarness::new()
//!     .with_stub_tool(StubTool::from_fixture(&fixture)?);
//!
//! // Run and validate
//! let result = harness.run(&fixture).await?;
//! assert!(result.passed());
//! ```

mod fixture;
mod harness;
mod stub;

pub use fixture::{Fixture, FixtureExpectation, FixtureInput, MemorySnapshot};
pub use harness::{EvalHarness, EvalResult, EvalRunConfig};
pub use stub::{StubRegistry, StubResponse, StubTool};

#[cfg(test)]
mod tests;



