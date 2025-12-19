//! Researcher agent for multi-agent coordination example

use thymos_core::config::MemoryMode;
use thymos_core::prelude::*;

/// Researcher agent that gathers information and stores it in shared memory
pub struct ResearcherAgent {
    /// Underlying agent
    pub agent: Agent,

    /// Research topic
    pub topic: String,
}

impl ResearcherAgent {
    /// Create a new researcher agent
    pub async fn new(
        id: impl Into<String>,
        topic: impl Into<String>,
        shared_url: &str,
    ) -> Result<Self> {
        let id = id.into();
        let topic = topic.into();

        // Configure hybrid memory with shared server
        let memory_config = MemoryConfig {
            mode: MemoryMode::Hybrid {
                private_data_dir: std::path::PathBuf::from(format!("./data/multi_agent/{}", id)),
                shared_url: shared_url.to_string(),
                shared_api_key: None,
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id(&id)
            .with_memory_config(memory_config)
            .build()
            .await?;

        Ok(Self { agent, topic })
    }

    /// Gather information about the research topic (simulated)
    pub async fn gather_information(&self) -> Result<String> {
        // Simulate information gathering
        let findings = format!(
            "Research finding about {}: Recent developments suggest significant progress in this area.",
            self.topic
        );
        Ok(findings)
    }

    /// Calculate confidence in research findings (simulated)
    pub fn calculate_confidence(&self, _finding: &str) -> f64 {
        // Simulated confidence calculation
        0.85
    }

    /// Conduct research and store findings
    pub async fn research(&self) -> Result<()> {
        // Gather information
        let finding = self.gather_information().await?;

        // Store in shared memory
        self.agent
            .remember_shared(&format!("Finding about {}: {}", self.topic, finding))
            .await?;

        // Store private note about confidence
        let confidence = self.calculate_confidence(&finding);
        self.agent
            .remember_private(&format!(
                "Research confidence for {}: {:.2}",
                self.topic, confidence
            ))
            .await?;

        Ok(())
    }
}
