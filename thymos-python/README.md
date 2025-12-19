# Thymos Python Bindings

Python bindings for the Thymos agent framework, built with PyO3.

## Status

**Phase 1 Complete**: Core bindings for agent creation, memory operations, and configuration.

## Features

- ✅ Agent creation and configuration
- ✅ Memory operations (remember, search, get_memory)
- ✅ Agent state management
- ✅ Configuration loading
- ✅ Pythonic API design

## Installation

### Prerequisites

- Rust toolchain (install from https://rustup.rs/)
- Python 3.8+ with development headers
- maturin (for building Python extensions)

### Building from Source

**Note**: Python bindings cannot be built directly with `cargo build` because they require Python development libraries. Use `maturin` instead, which handles Python detection and linking automatically.

**Important**: The first build will take several minutes because it compiles RocksDB (a large C++ dependency). This is normal! Subsequent builds will be much faster.

```bash
# Install maturin if you haven't already
pip install maturin
# Or with uv:
uv pip install maturin

# Navigate to the Python bindings directory
cd thymos-python

# Build and install in development mode (debug build - faster compilation)
maturin develop
# Or with uv:
uv run maturin develop

# For release builds (slower compilation but optimized):
maturin develop --release
# Or:
uv run maturin develop --release
```

### Alternative: Install Python Development Headers

If you want to use `cargo build` directly (not recommended), you need Python development headers:

```bash
# On Ubuntu/Debian
sudo apt-get install python3-dev

# On Fedora/RHEL
sudo dnf install python3-devel

# On macOS (with Homebrew)
brew install python3
```

## Usage

```python
import thymos

# Create an agent
agent = thymos.Agent("my_agent")

# Store memories
memory_id = agent.remember("Alice met Bob in Paris")

# Search memories
results = agent.search_memories("Alice", limit=10)
for memory in results:
    print(memory.content())

# Get agent state
state = agent.state()
print(f"Status: {state.status()}")
```

## API Reference

### Agent

- `Agent(agent_id: str)` - Create a new agent
- `remember(content: str) -> str` - Store a memory, returns memory ID
- `search_memories(query: str, limit: Optional[int] = None) -> List[Memory]` - Search memories
- `get_memory(memory_id: str) -> Optional[Memory]` - Get a memory by ID
- `state() -> AgentState` - Get current agent state
- `status() -> str` - Get current agent status
- `set_status(status: str)` - Set agent status (active, listening, dormant, archived)

### Memory

- `id() -> str` - Memory ID
- `content() -> str` - Memory content
- `properties() -> dict` - Memory properties/metadata
- `created_at() -> str` - Creation timestamp (RFC3339)
- `last_accessed() -> Optional[str]` - Last access timestamp
- `to_dict() -> dict` - Convert to Python dictionary

### Configuration

- `ThymosConfig.load()` - Load configuration from file and environment
- `ThymosConfig.from_file(path: str)` - Load configuration from specific file

## Examples

See `examples/basic_example.py` for a complete example.

## Development

### Building

**Important**: Python bindings must be built with `maturin`, not `cargo build` directly, because they require Python development libraries.

```bash
# Install maturin
pip install maturin

# Build in development mode (installs to current Python environment)
cd thymos-python
maturin develop

# Or build a wheel
maturin build --release
```

### Testing

```bash
# Run Rust tests (checking only, won't link Python)
cargo check --package thymos-python

# After building with maturin, test in Python
python -c "import thymos; print('Thymos Python bindings loaded successfully!')"

# Run the example
python examples/basic_example.py
```

### Troubleshooting

**Linking errors with `cargo build`**: This is expected. Python bindings require Python development libraries. Use `maturin` instead, which handles this automatically.

**Python version mismatch**: Ensure maturin detects the correct Python version:
```bash
maturin develop --python python3.12  # Specify version if needed
```

**TLS allocation error ("cannot allocate memory in static TLS block")**: This is a known issue with jemalloc (used by SurrealDB-core) in Python extensions.

**Root Cause**: SurrealDB-core enables jemalloc via its `allocator` feature, which uses Thread Local Storage (TLS) that conflicts with Python's limited TLS space. Unfortunately, **jemalloc cannot be disabled at build time** because:
- SurrealDB-core's `allocator` feature enables `tikv-jemallocator` (hard dependency)
- Locai requires the `allocator` feature for SurrealDB integration
- Even avoiding RocksDB doesn't help - jemalloc comes from SurrealDB-core, not RocksDB

**Current Workaround** (must be set before Python starts):
```bash
export MALLOC_CONF='background_thread:false'
python your_script.py
```

Or use a wrapper script:
```bash
#!/bin/bash
export MALLOC_CONF='background_thread:false'
exec python "$@"
```

**Why build-time fixes don't work**:
- SurrealDB-core's `allocator` feature enables jemalloc regardless of storage backend
- Locai requires `surrealdb-embedded` which includes the `allocator` feature
- Cargo patches can't remove hard dependencies, only replace versions
- Avoiding RocksDB (`kv-rocksdb`) doesn't remove jemalloc - it comes from SurrealDB-core

**Long-term Solutions**:
1. **Use Thymos as a server**: Run Thymos as a separate process and communicate via HTTP/gRPC API
2. **Fork SurrealDB-core**: Create a version without jemalloc (maintenance burden)
3. **Contribute upstream**: Work with SurrealDB to make jemalloc optional via feature flags
4. **Alternative storage**: Use a storage backend that doesn't require SurrealDB

**Note**: We've added `surrealdb-embedded-mem-only` feature to Locai to avoid RocksDB, but jemalloc still comes from SurrealDB-core's allocator.

## Roadmap

- [ ] Phase 2: LLM Integration
- [ ] Phase 3: Advanced Features (embeddings, hybrid search, events)
- [ ] Phase 4: Async API support
- [ ] Phase 5: PyPI packaging

## License

Same as Thymos (MIT OR Apache-2.0)

