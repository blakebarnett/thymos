# Contributing to Thymos

Thank you for your interest in contributing to Thymos! This document provides guidelines for contributing to the project.

## Development Philosophy

Thymos is a companion project to [Locai](https://github.com/blakebarnett/locai) and follows similar principles:

- **Clarity over cleverness**: Write code that is easy to understand
- **Performance matters**: Rust's strengths should be leveraged
- **Minimal dependencies**: Only add dependencies that provide clear value
- **Test coverage**: All features should be tested
- **Documentation**: Public APIs must be documented

## Getting Started

### Prerequisites

- Rust 1.90.0+ (Rust 2024 edition)
- Git
- Docker (optional, for containerized development)

### Setup

```bash
# Clone the repository
git clone https://github.com/blakebarnett/thymos.git
cd thymos

# Build the project
make build

# Run tests
make test

# Run examples
make example-simple
```

## Code Style

### Formatting

We use `rustfmt` with custom configuration (see `rustfmt.toml`):

```bash
# Format code
make fmt

# Check formatting
make fmt-check
```

### Linting

We use `clippy` with strict warnings:

```bash
# Run clippy
make lint
```

### Conventional Commits

We follow [Conventional Commits](https://www.conventionalcommits.org/) for commit messages:

```bash
# Feature addition
git commit -m "feat: add concept extraction pipeline"

# Bug fix
git commit -m "fix: resolve memory leak in event system"

# Documentation update
git commit -m "docs: update API examples in README"

# Breaking change
git commit -m "feat!: redesign agent lifecycle API

BREAKING CHANGE: Agent.start() now returns a Result instead of panicking"
```

#### Commit Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Formatting, missing semi-colons, etc.
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding or correcting tests
- `build`: Changes to build system or dependencies
- `ci`: Changes to CI configuration
- `chore`: Other changes that don't modify src or test files

## Development Workflow

### 1. Create a Feature Branch

```bash
git checkout -b feat/my-new-feature
```

### 2. Make Changes

- Write code following the style guide
- Add tests for new functionality
- Update documentation as needed
- Run `make check` before committing

### 3. Commit Changes

```bash
git add .
git commit -m "feat: add my new feature"
```

### 4. Push and Create PR

```bash
git push origin feat/my-new-feature
```

Then create a pull request on GitHub.

## Testing

### Running Tests

```bash
# All tests
make test

# Unit tests only
make test-unit

# Integration tests
make test-integration
```

### Writing Tests

Place tests in the same file as the code being tested:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_feature() {
        // Test code
    }
}
```

For integration tests, create files in `tests/`:

```rust
// tests/integration_test.rs
use thymos_core::prelude::*;

#[tokio::test]
async fn test_agent_integration() {
    // Integration test code
}
```

## Documentation

### Code Documentation

All public APIs must have documentation comments:

```rust
/// Calculate memory strength using forgetting curve
///
/// Uses the Ebbinghaus forgetting curve: R = e^(-t/S)
///
/// # Arguments
///
/// * `memory` - The memory to calculate strength for
///
/// # Returns
///
/// Memory strength value between 0.0 and 1.0
pub fn calculate_strength(&self, memory: &Memory) -> f64 {
    // Implementation
}
```

### Building Documentation

```bash
# Generate and open docs
make docs

# Check docs build
make docs-check
```

## Architectural Guidelines

### Follow Locai Patterns

Thymos is a companion to Locai and should follow similar patterns:

- **Trait-based abstractions**: Use traits for extensibility
- **Builder pattern**: For complex object construction
- **Result types**: Use `Result<T>` for fallible operations
- **Async by default**: Use `async/await` with Tokio

### Example Structure

```rust
// Define trait for extensibility
#[async_trait]
pub trait MyTrait: Send + Sync {
    async fn my_method(&self) -> Result<()>;
}

// Implement default behavior
pub struct DefaultImpl;

#[async_trait]
impl MyTrait for DefaultImpl {
    async fn my_method(&self) -> Result<()> {
        Ok(())
    }
}

// Use builder pattern
pub struct MyStruct {
    field: String,
}

pub struct MyStructBuilder {
    field: Option<String>,
}

impl MyStructBuilder {
    pub fn new() -> Self {
        Self { field: None }
    }

    pub fn field(mut self, field: String) -> Self {
        self.field = Some(field);
        self
    }

    pub fn build(self) -> Result<MyStruct> {
        Ok(MyStruct {
            field: self.field.ok_or("field is required")?,
        })
    }
}
```

## Release Process

Releases are managed using conventional commits and semantic versioning:

- `fix`: Patch version bump (0.1.0 → 0.1.1)
- `feat`: Minor version bump (0.1.0 → 0.2.0)
- `BREAKING CHANGE` or `!`: Major version bump (0.1.0 → 1.0.0)

## Community

- Be respectful and inclusive
- Ask questions in issues or discussions
- Help others when possible
- Report bugs with detailed information

## License

By contributing, you agree that your contributions will be licensed under MIT or Apache-2.0 (dual license).

## Questions?

If you have questions about contributing, please open an issue or discussion on GitHub.



