//! Research Lab: Multi-Agent Research System Showcase
//!
//! Demonstrates Thymos capabilities with real tool integration:
//! - Multi-agent coordination
//! - Supervisor lifecycle management
//! - Hybrid memory (shared + private)
//! - Real tools (browser, LLM)

pub mod tools;
pub mod agents;

pub use tools::{BrowserTool, Tool, ToolResult, WebSearchTool};
pub use agents::{ResearchCoordinator, LiteratureReviewer, WebResearcher, SynthesisAgent};



