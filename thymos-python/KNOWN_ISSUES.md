# Known Issues - Thymos Python Bindings

## TLS Allocation Error in Python Extensions

### Status: **MITIGATED**

As of the latest update, the TLS allocation issue should be significantly mitigated
by enabling the `disable_initial_exec_tls` feature on `tikv-jemalloc-sys`.

### Error

```
ImportError: cannot allocate memory in static TLS block
```

### Root Cause

jemalloc (used by SurrealDB) requires Thread Local Storage (TLS) space that 
exceeds what's available when loading Python extensions. The TLS space is 
allocated when the `.so` file is loaded, before any Python code can run.

The dependency chain is:
`surrealdb-core` → `tikv-jemallocator` → `tikv-jemalloc-sys`

### Applied Fix

The Python bindings now explicitly enable `disable_initial_exec_tls`:

```toml
# In thymos-python/Cargo.toml
tikv-jemalloc-sys = { version = "0.6", features = ["disable_initial_exec_tls"] }
```

This builds jemalloc with `--disable-initial-exec-tls`, allowing it to be
dynamically loaded after program startup.

### Additional Mitigation

The Python bindings also use `kv-mem` instead of `kv-rocksdb` to reduce
complexity:

```toml
[patch.crates-io]
surrealdb = { version = "2.3.10", default-features = false, features = ["kv-mem", ...] }
```

### If You Still Experience Issues

1. **Environment variables**:
   ```bash
   export MALLOC_CONF="narenas:1,tcache:false"
   ```

2. **LD_PRELOAD** (Linux only):
   ```bash
   export LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libjemalloc.so.2
   ```

3. **Use Thymos via Locai server**:
   Instead of embedding, connect to a remote Locai server which runs in a 
   separate process.

### Related Issues

- https://github.com/jemalloc/jemalloc/issues/1237
- https://bugs.python.org/issue37195
- Various RocksDB/Python binding discussions
