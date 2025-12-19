//! Batteries Included: Zero-to-Agent in Minutes
//!
//! This example demonstrates how easy it is to get a fully-featured agent running
//! with minimal setup. Thymos provides default implementations for LLMs, embeddings,
//! and concept extraction - just configure and go!

use thymos_core::prelude::*;
use thymos_core::config::{MemoryConfig, MemoryMode};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    // Suppress verbose logs from locai/surrealdb to keep example output clean
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new("info")
                        .add_directive("locai=warn".parse().unwrap())
                        .add_directive("surrealdb=warn".parse().unwrap())
                        .add_directive("thymos_core=info".parse().unwrap())
                }),
        )
        .init();

    println!("🚀 Thymos: Batteries Included Example");
    println!("=====================================\n");

    // Example 1: Minimal Agent (No LLM needed)
    println!("1️⃣  Minimal Agent (No LLM)");
    println!("   Just memory and search - works out of the box!\n");

    // Use unique data directory for each agent to avoid lock conflicts
    let memory_config = MemoryConfig {
        mode: MemoryMode::Embedded {
            data_dir: std::path::PathBuf::from("./data/examples/minimal_agent"),
        },
        ..Default::default()
    };
    
    let agent = Agent::builder()
        .id("minimal_agent")
        .with_memory_config(memory_config)
        .build()
        .await?;

    agent.remember("Alice met Bob in Paris").await?;
    let memories = agent.search_memories("Alice").await?;
    println!("   ✓ Found {} memories about Alice\n", memories.len());
    
    // Drop agent to release database lock before creating next one
    drop(agent);

    // Example 2: Agent with LLM (Groq) - Just set GROQ_API_KEY env var!
    println!("2️⃣  Agent with LLM (Groq)");
    println!("   Set GROQ_API_KEY env var and you're ready!\n");

    if std::env::var("GROQ_API_KEY").is_ok() {
        #[cfg(feature = "llm-groq")]
        {
            use std::sync::Arc;
            use thymos_core::llm::providers::groq::GroqProvider;
            
            // Use unique data directory for each agent
            let memory_config = MemoryConfig {
                mode: MemoryMode::Embedded {
                    data_dir: std::path::PathBuf::from("./data/examples/llm_agent"),
                },
                ..Default::default()
            };
            
            // Model can come from GROQ_MODEL env var, or specify explicitly
            let llm = GroqProvider::from_env(None::<String>)?;
            let agent = Agent::builder()
                .id("llm_agent")
                .with_memory_config(memory_config)
                .llm_provider(Arc::new(llm))
                .build()
                .await?;
            
            println!("   ✓ Agent created with LLM support");
            println!("   ✓ Model: {} (from GROQ_MODEL or default)", agent.llm_provider().unwrap().model_info().model_name);
            println!("   ✓ Can now use LLM for consolidation, extraction, etc.\n");
            
            drop(agent);
        }
        #[cfg(not(feature = "llm-groq"))]
        {
            println!("   ⚠️  llm-groq feature not enabled\n");
        }
    } else {
        println!("   ⚠️  GROQ_API_KEY not set (skipping LLM example)\n");
    }

    // Example 3: Agent with Local Embeddings
    println!("3️⃣  Agent with Local Embeddings");
    println!("   Fast, free, runs locally - no API keys needed!\n");

    #[cfg(feature = "embeddings-local")]
    {
        use std::sync::Arc;
        use thymos_core::embeddings::providers::local::LocalEmbeddings;
        
        // Use unique data directory for each agent
        let memory_config = MemoryConfig {
            mode: MemoryMode::Embedded {
                data_dir: std::path::PathBuf::from("./data/examples/embedding_agent"),
            },
            ..Default::default()
        };
        
        let embeddings = LocalEmbeddings::default()?;
        let agent = Agent::builder()
            .id("embedding_agent")
            .with_memory_config(memory_config)
            .embedding_provider(Arc::new(embeddings))
            .build()
            .await?;

        println!("   ✓ Agent created with local embeddings");
        println!("   ✓ Can now use semantic search\n");
        
        drop(agent);
    }
    #[cfg(not(feature = "embeddings-local"))]
    {
        println!("   ⚠️  embeddings-local feature not enabled\n");
    }

    // Example 4: Configuration-Based Setup (Simplest!)
    println!("4️⃣  Configuration-Based Setup (Simplest!)");
    println!("   Load from thymos.toml + env vars - everything auto-configured!\n");

    // Try to load config (will use defaults if file doesn't exist)
    match ThymosConfig::load() {
        Ok(mut config) => {
            // Override data directory to avoid lock conflicts
            config.memory.mode = MemoryMode::Embedded {
                data_dir: std::path::PathBuf::from("./data/examples/config_agent"),
            };
            
            let _agent = Agent::builder()
                .id("config_agent")
                .config(config)
                .build()
                .await?;
            
            println!("   ✓ Agent created from configuration");
            println!("   ✓ LLM, embeddings, and extractors auto-configured if present\n");
            
            drop(_agent);
        }
        Err(e) => {
            println!("   ℹ️  No config file found (using defaults): {}\n", e);
        }
    }

    // Example 5: Full-Featured Agent with Everything
    println!("5️⃣  Full-Featured Agent");
    println!("   LLM + Embeddings + Concept Extraction - all configured!\n");

    #[cfg(all(feature = "llm-groq", feature = "embeddings-local"))]
    {
        use thymos_core::embeddings::providers::local::LocalEmbeddings;
        use thymos_core::llm::providers::groq::GroqProvider;

        if std::env::var("GROQ_API_KEY").is_ok() {
            // Use unique data directory for each agent
            let memory_config = MemoryConfig {
                mode: MemoryMode::Embedded {
                    data_dir: std::path::PathBuf::from("./data/examples/full_featured_agent"),
                },
                ..Default::default()
            };
            
            // Create LLM provider (model from GROQ_MODEL env var or default)
            let llm = GroqProvider::from_env(None::<String>)?;

            // Create embedding provider
            let embeddings = LocalEmbeddings::default()?;

            // Create concept extractor (auto-uses LLM if available)
            let config = ThymosConfig::default();
            let extractor = thymos_core::concepts::create_default_extractor(Some(&config)).await?;
            
            // Build full-featured agent
            use std::sync::Arc;
            let agent = Agent::builder()
                .id("full_featured_agent")
                .with_memory_config(memory_config)
                .llm_provider(Arc::new(llm))
                .embedding_provider(Arc::new(embeddings))
                .concept_extractor(extractor)
                .build()
                .await?;

            println!("   ✓ Full-featured agent created!");
            println!("   ✓ LLM: Available");
            println!("   ✓ Embeddings: Available");
            println!("   ✓ Concept Extraction: Available");

            // Demonstrate concept extraction
            let text = "Elder Rowan lives in the village of Oakshire. He met with Alice yesterday.";
            if let Some(extractor) = agent.concept_extractor() {
                let concepts = extractor.extract(text, None).await?;

                println!("\n   Extracted concepts:");
                for concept in concepts.iter().take(5) {
                    println!(
                        "     - {} ({}) [significance: {:.2}]",
                        concept.text, concept.concept_type, concept.significance
                    );
                }
            }
            
            drop(agent);
        } else {
            println!("   ⚠️  GROQ_API_KEY not set (skipping full example)\n");
        }
    }
    #[cfg(not(all(feature = "llm-groq", feature = "embeddings-local")))]
    {
        println!("   ⚠️  Required features not enabled\n");
    }

    println!("\n✨ Batteries Included - Ready to Use!");
    println!("\n📋 Status Summary:");
    
    // Check what's available
    let has_groq_key = std::env::var("GROQ_API_KEY").is_ok();
    let groq_model = std::env::var("GROQ_MODEL").ok();
    
    #[cfg(feature = "llm-groq")]
    let llm_feature_enabled = true;
    #[cfg(not(feature = "llm-groq"))]
    let llm_feature_enabled = false;
    
    #[cfg(feature = "embeddings-local")]
    let embeddings_feature_enabled = true;
    #[cfg(not(feature = "embeddings-local"))]
    let embeddings_feature_enabled = false;
    
    // Display status
    if llm_feature_enabled {
        if has_groq_key {
            println!("  ✓ LLM (Groq): Available");
            if let Some(ref model) = groq_model {
                println!("    Model: {} (from GROQ_MODEL)", model);
            } else {
                println!("    Model: Using default (llama-3.3-70b-versatile)");
            }
        } else {
            println!("  ⚠️  LLM (Groq): Feature enabled but GROQ_API_KEY not set");
        }
    } else {
        println!("  ⚠️  LLM (Groq): Feature 'llm-groq' not enabled");
    }
    
    if embeddings_feature_enabled {
        println!("  ✓ Embeddings (Local): Available");
    } else {
        println!("  ⚠️  Embeddings (Local): Feature 'embeddings-local' not enabled");
    }
    
    // Provide contextual next steps
    println!("\n💡 Next Steps:");
    let mut has_suggestions = false;
    
    if llm_feature_enabled && !has_groq_key {
        println!("  • Set GROQ_API_KEY environment variable for LLM support");
        has_suggestions = true;
    }
    
    if llm_feature_enabled && has_groq_key && groq_model.is_none() {
        println!("  • Set GROQ_MODEL to choose a specific model (optional)");
        has_suggestions = true;
    }
    
    if !llm_feature_enabled {
        println!("  • Enable 'llm-groq' feature: cargo run --example batteries_included --features llm-groq");
        has_suggestions = true;
    }
    
    if !embeddings_feature_enabled {
        println!("  • Enable 'embeddings-local' feature: cargo run --example batteries_included --features embeddings-local");
        has_suggestions = true;
    }
    
    if !has_suggestions {
        println!("  • Create a thymos.toml config file for persistent configuration");
        println!("  • Explore the Agent API: agent.remember(), agent.search_memories(), etc.");
        println!("  • See docs/design/BATTERIES_INCLUDED.md for advanced features");
    } else {
        println!("  • Create a thymos.toml config file for easy setup");
        println!("  • See docs/design/BATTERIES_INCLUDED.md for details");
    }

    Ok(())
}
