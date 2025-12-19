//! Memory lifecycle example demonstrating forgetting curves

use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,thymos=debug")),
        )
        .init();

    tracing::info!("Memory lifecycle demonstration");

    // Create an agent with custom memory configuration
    let memory_config = MemoryConfig {
        forgetting_curve_enabled: true,
        recency_decay_hours: 24.0, // 1 day
        access_count_weight: 0.2,
        emotional_weight_multiplier: 2.0,
        ..Default::default()
    };

    let agent = Agent::builder()
        .id("lifecycle_agent")
        .with_memory_config(memory_config)
        .build()
        .await?;

    tracing::info!("Storing memories...");

    // Store various memories
    let memory_id_1 = agent.remember("Recent important event").await?;
    let memory_id_2 = agent.remember("Casual observation").await?;
    let memory_id_3 = agent.remember("Critical information").await?;

    tracing::info!("Calculating memory strengths...");

    // Get memories and calculate their strengths
    if let Some(memory) = agent.get_memory(&memory_id_1).await? {
        let strength = agent.memory().calculate_strength(&memory);
        tracing::info!("Memory 1 strength: {:.2} ({})", strength, memory.content);
    }

    if let Some(memory) = agent.get_memory(&memory_id_2).await? {
        let strength = agent.memory().calculate_strength(&memory);
        tracing::info!("Memory 2 strength: {:.2} ({})", strength, memory.content);
    }

    if let Some(memory) = agent.get_memory(&memory_id_3).await? {
        let strength = agent.memory().calculate_strength(&memory);
        tracing::info!("Memory 3 strength: {:.2} ({})", strength, memory.content);
    }

    tracing::info!("Memory lifecycle demonstration complete!");
    tracing::info!(
        "Note: In a real scenario, memory strengths would decay over time based on access patterns"
    );

    Ok(())
}
