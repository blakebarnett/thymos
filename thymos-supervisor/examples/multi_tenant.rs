//! Multi-Tenant SaaS Example
//!
//! Demonstrates cost optimization by automatically starting/stopping
//! agent processes based on customer activity.

use thymos_supervisor::SupervisorConfig;
use thymos_core::{
    lifecycle::{RelevanceContext, RelevanceEvaluator, RelevanceScore, RelevanceThresholds},
    error::Result,
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Evaluates relevance based on customer activity
struct CustomerRelevanceEvaluator;

#[async_trait]
impl RelevanceEvaluator for CustomerRelevanceEvaluator {
    async fn evaluate(&self, agent_id: &str, context: &RelevanceContext) -> Result<RelevanceScore> {
        let customer_id = agent_id.strip_prefix("customer_").unwrap_or(agent_id);
        
        // Get customer activity metrics
        let last_activity_days: u32 = context.get("last_activity_days").unwrap_or(999);
        let subscription_active: bool = context.get("subscription_active").unwrap_or(false);
        let pending_tasks: usize = context.get("pending_tasks").unwrap_or(0);
        
        let score = if !subscription_active {
            // Archived - subscription cancelled
            0.0
        } else if last_activity_days < 1 {
            // Active - customer used service today
            1.0
        } else if last_activity_days < 7 && pending_tasks > 0 {
            // Listening - recent activity with pending work
            0.6
        } else if last_activity_days < 30 {
            // Dormant - inactive but keep ready
            0.2
        } else {
            // Archived - inactive too long
            0.0
        };
        
        println!(
            "  Customer {}: last_activity={} days, active={}, pending={} → score={:.2}",
            customer_id, last_activity_days, subscription_active, pending_tasks, score
        );
        
        Ok(RelevanceScore::new(score))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Multi-Tenant SaaS Cost Optimization Example");
    println!("============================================\n");
    
    // Configure supervisor
    let _config = SupervisorConfig {
        agent_binary: PathBuf::from("./target/release/customer-agent"),
        port_start: 3000,
        startup_timeout: Duration::from_secs(10),
        shutdown_timeout: Duration::from_secs(5),
        working_dir: None,
    };
    
    // Note: In a real example, you'd create the supervisor
    // For this demo, we'll just show the reconciliation logic
    println!("Creating supervisor...");
    println!("(Note: This example requires a compiled agent binary)");
    println!();
    
    // Create lifecycle manager
    let evaluator = Arc::new(CustomerRelevanceEvaluator);
    let thresholds = RelevanceThresholds::default();
    
    println!("Simulating customer activity scenarios:\n");
    
    // Scenario 1: Active customer
    println!("Scenario 1: Active customer (used service today)");
    let mut context = RelevanceContext::new();
    context.set("last_activity_days", 0u32);
    context.set("subscription_active", true);
    context.set("pending_tasks", 5usize);
    
    let score = evaluator.evaluate("customer_123", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    // Scenario 2: Inactive customer
    println!("Scenario 2: Inactive customer (45 days inactive)");
    let mut context = RelevanceContext::new();
    context.set("last_activity_days", 45u32);
    context.set("subscription_active", true);
    context.set("pending_tasks", 0usize);
    
    let score = evaluator.evaluate("customer_456", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    // Scenario 3: Cancelled subscription
    println!("Scenario 3: Cancelled subscription");
    let mut context = RelevanceContext::new();
    context.set("last_activity_days", 5u32);
    context.set("subscription_active", false);
    context.set("pending_tasks", 0usize);
    
    let score = evaluator.evaluate("customer_789", &context).await?;
    println!("  → Relevance score: {:.2} → Status: {:?}\n", 
        score.value(), score.to_status(&thresholds));
    
    println!("Benefits:");
    println!("  • Only run agents for active customers");
    println!("  • Automatically stop agents for inactive customers");
    println!("  • Save compute costs on inactive subscriptions");
    println!("  • Scale resources based on actual usage");
    
    Ok(())
}

