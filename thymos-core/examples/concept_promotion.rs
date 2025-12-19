use thymos_core::concepts::prelude::*;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Concept Promotion Pipeline Example ===\n");

    // Create a promotion pipeline with custom configuration
    let config = PromotionConfig::new()
        .with_promotion_threshold(0.6)
        .with_min_mentions_provisional(2)
        .with_min_mentions_tracked(4);

    let pipeline = ConceptPromotionPipeline::new(config);

    println!("Tracking concept mentions with custom thresholds:");
    println!("- Threshold: 0.6");
    println!("- Min for Provisional: 2 mentions");
    println!("- Min for Tracked: 4 mentions\n");

    // Scenario 1: Track mentions of a character
    println!("Scenario: Tracking mentions of 'Elder Rowan'");
    println!();

    let mut current_tier;

    // Mention 1
    current_tier = pipeline
        .track_mention(
            "Elder Rowan",
            "memory_1",
            "Elder Rowan lives in the village",
            0.8,
        )
        .await?;
    println!("Mention 1: Tier = {:?}", current_tier);
    if let Ok(stats) = pipeline.get_stats("Elder Rowan") {
        if let Some(s) = stats {
            println!(
                "  Count: {}, Avg significance: {:.2}",
                s.mention_count, s.avg_significance
            );
        }
    }
    println!();

    // Mention 2
    current_tier = pipeline
        .track_mention(
            "Elder Rowan",
            "memory_2",
            "Elder Rowan was wise and respected",
            0.85,
        )
        .await?;
    println!(
        "Mention 2: Tier = {:?} ⬆ PROMOTED TO PROVISIONAL",
        current_tier
    );
    if let Ok(stats) = pipeline.get_stats("Elder Rowan") {
        if let Some(s) = stats {
            println!(
                "  Count: {}, Avg significance: {:.2}",
                s.mention_count, s.avg_significance
            );
        }
    }
    println!();

    // Mention 3
    current_tier = pipeline
        .track_mention(
            "Elder Rowan",
            "memory_3",
            "Many spoke highly of Elder Rowan",
            0.75,
        )
        .await?;
    println!("Mention 3: Tier = {:?}", current_tier);
    if let Ok(stats) = pipeline.get_stats("Elder Rowan") {
        if let Some(s) = stats {
            println!(
                "  Count: {}, Avg significance: {:.2}",
                s.mention_count, s.avg_significance
            );
        }
    }
    println!();

    // Mention 4
    current_tier = pipeline
        .track_mention(
            "Elder Rowan",
            "memory_4",
            "Elder Rowan's wisdom was legendary",
            0.9,
        )
        .await?;
    println!("Mention 4: Tier = {:?} ⬆ PROMOTED TO TRACKED", current_tier);
    if let Ok(stats) = pipeline.get_stats("Elder Rowan") {
        if let Some(s) = stats {
            println!(
                "  Count: {}, Avg significance: {:.2}",
                s.mention_count, s.avg_significance
            );
            println!("  Peak: {:.2}", s.peak_significance);
        }
    }
    println!();

    // Show tier progression
    println!("=== Tier Progression ===");
    println!(
        "1 mention → Mentioned (tier value: {})",
        ConceptTier::Mentioned as i32
    );
    println!(
        "2+ mentions (0.6+ significance) → Provisional (tier value: {})",
        ConceptTier::Provisional as i32
    );
    println!(
        "4+ mentions (0.6+ significance) → Tracked (tier value: {})",
        ConceptTier::Tracked as i32
    );
    println!();

    // Show all tracked concepts
    println!("=== All Tracked Concepts ===");
    if let Ok(concepts) = pipeline.get_all_concepts() {
        for (name, tier) in concepts {
            println!("  - {}: {}", name, tier);
        }
    }
    println!();

    // Show mention history
    println!("=== Mention History for 'Elder Rowan' ===");
    if let Ok(history) = pipeline.get_mention_history("Elder Rowan") {
        for (i, mention) in history.iter().enumerate() {
            println!(
                "  {}. Memory {}: significance {:.2}, timestamp: {}",
                i + 1,
                mention.memory_id,
                mention.significance,
                mention.timestamp.format("%H:%M:%S")
            );
        }
    }
    println!();

    // Demonstrate tier methods
    println!("=== Tier Operations ===");
    let tier = ConceptTier::Mentioned;
    println!("Mentioned tier:");
    println!("  as_str: {}", tier.as_str());
    println!("  promote: {:?}", tier.promote());
    println!("  demote: {:?}", tier.demote());
    println!();

    let tier = ConceptTier::Provisional;
    println!("Provisional tier:");
    println!("  as_str: {}", tier.as_str());
    println!("  promote: {:?}", tier.promote());
    println!("  demote: {:?}", tier.demote());

    Ok(())
}
