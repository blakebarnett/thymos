//! Tool system for agent capabilities
//!
//! This module provides a safe, policy-controlled tool execution layer for Thymos agents.
//! Key features:
//! - Deny-by-default capability enforcement
//! - Timeout and cancellation support
//! - Rate limiting and concurrency control
//! - Structured result envelopes with provenance
//!
//! # Example
//!
//! ```rust,no_run
//! use thymos_core::tools::{Tool, ToolContext, ToolRuntime, CapabilityPolicy};
//!
//! // Create a runtime with default (restrictive) policy
//! let runtime = ToolRuntime::new(CapabilityPolicy::deny_all());
//!
//! // Execute a tool with safety enforcement
//! let result = runtime.execute(&my_tool, args, &context).await?;
//! ```

mod capability;
mod registry;
mod result;
mod runtime;
mod tool;

pub use capability::{Capability, CapabilityPolicy, CapabilitySet};
pub use registry::{
    DiscoveryResult, DiscoveryStrategy, McpToolInfo, RegistryError, SubstringDiscovery,
    ToolRegistry, ToolSummary,
};
pub use result::{
    PolicyDecision, ToolError, ToolErrorKind, ToolProvenance, ToolResult, ToolResultEnvelope,
    ToolWarning, ValidationError,
};
pub use runtime::{RateLimitConfig, ToolContext, ToolRuntime, ToolRuntimeConfig};
pub use tool::{BoxedTool, Tool, ToolExample, ToolExecutionContext, ToolHandler, ToolMetadata, ToolSchema};

#[cfg(test)]
mod tests;

