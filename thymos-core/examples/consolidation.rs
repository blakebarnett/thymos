use thymos_core::consolidation::prelude::*;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Memory Consolidation Example ===\n");

    // Create consolidation configuration
    let config = ConsolidationConfig::new()
        .with_min_memories(5)
        .with_batch_size(50)
        .with_generate_insights(false); // No insights for MVP without LLM

    println!("Configuration:");
    println!("  Min memories: {}", config.min_memories);
    println!("  Batch size: {}", config.batch_size);
    println!("  Generate insights: {}", config.generate_insights);
    println!(
        "  Update importance scores: {}",
        config.update_importance_scores
    );
    println!();

    // Demonstrate insight types
    println!("=== Insight Types ===");
    let insight_types = [
        InsightType::Theme,
        InsightType::Pattern,
        InsightType::Relationship,
        InsightType::ImportantConcept,
        InsightType::EmotionalEvent,
        InsightType::Contradiction,
    ];

    for insight_type in &insight_types {
        println!(
            "- {}: {}",
            insight_type.as_str(),
            insight_type.description()
        );
    }
    println!();

    // Create example insights
    println!("=== Example Insights ===\n");

    let insight1 = Insight::theme(
        "The character consistently shows bravery in dangerous situations",
        vec![
            "memory_1".to_string(),
            "memory_5".to_string(),
            "memory_9".to_string(),
        ],
        0.85,
    );
    println!("Theme Insight:");
    println!("  Summary: {}", insight1.summary);
    println!("  Type: {}", insight1.insight_type.as_str());
    println!("  Confidence: {:.2}", insight1.confidence);
    println!("  Sources: {} memories", insight1.source_memory_ids.len());
    println!();

    let insight2 = Insight::pattern(
        "Elder mentions wisdom every time discussing important decisions",
        vec![
            "memory_2".to_string(),
            "memory_4".to_string(),
            "memory_7".to_string(),
        ],
        0.72,
    );
    println!("Pattern Insight:");
    println!("  Summary: {}", insight2.summary);
    println!("  Type: {}", insight2.insight_type.as_str());
    println!("  Confidence: {:.2}", insight2.confidence);
    println!("  Sources: {} memories", insight2.source_memory_ids.len());
    println!();

    let insight3 = Insight::relationship(
        "Strong alliance between the Elder and the Merchant",
        vec!["memory_3".to_string(), "memory_6".to_string()],
        0.90,
    );
    println!("Relationship Insight:");
    println!("  Summary: {}", insight3.summary);
    println!("  Type: {}", insight3.insight_type.as_str());
    println!("  Confidence: {:.2}", insight3.confidence);
    println!("  Sources: {} memories", insight3.source_memory_ids.len());
    println!();

    // Example with builder pattern and additional sources
    let insight4 = Insight::emotional_event(
        "The loss of the elder was deeply felt by the community",
        vec!["memory_10".to_string()],
        0.95,
    )
    .with_source("memory_11".to_string())
    .with_source("memory_12".to_string());

    println!("Emotional Event Insight:");
    println!("  Summary: {}", insight4.summary);
    println!("  Type: {}", insight4.insight_type.as_str());
    println!("  Confidence: {:.2}", insight4.confidence);
    println!("  Sources: {} memories", insight4.source_memory_ids.len());
    println!();

    // LLM configuration example
    println!("=== LLM Configuration ===");
    let llm_config = LLMConfig::new()
        .with_temperature(0.7)
        .with_max_tokens(1000)
        .with_system_prompt("You are analyzing memories to extract insights");

    println!("LLM Configuration:");
    println!("  Temperature: {}", llm_config.temperature);
    println!("  Max tokens: {}", llm_config.max_tokens);
    println!("  System prompt: {:?}", llm_config.system_prompt);
    println!();

    // Consolidation flow
    println!("=== Consolidation Workflow ===");
    println!("1. Collect memories over time");
    println!(
        "2. When threshold reached ({} memories):",
        config.min_memories
    );
    println!("   - Fetch memories in scope (session/time/tag)");
    println!("   - Generate insights using LLM (if enabled)");
    println!("   - Update importance scores");
    println!("   - Create consolidated memories");
    println!("3. Store insights with source references");
    println!("4. Return insights for further processing");

    Ok(())
}
