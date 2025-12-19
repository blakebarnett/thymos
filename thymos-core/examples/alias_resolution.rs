use thymos_core::concepts::prelude::*;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Alias Resolution Example ===\n");

    // Example 1: Extract aliases from text
    println!("Example 1: Extract Aliases for a Concept");
    let text = "Elder Rowan, known as the old badger, was a wise character. They called him Rowan for short.";
    let aliases = AliasExtractor::extract_aliases(text, "Elder Rowan")?;

    println!("Text: \"{}\"", text);
    println!("Aliases extracted for 'Elder Rowan':");
    for alias in &aliases {
        println!(
            "  - {} [{}] (confidence: {:.2})",
            alias.text,
            alias.alias_type.as_str(),
            alias.confidence
        );
    }

    println!();

    // Example 2: Create concept with aliases
    println!("Example 2: Concept with Aliases");
    let mut concept = Concept::new("Elder Rowan", "character", "A wise elder", 0.9);

    // Add manually extracted aliases
    concept = concept
        .with_alias(Alias::epithet("the old badger", 0.85))
        .with_alias(Alias::new_alias("Rowan", 0.90))
        .with_alias(Alias::descriptor("the wise", 0.75));

    println!("Concept: {}", concept.text);
    println!("Aliases by confidence:");
    for alias in concept.aliases_by_confidence() {
        println!("  - {} (confidence: {:.2})", alias.text, alias.confidence);
    }

    println!();

    // Example 3: Resolve aliases to canonical name
    println!("Example 3: Resolve Aliases to Canonical Names");
    let candidates = vec!["Elder Rowan", "Rowan", "The Badger", "Oakshire"];

    let test_aliases = vec!["rowan", "the badger", "elder"];

    for test_alias in test_aliases {
        match AliasExtractor::resolve_alias(test_alias, &candidates) {
            Some((canonical, confidence)) => {
                println!(
                    "  '{}' → '{}' (confidence: {:.2})",
                    test_alias, canonical, confidence
                );
            }
            None => {
                println!("  '{}' → No match found", test_alias);
            }
        }
    }

    println!();

    // Example 4: Alias types and provenance
    println!("Example 4: Alias Types and Provenance");
    let examples = vec![
        ("the old badger", Alias::epithet("the old badger", 0.85)),
        ("aka Rowan", Alias::new_alias("Rowan", 0.90)),
        ("Dr. Rowan", Alias::title("Dr.", 0.99)),
        ("the wise one", Alias::descriptor("the wise one", 0.75)),
    ];

    for (description, alias) in examples {
        println!(
            "  {} → {} [{}] (provenance: {})",
            description,
            alias.text,
            alias.alias_type.as_str(),
            alias.provenance.as_str()
        );
    }

    Ok(())
}
