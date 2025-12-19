//! Zera NPC Agent Example
//!
//! This example demonstrates how to use Thymos to build a Zera-style NPC agent
//! with hybrid memory, concept extraction, relevance evaluation, and lifecycle management.

mod concepts;
mod game_context;
mod npc;
mod personality;
mod relevance;

use game_context::{GameContext, SharedGameContext};
use npc::ZeraNPC;
use personality::Personality;
use std::sync::Arc;
use std::sync::RwLock;
use thymos_core::memory::SearchScope;
use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,thymos=debug")
        .init();

    println!("Zera NPC Agent Example");
    println!("======================\n");

    // Create game context
    let game_context: SharedGameContext = Arc::new(RwLock::new(GameContext::new()));

    // Create Elder Rowan NPC
    println!("Creating Elder Rowan NPC...");
    let elder_rowan = match ZeraNPC::new(
        "elder_rowan",
        Personality::new()
            .with_trait("wisdom", 0.9)
            .with_trait("patience", 0.8)
            .with_speech_pattern("speaks slowly and thoughtfully")
            .with_speech_pattern("uses nature metaphors"),
        game_context.clone(),
    )
    .await
    {
        Ok(npc) => npc,
        Err(e) => {
            eprintln!("Failed to create NPC: {}", e);
            eprintln!("\nNote: This example requires a Locai server running on localhost:3000");
            eprintln!("Start the server with: locai-server");
            return Err(e);
        }
    };
    println!("✓ Elder Rowan created\n");

    // Simulate game interactions

    // 1. World observation (shared)
    println!("1. World Observation (Shared Memory)");
    println!("------------------------------------");
    elder_rowan
        .observe("The party entered the village of Oakshire")
        .await?;
    println!("✓ Stored observation in shared memory\n");

    // 2. Internal thought (private)
    println!("2. Internal Thought (Private Memory)");
    println!("-------------------------------------");
    elder_rowan
        .think("These adventurers seem trustworthy")
        .await?;
    println!("✓ Stored thought in private memory\n");

    // 3. Another NPC can see world state
    println!("3. Multi-Agent Shared Memory");
    println!("-----------------------------");
    let blacksmith = ZeraNPC::new(
        "blacksmith",
        Personality::new().with_trait("craftsmanship", 0.9),
        game_context.clone(),
    )
    .await?;

    let shared_memories = blacksmith.recall("Oakshire", SearchScope::Shared).await?;
    println!(
        "Blacksmith found {} shared memories about Oakshire",
        shared_memories.len()
    );
    for memory in &shared_memories {
        println!("  - {}", memory.content);
    }
    println!();

    // 4. But cannot see private thoughts
    println!("4. Private Memory Isolation");
    println!("-----------------------------");
    let all_memories = blacksmith
        .recall("trustworthy", SearchScope::Shared)
        .await?;
    println!("Blacksmith searched shared memory for 'trustworthy':");
    if all_memories.is_empty() {
        println!("  ✓ No results (private thought not visible)");
    } else {
        println!("  Found {} memories", all_memories.len());
    }
    println!();

    // 5. Update relevance based on game state
    println!("5. Relevance Evaluation");
    println!("------------------------");
    let mut context = game_context.write().unwrap();
    context.current_zone = "Oakshire".to_string();
    context.party_members = vec!["elder_rowan".to_string()];
    drop(context);

    let relevance = elder_rowan.evaluate_relevance().await?;
    println!(
        "Elder Rowan relevance: {:.2} ({:?})",
        relevance.value(),
        relevance.to_status(&thymos_core::lifecycle::RelevanceThresholds::default())
    );
    println!();

    // 6. Show memory scopes
    println!("6. Memory Scope Demonstration");
    println!("------------------------------");
    let private_memories = elder_rowan.recall("", SearchScope::Private).await?;
    println!("Elder Rowan private memories: {}", private_memories.len());

    let shared_memories = elder_rowan.recall("", SearchScope::Shared).await?;
    println!("Elder Rowan shared memories: {}", shared_memories.len());
    println!();

    println!("✓ Zera NPC example complete!");

    Ok(())
}
