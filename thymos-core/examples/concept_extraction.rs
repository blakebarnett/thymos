use thymos_core::concepts::prelude::*;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Concept Extraction Example ===\n");

    // Create extractor with default configuration
    let extractor = BasicConceptExtractor::new(ConceptExtractionConfig::default())?;

    // Example 1: Simple text
    println!("Example 1: Character and Location Extraction");
    let text1 = "Elder Rowan lives in the village of Oakshire";
    let concepts1 = extractor.extract(text1, None).await?;

    println!("Text: \"{}\"", text1);
    println!("Extracted concepts:");
    for concept in &concepts1 {
        println!(
            "  - {} [{}] (significance: {:.2})",
            concept.text, concept.concept_type, concept.significance
        );
        println!("    Context: \"{}\"", concept.context);
    }

    println!();

    // Example 2: Custom configuration
    println!("Example 2: Custom Configuration with Higher Threshold");
    let custom_config = ConceptExtractionConfig::new()
        .with_concept_type("character", {
            use thymos_core::concepts::config::ConceptTypeConfig;
            ConceptTypeConfig::new(
                "Character",
                vec![r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)?)\b".to_string()],
            )
            .with_base_significance(0.85)
        })
        .with_concept_type("location", {
            use thymos_core::concepts::config::ConceptTypeConfig;
            ConceptTypeConfig::new(
                "Location",
                vec![r"\b(?:in|at|from|to)\s+([A-Z][a-z]+)\b".to_string()],
            )
            .with_base_significance(0.70)
        })
        .with_threshold(0.6);

    let custom_extractor = BasicConceptExtractor::new(custom_config)?;

    let text2 = "The merchant traveled from Riverholm to Westmarch, meeting with King Edward.";
    let concepts2 = custom_extractor.extract(text2, None).await?;

    println!("Text: \"{}\"", text2);
    println!("Extracted concepts (threshold: 0.6):");
    for concept in &concepts2 {
        println!(
            "  - {} [{}] (significance: {:.2})",
            concept.text, concept.concept_type, concept.significance
        );
    }

    println!();

    // Example 3: Duplicate handling
    println!("Example 3: Duplicate Handling");
    let text3 = "Alice met Bob. Alice liked Bob. Bob said goodbye to Alice.";
    let concepts3 = extractor.extract(text3, None).await?;

    println!("Text: \"{}\"", text3);
    println!(
        "Found {} unique concepts (duplicates automatically filtered):",
        concepts3.len()
    );
    for concept in &concepts3 {
        println!("  - {}", concept.text);
    }

    println!();

    // Example 4: Significance scoring
    println!("Example 4: Significance Scoring (position and length matter)");
    let text4 = "Christopher mentioned a sword. The blade was excellent.";
    let concepts4 = extractor.extract(text4, None).await?;

    println!("Text: \"{}\"", text4);
    println!("Concepts sorted by significance:");
    for concept in &concepts4 {
        println!(
            "  - {} (significance: {:.2}) - Early mention boost applied",
            concept.text, concept.significance
        );
    }

    Ok(())
}
