# Known Issues - Thymos WASM Component

## Current Limitations

### 1. Simple Storage Format (No SurrealDB)

**Status:** By Design

The WASM component uses an in-memory store with JSON file persistence rather than SurrealDB because:

- SurrealDB has native dependencies (RocksDB, jemalloc) that don't compile to WASM
- JSON format is portable and human-readable
- File-based storage works with WASI filesystem

**Capabilities:**
- `storage.save("path.json")` - persist all memories to file
- `storage.load("path.json")` - restore memories from file
- `storage.exists("path.json")` - check if file exists
- `storage.clear()` - clear in-memory store

**Limitations:**
- No concurrent access (single-writer)
- No query optimization (full scan for search)
- No transactions

**For advanced storage:** Use native Rust, Python, or Go bindings with full SurrealDB support.

### 2. No Real Clock Access

**Status:** Known, Minimal Impact

The `current_timestamp()` function returns a static timestamp because:

- WASI clock APIs are available but not yet integrated
- The WIT bindings would need to import `wasi:clocks/wall-clock`

**Impact:** Timestamps in memories will not be accurate. This affects `created_at` and `last_active` fields.

**Planned Fix:** Integrate WASI clock imports when adding full WASI support.

### 3. wit-bindgen Rust 2024 Compatibility

**Status:** Mitigated

The generated bindings produce `unsafe_op_in_unsafe_fn` warnings due to wit-bindgen not yet being fully Rust 2024 compatible.

**Mitigation:** Added `#![allow(unsafe_op_in_unsafe_fn)]` at crate level to suppress these upstream warnings.

**Tracking:** This is an upstream issue in the wit-bindgen project.

### 4. No Semantic Search / Embeddings

**Status:** Known Limitation

The WASM component uses simple keyword matching for search because:

- Embedding models are large and don't easily fit in WASM
- WASI SIMD support is still stabilizing
- ONNX runtimes don't have WASM builds

**Workaround:**
- Use for keyword-based search use cases
- Use native bindings for semantic search features
- Future: Connect to external embedding service

## Platform-Specific Notes

### Browser Usage

When using in browsers via `jco`:
- Must use `--instantiation async` for proper initialization
- WASI shims may be required for filesystem and random
- Consider memory limits of the browser environment

### Wasmtime

Fully supported. Use with:
```bash
wasmtime run --wasm component-model thymos_wasm.wasm
```

### Wazero (Go)

Component Model support in Wazero is experimental:
- Use the `experimental/wazeroapi` for component loading
- Some WASI features may not be available

## Reporting Issues

If you encounter issues not listed here, please file them at:
https://github.com/blakebarnett/thymos/issues

Include:
- WASM runtime and version
- Host language (if applicable)
- Minimal reproduction steps

