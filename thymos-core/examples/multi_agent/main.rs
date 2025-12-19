//! Multi-Agent Coordination Example
//!
//! This example demonstrates multiple agents coordinating through shared memory
//! and events. Shows how Thymos enables multi-agent scenarios with both private
//! and shared state.

mod analyzer;
mod coordinator;
mod researcher;

use analyzer::AnalyzerAgent;
use coordinator::CoordinatorAgent;
use researcher::ResearcherAgent;
use std::time::Duration;
use thymos_core::prelude::*;

const SHARED_LOCAI_URL: &str = "http://localhost:3000";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,thymos=debug")
        .init();

    println!("Multi-Agent Research Team Example");
    println!("==================================\n");

    // Create agents
    println!("Creating agents...");
    let researcher1 =
        match ResearcherAgent::new("researcher_1", "quantum computing", SHARED_LOCAI_URL).await {
            Ok(agent) => agent,
            Err(e) => {
                eprintln!("Failed to create researcher_1: {}", e);
                eprintln!("\nNote: This example requires a Locai server running on localhost:3000");
                eprintln!("Start the server with: locai-server");
                return Err(e);
            }
        };

    let researcher2 =
        match ResearcherAgent::new("researcher_2", "machine learning", SHARED_LOCAI_URL).await {
            Ok(agent) => agent,
            Err(e) => {
                eprintln!("Failed to create researcher_2: {}", e);
                return Err(e);
            }
        };

    let analyzer = match AnalyzerAgent::new("analyzer", SHARED_LOCAI_URL).await {
        Ok(agent) => agent,
        Err(e) => {
            eprintln!("Failed to create analyzer: {}", e);
            return Err(e);
        }
    };

    let coordinator = CoordinatorAgent::new(vec![
        "researcher_1".to_string(),
        "researcher_2".to_string(),
        "analyzer".to_string(),
    ]);

    println!("✓ All agents created\n");

    // Phase 1: Research
    println!("Phase 1: Research");
    println!("------------------");
    researcher1.research().await?;
    println!("✓ Researcher 1 completed research");

    researcher2.research().await?;
    println!("✓ Researcher 2 completed research\n");

    // Wait for memory to propagate
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Phase 2: Analysis
    println!("Phase 2: Analysis");
    println!("------------------");
    let insights = analyzer.analyze().await?;
    println!("✓ Analyzer generated {} insights", insights.len());
    for insight in &insights {
        println!(
            "  - {} (confidence: {:.2})",
            insight.summary, insight.confidence
        );
    }
    println!();

    // Phase 3: Coordination
    println!("Phase 3: Coordination");
    println!("----------------------");
    coordinator.coordinate().await?;
    println!();

    // Show shared vs private memory
    println!("Memory Demonstration");
    println!("--------------------");
    println!("Shared Memory (visible to all):");
    let shared = researcher1.agent.search_shared("").await?;
    for memory in shared.iter().take(5) {
        println!("  - {}", memory.content);
    }
    if shared.len() > 5 {
        println!("  ... and {} more", shared.len() - 5);
    }
    println!();

    println!("Private Memory (researcher_1 only):");
    let private = researcher1.agent.search_private("").await?;
    for memory in private.iter().take(5) {
        println!("  - {}", memory.content);
    }
    if private.len() > 5 {
        println!("  ... and {} more", private.len() - 5);
    }
    println!();

    println!("✓ Multi-agent coordination example complete!");

    Ok(())
}
