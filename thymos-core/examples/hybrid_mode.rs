use std::path::PathBuf;
use thymos_core::config::MemoryMode;
use thymos_core::prelude::*;

#[tokio::main]
async fn main() -> thymos_core::error::Result<()> {
    println!("=== Hybrid Memory Mode Example ===\n");

    // Create hybrid memory configuration
    let _config = MemoryConfig {
        mode: MemoryMode::Hybrid {
            private_data_dir: PathBuf::from("./data/private"),
            shared_url: "http://localhost:3000".to_string(),
            shared_api_key: None,
        },
        ..Default::default()
    };

    println!("Configuration:");
    println!("  Private: Embedded Locai at ./data/private");
    println!("  Shared: Locai server at http://localhost:3000");
    println!();

    // Note: This example demonstrates the API structure
    // In practice, you'd need a running Locai server for shared storage
    println!("=== Hybrid Memory Architecture ===");
    println!();
    println!("PRIVATE BACKEND (Embedded):");
    println!("  ✓ Agent's internal thoughts");
    println!("  ✓ Personal observations");
    println!("  ✓ Private state");
    println!("  ✓ Not visible to other agents");
    println!();

    println!("SHARED BACKEND (Server):");
    println!("  ✓ World state observations");
    println!("  ✓ Public facts");
    println!("  ✓ Shared knowledge");
    println!("  ✓ Visible to all agents");
    println!();

    println!("=== Usage Patterns ===");
    println!();
    println!("1. Store Private Memory:");
    println!("   memory_system.remember_private(\"I don't trust that merchant\").await?;");
    println!();

    println!("2. Store Shared Memory:");
    println!("   memory_system.remember_shared(\"The merchant showed rare artifacts\").await?;");
    println!();

    println!("3. Search Private Only:");
    println!("   let results = memory_system");
    println!("       .search_with_scope(\"merchant\", SearchScope::Private, None)");
    println!("       .await?;");
    println!();

    println!("4. Search Shared Only:");
    println!("   let results = memory_system");
    println!("       .search_with_scope(\"merchant\", SearchScope::Shared, None)");
    println!("       .await?;");
    println!();

    println!("5. Search Both:");
    println!("   let results = memory_system");
    println!("       .search_with_scope(\"merchant\", SearchScope::Both, None)");
    println!("       .await?;");
    println!();

    println!("=== Routing Strategy ===");
    println!();
    println!("Automatic routing based on tags:");
    println!("  - Tag 'private' → Private backend");
    println!("  - Tag 'shared' → Shared backend");
    println!("  - Default → Private backend");
    println!();

    println!("Example:");
    println!("  let routing = RoutingStrategy::new(MemoryScope::Private)");
    println!("      .with_tag_rule(\"shared\", MemoryScope::Shared)");
    println!("      .with_tag_rule(\"public\", MemoryScope::Shared);");
    println!();

    println!("=== Multi-Agent Scenario ===");
    println!();
    println!("Agent 1:");
    println!("  - Stores private: \"I think the quest is dangerous\"");
    println!("  - Stores shared: \"The party entered Oakshire\"");
    println!();

    println!("Agent 2:");
    println!("  - Can see: \"The party entered Oakshire\" (shared)");
    println!("  - Cannot see: \"I think the quest is dangerous\" (private)");
    println!();

    println!("=== Benefits ===");
    println!();
    println!("✓ Privacy: Internal thoughts stay private");
    println!("✓ Shared Reality: World state visible to all");
    println!("✓ Performance: Private queries don't hit network");
    println!("✓ Scalability: Shared storage scales independently");
    println!("✓ Flexibility: Choose storage location per memory");
    println!();

    println!("Note: This example shows the API structure.");
    println!("For a working example, start a Locai server and use real URLs.");

    Ok(())
}
