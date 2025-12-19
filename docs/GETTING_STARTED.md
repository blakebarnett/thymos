# Getting Started

## Prerequisites

- Rust 1.90.0+
- Docker (optional)

## Installation

```bash
git clone https://github.com/blakebarnett/thymos.git
cd thymos
make build
make test
```

## Your First Agent

```rust
use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let agent = Agent::builder()
        .id("my_agent")
        .build()
        .await?;
    
    agent.remember("Hello, World!").await?;
    let results = agent.search_memories("Hello").await?;
    
    for memory in results {
        println!("Found: {}", memory.content);
    }
    
    Ok(())
}
```

## With LLM Support

```rust
use thymos_core::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Set GROQ_API_KEY environment variable
    #[cfg(feature = "llm-groq")]
    {
        use thymos_core::llm::providers::groq::GroqProvider;
        let llm = GroqProvider::from_env(None::<String>)?;
        
        let agent = Agent::builder()
            .id("llm_agent")
            .llm_provider(Arc::new(llm))
            .build()
            .await?;
    }
    
    Ok(())
}
```

## Memory Modes

**Embedded** (default):
```rust
let config = MemoryConfig {
    mode: MemoryMode::Embedded {
        data_dir: PathBuf::from("./data"),
    },
    ..Default::default()
};
```

**Server**:
```rust
let config = MemoryConfig {
    mode: MemoryMode::Server {
        url: "http://localhost:3000".to_string(),
        api_key: None,
    },
    ..Default::default()
};
```

**Hybrid** (private + shared):
```rust
let config = MemoryConfig {
    mode: MemoryMode::Hybrid {
        private_data_dir: PathBuf::from("./private"),
        shared_url: "http://localhost:3000".to_string(),
        shared_api_key: None,
    },
    ..Default::default()
};
```

## Examples

```bash
cargo run --example simple_agent
cargo run --example batteries_included --all-features
cargo run --example memory_lifecycle
```

## Next Steps

- [Agent Framework Design](design/AGENT_FRAMEWORK_DESIGN.md)
- [Memory Versioning](design/GIT_STYLE_MEMORY_VERSIONING.md)
- [Workflow Patterns](design/LLM_NATIVE_AGENT_DESIGN.md)
