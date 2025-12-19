.PHONY: help build test lint fmt check clean docker-build docker-run

# Default target
.DEFAULT_GOAL := help

# Variables
VERSION := $(shell grep '^version = ' Cargo.toml | head -n 1 | sed 's/version = "\(.*\)"/\1/')
DOCKER_IMAGE := thymos-agent
DOCKER_TAG := latest

# Help target
help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Version: $(VERSION)'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# Build targets
build: ## Build release binary
	cargo build --release --workspace

build-debug: ## Build debug binary
	cargo build --workspace

clean: ## Clean build artifacts
	cargo clean

# Test targets
test: ## Run all tests
	cargo test --workspace

test-unit: ## Run unit tests only
	cargo test --lib --workspace

test-integration: ## Run integration tests
	cargo test --test '*' --workspace

test-watch: ## Run tests in watch mode
	cargo watch -x test

# Code quality targets
lint: ## Run linter (clippy)
	cargo clippy --workspace --all-features -- -D warnings

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check code formatting
	cargo fmt --all -- --check

check: fmt-check lint test ## Run all checks (fmt, lint, test)

# Documentation targets
docs: ## Generate and open documentation
	cargo doc --workspace --no-deps --open

docs-check: ## Check documentation
	cargo doc --workspace --no-deps

# Development targets
dev: ## Run development (not implemented yet)
	@echo "Development server not yet implemented"

watch: ## Watch for changes and rebuild
	cargo watch -x build

# Docker targets
docker-build: ## Build Docker image
	@echo "Building Docker image..."
	DOCKER_BUILDKIT=1 docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) .
	@echo "Tagging with version $(VERSION)..."
	docker tag $(DOCKER_IMAGE):$(DOCKER_TAG) $(DOCKER_IMAGE):$(VERSION)

docker-run: ## Run Docker container
	@echo "Starting agent container..."
	docker run -d \
		--name thymos-agent \
		-v thymos-data:/data \
		-e RUST_LOG=info,thymos=debug \
		$(DOCKER_IMAGE):$(DOCKER_TAG)

docker-stop: ## Stop running container
	@echo "Stopping agent container..."
	docker stop thymos-agent || true
	docker rm thymos-agent || true

docker-logs: ## View container logs
	docker logs -f thymos-agent

docker-shell: ## Open shell in running container
	docker exec -it thymos-agent bash

docker-clean: ## Clean up Docker resources
	@echo "Cleaning up Docker resources..."
	docker stop thymos-agent 2>/dev/null || true
	docker rm thymos-agent 2>/dev/null || true
	docker rmi $(DOCKER_IMAGE):$(DOCKER_TAG) 2>/dev/null || true
	docker rmi $(DOCKER_IMAGE):$(VERSION) 2>/dev/null || true

# Docker Compose targets
compose-up: ## Start services with docker-compose
	docker-compose up -d

compose-down: ## Stop services with docker-compose
	docker-compose down

compose-logs: ## View docker-compose logs
	docker-compose logs -f

compose-rebuild: ## Rebuild and restart services
	docker-compose up -d --build

# Example targets
example-simple: ## Run simple agent example
	cargo run --example simple_agent

example-lifecycle: ## Run memory lifecycle example
	cargo run --example memory_lifecycle

# CI targets
ci-test: ## Run tests in CI
	cargo test --workspace --release

ci-build: ## Build for CI
	cargo build --workspace --release

# WASM targets
build-wasm: ## Build WASM component (release)
	@command -v cargo-component >/dev/null 2>&1 || { echo "Installing cargo-component..."; cargo install cargo-component; }
	@rustup target list --installed | grep -q wasm32-wasip1 || rustup target add wasm32-wasip1
	cd thymos-wasm && cargo component build --release
	@echo "Output: target/wasm32-wasip1/release/thymos_wasm.wasm"
	@ls -lh target/wasm32-wasip1/release/thymos_wasm.wasm

build-wasm-debug: ## Build WASM component (debug)
	@command -v cargo-component >/dev/null 2>&1 || { echo "Installing cargo-component..."; cargo install cargo-component; }
	@rustup target list --installed | grep -q wasm32-wasip1 || rustup target add wasm32-wasip1
	cd thymos-wasm && cargo component build

# Installation
install: ## Install binaries to ~/.cargo/bin
	cargo install --path thymos-cli

uninstall: ## Uninstall binaries
	cargo uninstall thymos-cli



