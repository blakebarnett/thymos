//! Simple agent example demonstrating basic usage
//!
//! This example shows the simplest way to create and use a Thymos agent.
//! No configuration needed - just create and go!

use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("🚀 Simple Agent Example");
    println!("=======================\n");

    // Create an agent with default configuration - that's it!
    // No LLM, embeddings, or extractors needed for basic memory operations
    println!("Creating agent...");
    let agent = Agent::builder().id("simple_agent").build().await?;

    println!("✓ Agent created: {}", agent.id());
    println!("✓ Status: {:?}\n", agent.status().await);

    // Store some memories
    println!("Storing memories...");
    agent.remember("The sky is blue").await?;
    agent.remember("Water boils at 100°C").await?;
    agent
        .remember("Rust is a systems programming language")
        .await?;
    println!("✓ Stored 3 memories\n");

    // Search memories using BM25 search (built-in, no embeddings needed)
    println!("Searching for 'sky'...");
    let results = agent.search_memories("sky").await?;
    for memory in &results {
        println!("  • {}", memory.content);
    }
    println!("✓ Found {} result(s)\n", results.len());

    // Search for another term
    println!("Searching for 'Rust'...");
    let results = agent.search_memories("Rust").await?;
    for memory in &results {
        println!("  • {}", memory.content);
    }
    println!("✓ Found {} result(s)\n", results.len());

    println!("✨ Simple agent demonstration complete!");
    println!("\nNext steps:");
    println!("  • See examples/batteries_included.rs for LLM and embedding examples");
    println!("  • Add LLM provider for consolidation and concept extraction");
    println!("  • Add embedding provider for semantic search");

    Ok(())
}
