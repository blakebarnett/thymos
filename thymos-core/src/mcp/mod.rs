//! Model Context Protocol (MCP) Server Implementation
//!
//! This module provides an MCP server that exposes Thymos agents, tools,
//! and memories to Claude and other MCP-compatible clients.
//!
//! MCP is an open protocol introduced by Anthropic that standardizes
//! LLM ↔ External System connections using JSON-RPC 2.0.
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::mcp::{McpServer, McpServerConfig};
//! use thymos_core::tools::ToolRegistry;
//!
//! let mut registry = ToolRegistry::new();
//! registry.register(my_tool)?;
//!
//! let server = McpServer::builder()
//!     .name("thymos-mcp")
//!     .version("1.0.0")
//!     .with_tools(registry)
//!     .build();
//!
//! // Run with stdio transport
//! server.run_stdio().await?;
//! ```
//!
//! # Protocol Overview
//!
//! MCP uses JSON-RPC 2.0 with the following main methods:
//! - `initialize` / `initialized` - Connection setup
//! - `tools/list` - List available tools
//! - `tools/call` - Call a tool
//! - `resources/list` - List available resources
//! - `resources/read` - Read a resource
//! - `prompts/list` - List available prompts
//! - `prompts/get` - Get a prompt
//!
//! # References
//!
//! - [MCP Specification](https://modelcontextprotocol.io/specification)
//! - [Anthropic Skills Spec](https://github.com/anthropics/skills)

mod protocol;
mod server;
mod transport;
mod resources;

pub use protocol::*;
pub use server::{McpServer, McpServerBuilder, McpServerConfig};
pub use transport::{StdioTransport, Transport};
pub use resources::{MemoryResource, ResourceProvider};
