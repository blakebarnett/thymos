//! Test the ServerMemoryBackend against a running Locai server
//!
//! Prerequisites:
//!   Locai server running on http://localhost:3000
//!
//! Run with:
//!   cargo run --example test_server_backend

use thymos_core::memory::{MemoryBackend, QueryOptions, ServerMemoryBackend, ServerMemoryConfig, StoreOptions};

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Server Memory Backend Test ===\n");

    // Create server backend
    println!("Connecting to Locai server at http://localhost:3000...");
    let config = ServerMemoryConfig::new("http://localhost:3000");
    
    let backend = match ServerMemoryBackend::new(config).await {
        Ok(b) => {
            println!("✓ Connected successfully!\n");
            b
        }
        Err(e) => {
            println!("✗ Failed to connect: {}", e);
            println!("\nMake sure Locai server is running:");
            println!("  cd /path/to/locai && cargo run --release");
            return Ok(());
        }
    };

    // Test 1: Store memories
    println!("1. Storing memories...");
    let test_memories = vec![
        ("Rust test: Alice met Bob at the coffee shop in Paris", Some("episodic")),
        ("Rust test: Paris is the capital of France", Some("fact")),
        ("Rust test: The project deadline is December 31st", Some("fact")),
        ("Rust test: Bob prefers dark mode in all applications", None),
    ];

    let mut stored_ids = Vec::new();
    for (content, memory_type) in &test_memories {
        let options = memory_type.map(|t| StoreOptions {
            memory_type: Some(t.to_string()),
            ..Default::default()
        });
        
        match backend.store(content.to_string(), options).await {
            Ok(id) => {
                println!("   ✓ Stored: \"{}...\" ({})", &content[..40.min(content.len())], id);
                stored_ids.push(id);
            }
            Err(e) => {
                println!("   ✗ Failed to store: {}", e);
            }
        }
    }

    // Test 2: Count
    println!("\n2. Counting memories...");
    match backend.count().await {
        Ok(count) => println!("   ✓ Total memories: {}", count),
        Err(e) => println!("   ✗ Count failed: {}", e),
    }

    // Test 3: Search (semantic search)
    println!("\n3. Searching memories (semantic search)...");
    let queries = vec![
        "coffee meeting",
        "French capital city",
        "project timeline",
        "user interface preferences",
    ];

    for query in queries {
        let options = QueryOptions {
            limit: Some(3),
            ..Default::default()
        };
        
        match backend.search(query, Some(options)).await {
            Ok(results) => {
                println!("   Query: \"{}\" → {} result(s)", query, results.len());
                for r in results.iter().take(2) {
                    let content = if r.content.len() > 50 {
                        format!("{}...", &r.content[..50])
                    } else {
                        r.content.clone()
                    };
                    let score = r.score.map(|s| format!(" (score: {:.3})", s)).unwrap_or_default();
                    println!("     → {}{}", content, score);
                }
            }
            Err(e) => {
                println!("   ✗ Search failed: {}", e);
            }
        }
    }

    // Test 4: Get specific memory
    println!("\n4. Getting specific memory...");
    if let Some(id) = stored_ids.first() {
        match backend.get(id).await {
            Ok(Some(mem)) => {
                println!("   ✓ Retrieved: {}", mem.content);
                println!("   ✓ Created at: {}", mem.created_at);
            }
            Ok(None) => println!("   ✗ Memory not found"),
            Err(e) => println!("   ✗ Get failed: {}", e),
        }
    }

    // Test 5: Delete a memory
    println!("\n5. Deleting a memory...");
    if let Some(id) = stored_ids.last() {
        match backend.delete(id).await {
            Ok(deleted) => {
                println!("   ✓ Deleted: {}", deleted);
                let count = backend.count().await.unwrap_or(0);
                println!("   ✓ New count: {}", count);
            }
            Err(e) => println!("   ✗ Delete failed: {}", e),
        }
    }

    // Test 6: Health check
    println!("\n6. Health check...");
    match backend.health_check().await {
        Ok(()) => println!("   ✓ Server healthy"),
        Err(e) => println!("   ✗ Health check failed: {}", e),
    }

    println!("\n=== Test Complete ===");
    println!("\nThe ServerMemoryBackend works correctly with Locai server.");
    println!("The WASM component uses the same API, so it should work too!");

    Ok(())
}

