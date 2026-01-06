# Thymos

**Thymos** (Θυμός) is a domain-agnostic agent framework for building autonomous agents with semantic memory, lifecycle management, and multi-agent coordination. Written in Rust with bindings for Python, Go, and WebAssembly.

> *"Locai remembers. Thymos acts."*

Thymos is a companion project to [Locai](https://github.com/blakebarnett/locai), which provides the semantic memory backend.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Getting Started](#getting-started)
4. [Crates](#crates)
5. [Language Bindings](#language-bindings)
6. [Documentation](#documentation)
7. [License](#license)

## Overview

Thymos provides the infrastructure for building intelligent agents that can remember, reason, and coordinate. Core capabilities include:

**Semantic Memory** with embedded Locai for storing and retrieving memories using semantic search, supporting embedded, server, and hybrid memory modes.

**Temporal Awareness** through forgetting curves, recency weighting, and memory consolidation that models how memories fade and strengthen over time.

**Concept Extraction** for domain-agnostic entity identification, significance scoring, alias resolution, and automatic concept promotion.

**Agent Lifecycle** with relevance-based state transitions (active, listening, dormant, archived) and automatic process management.

**Multi-Agent Coordination** via event-driven communication, pub/sub messaging, and A2A (Agent-to-Agent) protocol support.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              Application Layer                      │
│  (Domain-specific: Games, Assistants, Bots, etc.)  │
└─────────────────────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
   ┌────▼────┐   ┌───▼────┐   ┌───▼────┐
   │ Agent 1 │   │Agent 2 │   │Agent 3 │
   └─────────┘   └────────┘   └────────┘
        │             │             │
   ┌────▼─────────────▼─────────────▼───┐
   │        Thymos Framework Core       │
   │  Memory Lifecycle │ Concept Engine │
   │  Event System     │ Consolidation  │
   │  Relevance Eval   │ State Manager  │
   └────────────────────────────────────┘
        │             │             │
   ┌────▼────┐   ┌───▼──────┐  ┌──▼──────┐
   │ Locai   │   │SurrealDB │  │ Event   │
   │Embedded │   │Embedded  │  │ Stream  │
   └─────────┘   └──────────┘  └─────────┘
```

## Getting Started

### Prerequisites

Rust 1.90.0 or later is required. Docker is optional for containerized development.

### Installation

```bash
git clone https://github.com/blakebarnett/thymos.git
cd thymos
make build
make test
```

### Quick Example

```rust
use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let agent = Agent::builder()
        .id("my_agent")
        .build()
        .await?;
    
    agent.remember("The user prefers dark mode").await?;
    let results = agent.search_memories("user preferences").await?;
    
    for memory in results {
        println!("Found: {}", memory.content);
    }
    
    Ok(())
}
```

### Running as a Daemon

```bash
# Start an agent daemon with A2A support
thymos daemon --id my-agent --port 8080

# Or from configuration file
thymos daemon --config agent.toml
```

See [Getting Started](docs/GETTING_STARTED.md) for more detailed examples.

## Crates

| Crate | Description |
|-------|-------------|
| [thymos-core](thymos-core/) | Core framework with memory, lifecycle, events, and concept extraction |
| [thymos-cli](thymos-cli/) | Command-line interface for running agents as daemons |
| [thymos-daemon](thymos-daemon/) | Continuous runtime with scheduling, budgets, and state persistence |
| [thymos-supervisor](thymos-supervisor/) | Optional process supervisor for multi-agent deployments |

## Language Bindings

| Binding | Description |
|---------|-------------|
| [thymos-python](thymos-python/) | Python bindings via PyO3 |
| [thymos-go](thymos-go/) | Go bindings via CGO/FFI |
| [thymos-wasm](thymos-wasm/) | WebAssembly via WASI Component Model |

## Documentation

### Getting Started

The [Getting Started Guide](docs/GETTING_STARTED.md) covers installation, configuration, and your first agent.

### Design Documents

Core architecture and system design are documented in [docs/design/](docs/design/):

| Document | Topic |
|----------|-------|
| [Agent Framework Design](docs/design/AGENT_FRAMEWORK_DESIGN.md) | Core architecture, memory system, lifecycle management |
| [LLM Native Agent Design](docs/design/LLM_NATIVE_AGENT_DESIGN.md) | Workflow patterns and context management |
| [Git-Style Memory Versioning](docs/design/GIT_STYLE_MEMORY_VERSIONING.md) | Branching, commits, and worktrees for memory |
| [Named Memory Scopes](docs/design/NAMED_MEMORY_SCOPES.md) | Scoped memory with configurable decay |
| [Pub/Sub Abstraction](docs/design/PUBSUB_ABSTRACTION_DESIGN.md) | Unified messaging for local and distributed agents |

### Enhancement Proposals

Active proposals and planned features are tracked in [docs/proposals/](docs/proposals/).

### Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and contribution guidelines.

## License

Thymos is dual-licensed under MIT and Apache 2.0.

