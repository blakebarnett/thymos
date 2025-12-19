# Known Issues - Thymos Go Bindings

## jemalloc / TLS Allocation Issues

### Status: **MITIGATED**

As of the latest update, the TLS allocation issue should be significantly mitigated
by enabling the `disable_initial_exec_tls` feature on `tikv-jemalloc-sys`.

### Background

When loading the Thymos shared library via CGO, you may encounter:

```
fatal error: failed to allocate memory
```

or on Linux:

```
cannot allocate memory in static TLS block
```

### Root Cause

SurrealDB (used by Locai for storage) depends on jemalloc via the dependency chain:
`surrealdb-core` → `tikv-jemallocator` → `tikv-jemalloc-sys`

jemalloc's default `initial-exec` TLS model conflicts with dynamic loading in CGO.

### Applied Fix

The Go bindings now explicitly enable `disable_initial_exec_tls`:

```toml
# In thymos-go/Cargo.toml
tikv-jemalloc-sys = { version = "0.6", features = ["disable_initial_exec_tls"] }
```

This builds jemalloc with `--disable-initial-exec-tls`, allowing it to be
dynamically loaded after program startup.

### If You Still Experience Issues

1. **Set LD_PRELOAD** (Linux only):
   ```bash
   export LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libjemalloc.so.2
   ```

2. **Use environment tuning**:
   ```bash
   export MALLOC_CONF="narenas:1,tcache:false"
   ```

3. **Connect to a Locai server** instead of using embedded mode:
   Configure your agent to connect to a remote Locai server, which runs in a
   separate process and avoids the CGO/jemalloc conflict entirely.

## Alternative Approaches Considered

During research, we evaluated several alternative approaches:

### 1. purego + libffi (CGO-free)
Apache OpenDAL uses this approach to eliminate CGO entirely.
- Pros: No CGO complexity, no TLS issues
- Cons: Requires libffi runtime dependency, more complex setup
- Status: Considered for future if current fix proves insufficient

### 2. UniFFI Go Bindings
Mozilla's UniFFI with third-party Go generator (uniffi-bindgen-go by NordSecurity).
- Pros: Type-safe, auto-generated interfaces
- Cons: Experimental for Go, additional tooling
- Status: Monitoring for maturity

### 3. WebAssembly (Wazero)
Compile Rust to WASM, run in pure Go via Wazero.
- Pros: Zero CGO, maximum portability
- Cons: Performance overhead, API limitations
- Status: Not suitable for current feature set

### 4. Disable `allocator` Feature in SurrealDB
SurrealDB has an `allocator` feature that can be disabled.
- Pros: Removes jemalloc entirely
- Cons: Requires changes to locai dependency
- Status: Possible if TLS fix proves insufficient

## CGO Build Requirements

### Prerequisites

Building the Go bindings requires:

- Rust toolchain (1.90.0+)
- Go 1.21+
- C compiler (gcc/clang)
- pkg-config

### Library Path Issues

If you encounter "library not found" errors at runtime:

```bash
# Set library path
export LD_LIBRARY_PATH="$WORKSPACE_ROOT/target/release:$LD_LIBRARY_PATH"

# Or use the helper script
./run_example.sh
```

## Memory Management

### Finalizers

Go finalizers are registered on `Agent`, `Config`, and `MemoryConfig` objects
as a safety net. However, you should always call `Close()` explicitly:

```go
agent, _ := thymos.NewAgent("my_agent")
defer agent.Close()  // Always do this
```

### Thread Safety

All exported functions are thread-safe. The Go wrappers use `sync.RWMutex` to
protect against concurrent access to closed handles.

## Hybrid Mode Limitations

When not in hybrid mode, the following operations will return
`ErrNotHybridMode`:

- `RememberPrivate()`
- `RememberShared()`
- `SearchPrivate()`
- `SearchShared()`

## Platform-Specific Notes

### Linux

- Tested on Ubuntu 20.04+, Debian 11+
- Requires `libc6-dev` and `libdl-dev`

### macOS

- Tested on macOS 12+ (Intel and Apple Silicon)
- May require `brew install pkg-config`

### Windows

- Not officially supported
- CGO on Windows has additional complexity
- Consider using WSL2 for Windows development

## Reporting Issues

When reporting issues, please include:

1. Go version (`go version`)
2. Rust version (`rustc --version`)
3. Operating system and version
4. Full error message/stack trace
5. Minimal reproduction code

File issues at: https://github.com/blakebarnett/thymos/issues

