# Thymos WASM Component

WebAssembly bindings for the Thymos agent framework using the WASI Component Model.

## Overview

This crate compiles Thymos agent functionality to a WASM Component with two modes:

| Mode | Backend | Features |
|------|---------|----------|
| **Server** | Locai via wasi:http | Full semantic search, embeddings, persistence |
| **In-Memory** | Local HashMap | Offline use, testing, keyword search only |

The WASM component mirrors the `MemoryBackend` trait from `thymos-core`, ensuring
API consistency across all Thymos bindings.

## Quick Start

```javascript
import { agent, memory, storage } from './thymos-wasm.js';

// Option 1: Connect to Locai server (full features)
storage.connect("http://localhost:3000", null);

// Option 2: Use in-memory mode (offline)
// (default, no connection needed)

// Create an agent
agent.create("my-agent");

// Store memories
memory.remember("The user prefers dark mode");
memory.rememberTyped("Paris is the capital of France", "fact");

// Search (semantic search in server mode, keyword in memory mode)
const results = memory.search("user preferences", { limit: 5 });
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  WASM Component (296KB)                                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Backend Enum                                               │ │
│  │  ┌─────────────────┐      ┌─────────────────────────────┐  │ │
│  │  │ InMemoryBackend │  OR  │ ServerBackend               │  │ │
│  │  │ (HashMap)       │      │ (wasi:http → Locai)         │  │ │
│  │  └─────────────────┘      └─────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  WIT Interfaces:                                                 │
│  • thymos:agent/agent    (lifecycle)                            │
│  • thymos:agent/memory   (store/search/get/delete)              │
│  • thymos:agent/storage  (connect/save/load)                    │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ wasi:http (when connected)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│  Locai Server                                                    │
│  • Semantic search with BGE-M3 embeddings                       │
│  • SurrealDB persistence                                        │
│  • Multi-agent shared memory                                    │
└─────────────────────────────────────────────────────────────────┘
```

## Building

### Prerequisites

```bash
cargo install cargo-component
rustup target add wasm32-wasip1
```

### Build

```bash
# Via Makefile
make build-wasm

# Direct
cd thymos-wasm && cargo component build --release
```

### Output

- Debug: `target/wasm32-wasip1/debug/thymos_wasm.wasm` (~3MB)
- Release: `target/wasm32-wasip1/release/thymos_wasm.wasm` (~296KB)

## API Reference

### Storage Interface

```wit
/// Connect to a Locai server (enables semantic search)
connect: func(server-url: string, api-key: option<string>) -> result<_, thymos-error>;

/// Disconnect and switch to in-memory mode
disconnect: func() -> result<_, thymos-error>;

/// Check if connected to a server
is-connected: func() -> bool;

/// Save memories to file (in-memory mode only)
save: func(path: string) -> result<u64, thymos-error>;

/// Load memories from file
load: func(path: string) -> result<u64, thymos-error>;
```

### Memory Interface

```wit
/// Store a memory
remember: func(content: string) -> result<memory-id, thymos-error>;

/// Store with type hint (episodic, fact, conversation)
remember-typed: func(content: string, memory-type: memory-type) -> result<memory-id, thymos-error>;

/// Search memories
search: func(query: string, options: option<search-options>) -> result<list<memory>, thymos-error>;

/// Get, delete, count
get: func(id: memory-id) -> result<option<memory>, thymos-error>;
delete: func(id: memory-id) -> result<bool, thymos-error>;
count: func() -> result<u64, thymos-error>;
```

### Agent Interface

```wit
create: func(id: agent-id) -> result<_, thymos-error>;
id: func() -> result<agent-id, thymos-error>;
status: func() -> result<agent-status, thymos-error>;
set-status: func(status: agent-status) -> result<_, thymos-error>;
state: func() -> result<agent-state, thymos-error>;
```

## Usage Examples

### JavaScript (via jco)

```bash
npx jco transpile thymos_wasm.wasm -o thymos-js
```

```javascript
import { storage, memory } from './thymos-js/thymos_wasm.js';

// Connect to server for full features
try {
    storage.connect("http://localhost:3000", null);
    console.log("Connected to Locai server");
} catch (e) {
    console.log("Using offline mode");
}

// Store and search
memory.remember("Meeting with Alice about project X");
const results = memory.search("Alice project", { limit: 5 });
```

### Running with Wasmtime

```bash
# With network access for server mode
wasmtime run \
    --wasm component-model \
    --wasi http \
    thymos_wasm.wasm

# With filesystem for persistence
wasmtime run \
    --wasm component-model \
    --dir=/data \
    thymos_wasm.wasm
```

## Comparison: Server vs In-Memory Mode

| Feature | Server Mode | In-Memory Mode |
|---------|-------------|----------------|
| Search | Semantic (embeddings) | Keyword only |
| Persistence | SurrealDB | File (optional) |
| Multi-agent | Shared memory | Single instance |
| Offline | ❌ | ✅ |
| Setup | Requires Locai server | None |

## Consistency with thymos-core

The WASM component uses the same backend pattern as `thymos-core`:

```rust
// thymos-core
pub trait MemoryBackend {
    async fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String>;
    async fn search(&self, query: &str, options: Option<QueryOptions>) -> Result<Vec<MemoryRecord>>;
    async fn get(&self, id: &str) -> Result<Option<MemoryRecord>>;
    async fn delete(&self, id: &str) -> Result<bool>;
    async fn count(&self) -> Result<u64>;
}
```

Both `ServerMemoryBackend` (in thymos-core) and `ServerBackend` (in thymos-wasm)
implement the same Locai REST API, ensuring consistent behavior.

## License

MIT OR Apache-2.0

