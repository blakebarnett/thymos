//! Research Lab Showcase: Multi-Agent Research System
//!
//! Demonstrates Thymos capabilities:
//! - Multi-agent coordination
//! - Real tool integration (browser, LLM)
//! - Memory sharing and coordination
//! - Agent lifecycle management

use thymos_core::prelude::*;
use thymos_core::llm::providers::ollama::OllamaProvider;
use std::sync::Arc;

mod tools;
mod agents;

use agents::{ResearchCoordinator, LiteratureReviewer, WebResearcher, SynthesisAgent};

async fn check_locai_server(url: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    
    match client
        .get(format!("{}/api/health", url))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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

    println!("🔬 Thymos Research Lab Showcase");
    println!("================================\n");

    println!("Initializing Ollama LLM provider...");
    let llm = Arc::new(OllamaProvider::from_env(Some("qwen3:14b"))?);
    println!("✓ LLM provider ready: {}\n", llm.model_info().model_name);

    let shared_memory_url = std::env::var("SHARED_LOCAI_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    println!("Checking for shared Locai server at {}...", shared_memory_url);
    let server_available = check_locai_server(&shared_memory_url).await;
    
    if !server_available {
        println!("⚠️  Locai server not available at {}", shared_memory_url);
        println!("   Agents will use separate memory stores (findings won't be shared)");
        println!("   To enable shared memory, start Locai server: locai-server\n");
    } else {
        println!("✓ Locai server found - agents will share findings\n");
    }

    println!("Creating research agents...\n");

    let coordinator = ResearchCoordinator::new(llm.clone()).await?;
    println!("✓ Research Coordinator created");

    let _literature_reviewer = LiteratureReviewer::new(llm.clone(), Some(shared_memory_url.clone())).await?;
    println!("✓ Literature Reviewer created");

    let web_researcher = WebResearcher::new(llm.clone(), Some(shared_memory_url.clone())).await?;
    println!("✓ Web Researcher created");

    let synthesis_agent = SynthesisAgent::new(llm.clone(), Some(shared_memory_url)).await?;
    println!("✓ Synthesis Agent created\n");

    println!("📋 Research Query:");
    let query = "How do large language models work?";
    println!("   {}\n", query);

    println!("1️⃣  Coordinator Planning Research...");
    let plan = match coordinator.plan_research(query).await {
        Ok(p) => {
            if p.trim().is_empty() {
                eprintln!("   ⚠️  Warning: Plan is empty!");
                "No plan generated".to_string()
            } else {
                p
            }
        }
        Err(e) => {
            eprintln!("   ❌ Error planning research: {}", e);
            format!("Error: {}", e)
        }
    };
    println!("   Plan created:\n   {}\n", plan.lines().take(5).collect::<Vec<_>>().join("\n   "));

    println!("2️⃣  Web Researcher Conducting Research...");
    let web_findings = match web_researcher.research(query).await {
        Ok(f) => {
            if f.trim().is_empty() {
                eprintln!("   ⚠️  Warning: Web findings are empty!");
                "No findings".to_string()
            } else {
                f
            }
        }
        Err(e) => {
            eprintln!("   ❌ Error conducting web research: {}", e);
            format!("Error: {}", e)
        }
    };
    println!("   ✓ Web research complete");
    println!("   Findings preview: {}\n", 
        &web_findings[..web_findings.len().min(200)]);

    println!("3️⃣  Literature Reviewer Processing Papers...");
    println!("   (Skipping paper review in demo - would fetch and summarize papers)\n");

    println!("4️⃣  Synthesis Agent Combining Findings...");
    let synthesis = synthesis_agent.synthesize(query).await?;
    println!("   ✓ Synthesis complete\n");

    println!("📊 Final Answer:");
    println!("{}\n", synthesis);

    println!("💾 Memory Statistics:");
    let coordinator_shared = coordinator.agent().search_shared("research").await.unwrap_or_default();
    let web_shared = web_researcher.agent().search_shared("research").await.unwrap_or_default();
    let synthesis_shared = synthesis_agent.agent().search_shared("research").await.unwrap_or_default();
    
    println!("   Coordinator (shared): {} memories", coordinator_shared.len());
    println!("   Web Researcher (shared): {} memories", web_shared.len());
    println!("   Synthesis Agent (shared): {} memories", synthesis_shared.len());
    println!("   Total shared findings: {} memories", coordinator_shared.len() + web_shared.len() + synthesis_shared.len());

    println!("\n✨ Research Lab showcase complete!");
    println!("\nThis demonstrates:");
    println!("  • Multi-agent coordination");
    println!("  • Real tool integration (browser, web search)");
    println!("  • LLM-powered analysis and synthesis");
    println!("  • Memory management across agents");
    println!("  • Supervisor-ready architecture (agents can start/stop based on relevance)");

    Ok(())
}

