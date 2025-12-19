# Thymos

**Domain-agnostic agent framework with semantic memory, versioning, and multi-agent coordination.**

Thymos provides lifecycle management, memory versioning, and workflow orchestration for AI agents. Built in Rust as a companion to [Locai](https://github.com/blakebarnett/locai).

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Features](#features)
- [Architecture](#architecture)
- [Documentation](#documentation)
- [Development](#development)
- [License](#license)

## Installation

```toml
[dependencies]
thymos-core = { version = "0.1.0", features = ["llm-groq", "embeddings-local"] }
```

**Feature flags:**
- `llm-groq` / `llm-openai` / `llm-anthropic` - LLM providers
- `embeddings-local` - Local embeddings (no API needed)
- `pubsub-distributed` - Distributed messaging with SurrealDB

## Quick Start

```rust
use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let agent = Agent::builder()
        .id("my_agent")
        .build()
        .await?;
    
    agent.remember("Alice met Bob in Paris").await?;
    let memories = agent.search_memories("Alice").await?;
    
    Ok(())
}
```

See [examples/](thymos-core/examples/) for more complete examples.

## Features

### Core

| Feature | Description |
|---------|-------------|
| **Agent Framework** | Lifecycle management, relevance evaluation, event hooks |
| **Semantic Memory** | BM25 + vector search via Locai, temporal decay |
| **Memory Versioning** | Git-style branches, commits, worktrees for agent memory |
| **Concept Extraction** | Entity tracking, aliases, promotion pipelines |

### Workflows

| Pattern | Description |
|---------|-------------|
| **Chain** | Sequential steps with gates and data flow |
| **Router** | Classification-based routing to handlers |
| **Parallel** | Concurrent execution with aggregation |
| **Orchestrator** | Task decomposition and delegation |
| **Evaluator-Optimizer** | Iterative refinement loops |

### Advanced Patterns

| Pattern | Description |
|---------|-------------|
| **Speculative Execution** | Try multiple approaches, commit best result |
| **Parallel Isolation** | Worktree-based memory isolation |
| **Consensus Merge** | Multi-agent voting and LLM-assisted synthesis |
| **Bisect Regression** | Binary search debugging through memory history |

### Infrastructure

| Component | Description |
|-----------|-------------|
| **LLM Providers** | Groq, OpenAI, Anthropic, Ollama |
| **MCP Server** | Model Context Protocol with tools, resources, prompts |
| **Pub/Sub** | Local, distributed (SurrealDB), and hybrid modes |
| **Tracing** | Execution traces, cost estimation, OpenTelemetry export |

## Architecture

```
thymos/
├── thymos-core/        # Core framework
├── thymos-supervisor/  # Production process management (optional)
├── thymos-cli/         # Command-line tools
├── thymos-go/          # Go bindings
├── thymos-python/      # Python bindings
├── thymos-wasm/        # WebAssembly bindings
└── docs/               # Documentation
```

### Memory Modes

- **Embedded** - Local storage (default)
- **Server** - Remote Locai server
- **Hybrid** - Private embedded + shared server

### Comparison to Locai

| | Locai | Thymos |
|---|-------|--------|
| Purpose | Memory storage | Agent lifecycle |
| Focus | Search, embeddings | Versioning, coordination |
| Usage | Library/service | Framework (uses Locai) |

## Documentation

| Document | Description |
|----------|-------------|
| [Getting Started](docs/GETTING_STARTED.md) | Setup and first agent |
| [Agent Framework](docs/design/AGENT_FRAMEWORK_DESIGN.md) | Core architecture |
| [Memory Versioning](docs/design/GIT_STYLE_MEMORY_VERSIONING.md) | Git-style operations |
| [LLM-Native Design](docs/design/LLM_NATIVE_AGENT_DESIGN.md) | Workflow patterns |
| [Pub/Sub System](docs/design/PUBSUB_ABSTRACTION_DESIGN.md) | Agent coordination |

See [docs/design/README.md](docs/design/README.md) for complete documentation index.

## Development

### Prerequisites

- Rust 1.90.0+
- Docker (optional)

### Commands

```bash
make build      # Build all crates
make test       # Run tests
make check      # Lint and format
```

### Examples

```bash
cargo run --example batteries_included --all-features
cargo run --example simple_agent
cargo run --example memory_lifecycle
```

## License

MIT or Apache-2.0 (dual license)

## Acknowledgments

- [Locai](https://github.com/blakebarnett/locai) - Semantic memory
- [SurrealDB](https://surrealdb.com/) - Distributed pub/sub
- [Tokio](https://tokio.rs/) - Async runtime
