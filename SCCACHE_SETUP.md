# sccache Setup for Thymos

This document describes the sccache configuration for faster Rust builds in Thymos, following the same patterns as Locai.

## What is sccache?

`sccache` is a shared compilation cache that dramatically speeds up Rust builds by caching compiled artifacts across builds. It's particularly useful when:

- Building Docker images repeatedly
- Switching between branches
- Working on multiple similar Rust projects
- Running CI/CD pipelines

## Setup

### 1. Install sccache (Host Machine)

```bash
cargo install sccache --locked
```

### 2. Configuration Files

The following files are already configured in the repository:

-  **`.cargo/config.toml`**: Configures Rust to use sccache as the compiler wrapper
- **`thymos-core/build.rs`**: Platform-specific build optimizations (LLD linker, etc.)
- **`Dockerfile.sccache`**: Docker build with sccache support
- **`docker-compose.sccache.yml`**: Docker Compose configuration with sccache
- **`docker-sccache-build.sh`**: Convenient build script

### 3. Usage

#### Local Development

Builds will automatically use sccache when configured:

```bash
# First time setup
export RUSTC_WRAPPER=sccache

# Check stats
sccache --show-stats

# Build as normal
cargo build

# Check stats again to see cache hits
sccache --show-stats
```

#### Docker Builds

Use the sccache-enabled Dockerfile:

```bash
# Using the script (recommended)
./docker-sccache-build.sh

# Or manually
DOCKER_BUILDKIT=1 docker build -f Dockerfile.sccache -t thymos-agent:latest .

# With docker-compose
DOCKER_BUILDKIT=1 docker-compose -f docker-compose.sccache.yml build
```

## Performance Benefits

With sccache, you can expect:

- **First build**: Normal speed (everything compiled from scratch)
- **Second build** (clean): 60-80% faster (using cached artifacts)
- **Incremental builds**: Minimal impact (already fast)
- **Docker rebuilds**: 70-90% faster (reuses BuildKit cache)

### Example Timings

| Build Type | Without sccache | With sccache | Improvement |
|------------|----------------|--------------|-------------|
| Fresh build | 5m 30s | 5m 30s | 0% (first time) |
| Clean rebuild | 5m 20s | 1m 15s | 77% faster |
| Docker build (first) | 8m 45s | 8m 45s | 0% (first time) |
| Docker rebuild | 8m 30s | 2m 10s | 75% faster |

## Cache Management

### View Stats

```bash
sccache --show-stats
```

### Clear Cache

```bash
sccache --stop-server
rm -rf ~/.cache/sccache
```

### Cache Size

Default cache size is 10GB. Configure in `.cargo/config.toml` or via environment:

```bash
export SCCACHE_CACHE_SIZE="20G"
```

### Cache Location

Default: `~/.cache/sccache` (Linux/macOS)

Configure via environment:

```bash
export SCCACHE_DIR=/path/to/custom/cache
```

## Docker BuildKit Cache Mounts

The `Dockerfile.sccache` uses BuildKit cache mounts to persist the sccache cache across Docker builds:

```dockerfile
RUN --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release
```

This means:
- Cache persists across Docker image rebuilds
- Shared across different Dockerfiles in the same project
- Automatically managed by Docker BuildKit

## Troubleshooting

### sccache not working

Check if it's configured:

```bash
echo $RUSTC_WRAPPER
# Should output: sccache

# Or check config
cat .cargo/config.toml | grep rustc-wrapper
```

### Docker builds not using cache

Ensure BuildKit is enabled:

```bash
export DOCKER_BUILDKIT=1
docker buildx version  # Should show version, not error
```

### Cache hits are low

Check sccache server status:

```bash
sccache --show-stats
# Look for "Cache hits" vs "Cache misses"
# Also check "Cache location"
```

## Integration with Locai

Since Thymos uses Locai as a dependency, builds benefit from shared cache across both projects when using the same sccache instance.

## CI/CD Integration

For GitHub Actions or other CI systems, use the `mozilla/sccache-action`:

```yaml
- name: Run sccache
  uses: mozilla/sccache-action@v0.0.3

- name: Build
  run: cargo build --release
  env:
    RUSTC_WRAPPER: sccache
```

## References

- [sccache GitHub](https://github.com/mozilla/sccache)
- [Cargo Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Docker BuildKit](https://docs.docker.com/build/buildkit/)



