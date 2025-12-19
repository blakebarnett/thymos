# Thymos Go Bindings

Production-ready Go bindings for the Thymos agent framework, built with CGO and Rust FFI.

Thymos (Θυμός) is a domain-agnostic agent framework providing:
- Semantic memory via embedded Locai
- Memory lifecycle management with forgetting curves
- Concept extraction and entity tracking
- Multi-agent coordination

## Features

- ✅ Full agent lifecycle management
- ✅ Memory operations (remember, search, get)
- ✅ Memory types (general, fact, conversation)
- ✅ Hybrid memory mode (private/shared)
- ✅ Agent state and status management
- ✅ Configuration from file/environment
- ✅ Thread-safe operations
- ✅ Go finalizers for safety
- ✅ Auto-generated C headers via cbindgen

## Prerequisites

- **Rust** toolchain 1.90.0+ ([rustup.rs](https://rustup.rs/))
- **Go** 1.21+
- **C compiler** (gcc/clang)
- **pkg-config**

## Installation

### 1. Build the Rust Library

```bash
cd thymos-go
./build.sh
```

This creates `target/release/libthymos_go.so` (or `.dylib` on macOS).

### 2. Use in Your Go Project

```go
import thymos "github.com/blakebarnett/thymos-go"
```

Set the library path for runtime:

```bash
export LD_LIBRARY_PATH="/path/to/thymos/target/release:$LD_LIBRARY_PATH"
```

## Quick Start

```go
package main

import (
    "fmt"
    "log"
    
    thymos "github.com/blakebarnett/thymos-go"
)

func main() {
    // Create an agent with default config
    agent, err := thymos.NewAgent("my_agent")
    if err != nil {
        log.Fatal(err)
    }
    defer agent.Close()

    // Store memories
    id, _ := agent.Remember("Alice met Bob in Paris")
    fmt.Printf("Stored memory: %s\n", id)

    // Store facts (durable knowledge)
    agent.RememberFact("Paris is the capital of France")

    // Store conversations (dialogue context)
    agent.RememberConversation("User asked about travel")

    // Search memories
    results, _ := agent.SearchMemories("Paris", 10)
    for _, mem := range results {
        fmt.Printf("Found: %s\n", mem.Content)
    }

    // Get agent status
    status, _ := agent.Status()
    fmt.Printf("Status: %s\n", status)
}
```

## Configuration

### Default Configuration

```go
agent, err := thymos.NewAgent("my_agent")
```

### Custom Memory Directory

```go
config, err := thymos.NewMemoryConfigWithDataDir("/path/to/data")
if err != nil {
    log.Fatal(err)
}
defer config.Close()

agent, err := thymos.NewAgentWithMemoryConfig("my_agent", config)
```

### Load from File

```go
// Loads thymos.toml, thymos.yaml, or thymos.json from standard locations
// Environment variables with THYMOS_ prefix override file settings
config, err := thymos.LoadConfig()
if err != nil {
    log.Fatal(err)
}
defer config.Close()

agent, err := thymos.NewAgentWithConfig("my_agent", config)
```

### Load from Specific File

```go
config, err := thymos.LoadConfigFromFile("/path/to/config.toml")
```

## API Reference

### Agent Creation

| Function | Description |
|----------|-------------|
| `NewAgent(id)` | Create with default config |
| `NewAgentWithMemoryConfig(id, config)` | Create with custom memory config |
| `NewAgentWithConfig(id, config)` | Create with full Thymos config |
| `agent.Close()` | Release agent resources |

### Memory Operations

| Function | Description |
|----------|-------------|
| `Remember(content)` | Store a general memory |
| `RememberFact(content)` | Store durable knowledge |
| `RememberConversation(content)` | Store dialogue context |
| `RememberPrivate(content)` | Store in private backend (hybrid mode) |
| `RememberShared(content)` | Store in shared backend (hybrid mode) |

### Memory Search

| Function | Description |
|----------|-------------|
| `SearchMemories(query, limit)` | Search all memories |
| `SearchPrivate(query, limit)` | Search private memories (hybrid mode) |
| `SearchShared(query, limit)` | Search shared memories (hybrid mode) |
| `GetMemory(id)` | Get memory by ID |

### Agent State

| Function | Description |
|----------|-------------|
| `ID()` | Get agent ID |
| `Description()` | Get agent description |
| `Status()` | Get current status |
| `SetStatus(status)` | Set status (Active, Listening, Dormant, Archived) |
| `State()` | Get full agent state |
| `IsHybrid()` | Check if using hybrid memory mode |

### Configuration

| Function | Description |
|----------|-------------|
| `NewMemoryConfig()` | Create default memory config |
| `NewMemoryConfigWithDataDir(path)` | Create with custom data directory |
| `NewConfig()` | Create default Thymos config |
| `LoadConfig()` | Load from file/environment |
| `LoadConfigFromFile(path)` | Load from specific file |

### Utilities

| Function | Description |
|----------|-------------|
| `Version()` | Get Thymos library version |

## Memory Types

### Memory

```go
type Memory struct {
    ID           string
    Content      string
    Properties   map[string]interface{}
    CreatedAt    string
    LastAccessed *string
}
```

### State

```go
type State struct {
    Status     Status  // Active, Listening, Dormant, Archived
    StartedAt  *string
    LastActive string
    Properties map[string]interface{}
}
```

### Status Constants

```go
const (
    StatusActive    Status = "Active"
    StatusListening Status = "Listening"
    StatusDormant   Status = "Dormant"
    StatusArchived  Status = "Archived"
)
```

## Error Handling

```go
// Sentinel errors
thymos.ErrNilHandle     // Agent is closed
thymos.ErrNotHybridMode // Hybrid-only operation on non-hybrid agent

// Check for specific errors
_, err := agent.RememberPrivate("test")
if err == thymos.ErrNotHybridMode {
    // Handle non-hybrid mode
}
```

## Thread Safety

All operations are thread-safe. You can safely use a single agent from multiple
goroutines:

```go
agent, _ := thymos.NewAgent("shared_agent")
defer agent.Close()

var wg sync.WaitGroup
for i := 0; i < 10; i++ {
    wg.Add(1)
    go func(n int) {
        defer wg.Done()
        agent.Remember(fmt.Sprintf("Memory from goroutine %d", n))
    }(i)
}
wg.Wait()
```

## Memory Management

While Go finalizers provide a safety net, always close resources explicitly:

```go
agent, _ := thymos.NewAgent("my_agent")
defer agent.Close()  // Always do this!

config, _ := thymos.LoadConfig()
defer config.Close()  // Always do this!
```

## Examples

See the [example](go/example/main.go) for comprehensive usage.

Run the example:

```bash
./run_example.sh
```

## Building from Source

### Build Rust Library

```bash
cargo build --release --package thymos-go
```

### Generate C Headers

Headers are auto-generated during build via cbindgen:

```bash
cargo build --package thymos-go
# Headers output to: include/thymos.h
```

### Run Tests

```bash
# Rust tests
cargo test --package thymos-go

# Go example
./run_example.sh
```

## Troubleshooting

### Library Not Found

```bash
export LD_LIBRARY_PATH="/path/to/thymos/target/release:$LD_LIBRARY_PATH"
```

### CGO Linking Errors

Ensure the Rust library is built and paths are correct:

```bash
./build.sh  # Builds the Rust library
```

### Memory/TLS Issues

See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for jemalloc and TLS allocation issues.

## Architecture

```
┌─────────────────┐
│   Go Application │
├─────────────────┤
│  thymos.go      │  ← Go wrapper with idiomatic API
├─────────────────┤
│  CGO FFI        │  ← C function calls
├─────────────────┤
│  lib.rs         │  ← Rust FFI layer (extern "C")
├─────────────────┤
│  thymos-core    │  ← Thymos Rust library
├─────────────────┤
│  Locai          │  ← Semantic memory engine
└─────────────────┘
```

## Roadmap

- [x] Core agent operations
- [x] Memory operations (remember, search, get)
- [x] Memory types (fact, conversation)
- [x] Hybrid memory mode
- [x] Configuration loading
- [x] Agent state management
- [x] Auto-generated headers (cbindgen)
- [ ] Embedding provider integration
- [ ] LLM provider integration
- [ ] Concept extraction
- [ ] Pub/sub coordination
- [ ] Tool registry

## License

MIT OR Apache-2.0
