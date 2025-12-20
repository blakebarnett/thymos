# Design Documents

Architectural design documents for the Thymos agent framework.

## Core Architecture

| Document | Description |
|----------|-------------|
| [AGENT_FRAMEWORK_DESIGN.md](AGENT_FRAMEWORK_DESIGN.md) | Core agent framework with memory, lifecycle, and events |
| [LLM_NATIVE_AGENT_DESIGN.md](LLM_NATIVE_AGENT_DESIGN.md) | Workflow patterns, MCP integration, context management |

## Memory & Versioning

| Document | Description |
|----------|-------------|
| [GIT_STYLE_MEMORY_VERSIONING.md](GIT_STYLE_MEMORY_VERSIONING.md) | Git-like operations (branches, commits, worktrees) |
| [NAMED_MEMORY_SCOPES.md](NAMED_MEMORY_SCOPES.md) | Named scopes with configurable decay and search weights |

## Context & Session

| Document | Description |
|----------|-------------|
| [CONTEXT_MANAGER.md](CONTEXT_MANAGER.md) | High-level context management with grounding and rollback |
| [SUBAGENT_API.md](SUBAGENT_API.md) | Ergonomic subagent spawning with worktree isolation |

## Agent Coordination

| Document | Description |
|----------|-------------|
| [PUBSUB_ABSTRACTION_DESIGN.md](PUBSUB_ABSTRACTION_DESIGN.md) | Unified pub/sub for local and distributed messaging |

## Performance & Supervision

| Document | Description |
|----------|-------------|
| [AGENT_PERFORMANCE_METRICS.md](AGENT_PERFORMANCE_METRICS.md) | Agent performance tracking and metrics |
| [SUPERVISOR_VERSIONING_INTEGRATION.md](SUPERVISOR_VERSIONING_INTEGRATION.md) | Auto-branching, auto-rollback patterns |
