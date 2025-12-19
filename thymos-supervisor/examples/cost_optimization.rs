//! Cost Optimization Example
//!
//! Demonstrates cost optimization for expensive LLM agents by automatically
//! managing agent lifecycle based on relevance and usage patterns.

use thymos_supervisor::SupervisorConfig;
use thymos_core::{
    lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceScore, RelevanceThresholds},
    error::Result,
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Evaluates relevance based on agent usage and cost factors
struct CostOptimizedRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for CostOptimizedRelevanceEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        // Get usage metrics
        let last_used_hours: u32 = context.get("last_used_hours").unwrap_or(999);
        let active_sessions: usize = context.get("active_sessions").unwrap_or(0);
        let cost_per_hour: f64 = context.get("cost_per_hour").unwrap_or(1.0);
        let priority: f64 = context.get("priority").unwrap_or(0.5);
        
        // Calculate relevance based on usage and cost
        let score = if active_sessions > 0 {
            // Active sessions - keep running
            1.0
        } else if last_used_hours < 1 {
            // Recently used - keep warm
            0.8
        } else if last_used_hours < 6 && priority > 0.7 {
            // High priority, recently used
            0.6
        } else if cost_per_hour < 0.1 {
            // Low cost - keep running longer
            if last_used_hours < 24 {
                0.4
            } else {
                0.1
            }
        } else {
            // High cost - stop quickly when idle
            if last_used_hours < 2 {
                0.3
            } else {
                0.0
            }
        };
        
        println!(
            "  Agent {}: last_used={}h, sessions={}, cost=${}/h, priority={:.2} → score={:.2}",
            agent_id, last_used_hours, active_sessions, cost_per_hour, priority, score
        );
        
        Ok(RelevanceScore::new(score))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Cost Optimization Example for Expensive LLM Agents");
    println!("==================================================\n");
    
    // Configure supervisor
    let _config = SupervisorConfig {
        agent_binary: PathBuf::from("./target/release/llm-agent"),
        port_start: 4000,
        startup_timeout: Duration::from_secs(15),
        shutdown_timeout: Duration::from_secs(10),
        working_dir: None,
    };
    
    println!("Creating supervisor...");
    println!("(Note: This example requires a compiled agent binary)");
    println!();
    
    // Create lifecycle manager
    let evaluator = Arc::new(CostOptimizedRelevanceEvaluator);
    let thresholds = RelevanceThresholds::default();
    
    println!("Simulating agent cost optimization scenarios:\n");
    
    // Scenario 1: Active agent with sessions
    println!("Scenario 1: Active agent with active sessions");
    let mut context = RelevanceContext::new();
    context.set("last_used_hours", 0u32);
    context.set("active_sessions", 3usize);
    context.set("cost_per_hour", 2.5f64);
    context.set("priority", 0.8f64);
    
    let score = evaluator.evaluate("agent_expensive_llm", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    // Scenario 2: High-cost idle agent
    println!("Scenario 2: High-cost agent idle for 3 hours");
    let mut context = RelevanceContext::new();
    context.set("last_used_hours", 3u32);
    context.set("active_sessions", 0usize);
    context.set("cost_per_hour", 5.0f64);
    context.set("priority", 0.5f64);
    
    let score = evaluator.evaluate("agent_expensive_llm", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    // Scenario 3: Low-cost agent
    println!("Scenario 3: Low-cost agent idle for 12 hours");
    let mut context = RelevanceContext::new();
    context.set("last_used_hours", 12u32);
    context.set("active_sessions", 0usize);
    context.set("cost_per_hour", 0.05f64);
    context.set("priority", 0.6f64);
    
    let score = evaluator.evaluate("agent_cheap", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    println!("Benefits:");
    println!("  • Automatically stop expensive idle agents");
    println!("  • Keep low-cost agents running longer");
    println!("  • Prioritize agents based on usage patterns");
    println!("  • Reduce infrastructure costs significantly");
    
    Ok(())
}

