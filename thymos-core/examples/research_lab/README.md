# Research Lab Showcase

A multi-agent research system demonstrating Thymos capabilities with real tool integration.

## Features Demonstrated

- ✅ **Multi-Agent Coordination** - Specialized agents working together
- ✅ **Real Tool Integration** - Browser automation, web search, LLM
- ✅ **Memory Management** - Agents store and retrieve research findings
- ✅ **LLM-Powered Analysis** - Uses local Ollama LLM for reasoning
- ✅ **Supervisor-Ready** - Architecture supports automatic lifecycle management

## Prerequisites

1. **Ollama running** with `qwen3:14b` model:
   ```bash
   ollama pull qwen3:14b
   ollama serve
   ```

2. **Playwright installed** (will be installed automatically on first run, or install manually):
   ```bash
   # Playwright will auto-install on first run, or install manually:
   npx playwright install chromium
   ```

3. **Locai Server (Optional but Recommended)** - For shared memory between agents:
   ```bash
   # Install locai-server if not already installed
   cargo install locai-server
   
   # Start the server (defaults to http://localhost:3000)
   locai-server
   ```
   
   **Note**: Without a Locai server, agents will use separate memory stores and won't be able to share findings. The example will detect if the server is available and warn if it's not running.

4. **Rust toolchain** with required features

## Running the Example

```bash
# From the thymos-core directory
cargo run --example research_lab --features llm-ollama,browser-playwright
```

Or set environment variables for custom configuration:

```bash
export OLLAMA_MODEL=qwen3:14b
export OLLAMA_BASE_URL=http://localhost:11434
export SHARED_LOCAI_URL=http://localhost:3000  # Optional: custom Locai server URL
cargo run --example research_lab --features llm-ollama,browser-playwright
```

**Important**: For full functionality, start the Locai server first:
```bash
# Terminal 1: Start Locai server
locai-server

# Terminal 2: Run the example
cargo run --example research_lab --features llm-ollama,browser-playwright
```

## Architecture

### Agents

1. **Research Coordinator** - Plans research and orchestrates tasks
2. **Literature Reviewer** - Reviews and summarizes papers (demo ready)
3. **Web Researcher** - Conducts web research and fact-checking
4. **Synthesis Agent** - Combines findings into comprehensive answers

### Tools

- **BrowserTool** - Fetches and extracts content from URLs
- **WebSearchTool** - Searches the web for information

## Example Workflow

1. User submits research query
2. Coordinator creates research plan
3. Web Researcher searches for current information
4. Literature Reviewer processes papers (when available)
5. Synthesis Agent combines findings into final answer

## Memory Architecture

The example uses **Hybrid Memory Mode** by default:
- **Private Memory**: Each agent stores internal thoughts and private state locally
- **Shared Memory**: Research findings are stored in a shared Locai server, visible to all agents
- **Automatic Detection**: The example checks for a Locai server and falls back to embedded mode if unavailable

This enables:
- ✅ **Agent Coordination** - Agents can see each other's findings
- ✅ **Synthesis** - The synthesis agent can access all research results
- ✅ **Privacy** - Internal agent thoughts remain private

## Extending

This showcase can be extended with:

- **Supervisor Integration** - Add automatic agent lifecycle management
- **Memory Versioning** - Track research evolution over time
- **More Tools** - Add PDF processing, academic search APIs, etc.
- **Better Error Handling** - Retry logic, fallback strategies

## See Also

- [Research Lab Showcase Design](../../../docs/showcase/RESEARCH_LAB_SHOWCASE.md)
- [Thymos Documentation](../../../README.md)

