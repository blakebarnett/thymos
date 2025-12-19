use std::time::Duration;
use thymos_core::memory::ServerMemoryConfig;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Server Mode Memory Example ===\n");

    // Example 1: Configuration
    println!("Example 1: Server Mode Configuration");
    let config = ServerMemoryConfig::new("http://localhost:3000")
        .with_api_key("optional_api_key")
        .with_timeout(Duration::from_secs(60))
        .with_max_retries(3)
        .with_initial_backoff(Duration::from_millis(100));

    println!("Server Config:");
    println!("  Base URL: {}", config.base_url);
    println!("  API Key: {:?}", config.api_key.is_some());
    println!("  Timeout: {:?}", config.timeout);
    println!("  Max Retries: {}", config.max_retries);
    println!();

    // Example 2: What server mode enables
    println!("Example 2: Server Mode Capabilities");
    println!();
    println!("Server Mode (via HTTP):");
    println!("  ✓ Connect to shared Locai server");
    println!("  ✓ Multi-agent shared reality");
    println!("  ✓ Centralized memory store");
    println!("  ✓ Network-based access");
    println!("  ✓ API authentication support");
    println!();

    println!("Embedded Mode (current default):");
    println!("  ✓ Local-only memory store");
    println!("  ✓ Single-agent use");
    println!("  ✓ No network overhead");
    println!("  ✓ Privacy-focused");
    println!();

    // Example 3: Operations available through ServerMemoryBackend
    println!("Example 3: Available Operations");
    println!();
    println!("pub async fn store_memory(&self, content: String) -> Result<String>");
    println!("  Stores a memory on the server, returns memory ID");
    println!();
    println!("pub async fn search_memories(");
    println!("  &self,");
    println!("  query: &str,");
    println!("  limit: Option<usize>,");
    println!(") -> Result<Vec<Value>>");
    println!("  Search memories on server with optional limit");
    println!();
    println!("pub async fn health_check(&self) -> Result<()>");
    println!("  Verify connection to Locai server");
    println!();

    // Example 4: Architecture comparison
    println!("Example 4: Architecture Comparison");
    println!();
    println!("EMBEDDED MODE (Current):       SERVER MODE (Future):");
    println!("┌─────────────────┐           ┌─────────────────┐");
    println!("│   Agent 1       │           │   Agent 1       │");
    println!("│  ┌───────────┐  │           │  ┌───────────┐  │");
    println!("│  │ Memory    │  │           │  │ HTTP      │  │");
    println!("│  │ (Local)   │  │           │  │ Client    │  │");
    println!("│  └───────────┘  │           │  └────┬──────┘  │");
    println!("└─────────────────┘           └───────┼─────────┘");
    println!("                                      │ HTTP");
    println!("                              ┌───────▼─────────┐");
    println!("                              │ Locai Server    │");
    println!("                              │ ┌───────────┐   │");
    println!("                              │ │ Shared    │   │");
    println!("                              │ │ Memory    │   │");
    println!("                              │ │ Store     │   │");
    println!("                              │ └───────────┘   │");
    println!("                              └─────────────────┘");
    println!("                                      ▲ HTTP");
    println!("                              ┌───────┴─────────┐");
    println!("                              │   Agent 2       │");
    println!("                              │  ┌───────────┐  │");
    println!("                              │  │ HTTP      │  │");
    println!("                              │  │ Client    │  │");
    println!("                              │  └───────────┘  │");
    println!("                              └─────────────────┘");
    println!();

    // Example 5: Typical workflow
    println!("Example 5: Typical Server Mode Workflow");
    println!();
    println!("1. Create ServerMemoryConfig with server URL");
    println!("2. Build ServerMemoryBackend (performs health check)");
    println!("3. Use store_memory() to persist thoughts");
    println!("4. Use search_memories() to retrieve relevant memories");
    println!("5. Multiple agents access same shared reality");
    println!();

    // Example 6: When to use each mode
    println!("Example 6: When to Use Each Mode");
    println!();
    println!("Use EMBEDDED MODE when:");
    println!("  • Single-agent, local-only use");
    println!("  • Privacy is paramount");
    println!("  • No network infrastructure");
    println!("  • Simple agent experiments");
    println!();
    println!("Use SERVER MODE when:");
    println!("  • Multi-agent coordination needed");
    println!("  • Shared memory is required");
    println!("  • Scaling across machines");
    println!("  • Centralized memory management");
    println!("  • Game worlds with multiple NPCs");
    println!();

    println!("Note: Server mode requires a running Locai server instance");
    println!("For more info, see: https://github.com/blakebarnett/locai");

    Ok(())
}
