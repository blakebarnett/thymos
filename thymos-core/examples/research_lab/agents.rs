//! Research Lab Agents

use std::sync::Arc;
use thymos_core::prelude::*;
use thymos_core::config::{MemoryConfig, MemoryMode};
use thymos_core::llm::{LLMConfig, LLMProvider};
use thymos_core::error::ThymosError;
use super::tools::{BrowserTool, WebSearchTool};

/// Research Coordinator - Orchestrates research tasks
pub struct ResearchCoordinator {
    agent: Agent,
}

impl ResearchCoordinator {
    pub async fn new(llm: Arc<dyn LLMProvider>) -> Result<Self> {
        let shared_url = std::env::var("SHARED_LOCAI_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        
        let memory_config = MemoryConfig {
            mode: MemoryMode::Hybrid {
                private_data_dir: std::path::PathBuf::from("./data/research_lab/coordinator/private"),
                shared_url,
                shared_api_key: None,
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id("research_coordinator")
            .with_memory_config(memory_config)
            .llm_provider(llm)
            .build()
            .await?;

        Ok(Self { agent })
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub async fn plan_research(&self, query: &str) -> Result<String> {
        let prompt = format!(
            "You are a research coordinator. Break down this research query into sub-tasks:\n\nQuery: {}\n\nProvide a structured plan with specific tasks for literature review, web research, and synthesis.",
            query
        );

        let llm = self.agent.llm_provider().ok_or_else(|| {
            ThymosError::Configuration("LLM provider required for coordinator".to_string())
        })?;

        let config = LLMConfig {
            temperature: 0.7,
            max_tokens: 500,
            system_prompt: Some("You are a research coordinator that breaks down complex research queries into actionable tasks.".to_string()),
        };

        let plan = llm.generate(&prompt, &config).await?;
        
        let memory_content = format!("Research query: {}\nPlan: {}", query, plan);
        self.agent.remember_shared(memory_content).await?;
        
        Ok(plan)
    }
}

/// Literature Reviewer - Reviews and summarizes papers
pub struct LiteratureReviewer {
    agent: Agent,
    browser: BrowserTool,
}

impl LiteratureReviewer {
    pub async fn new(llm: Arc<dyn LLMProvider>, shared_memory_url: Option<String>) -> Result<Self> {
        let memory_config = if let Some(url) = shared_memory_url {
            MemoryConfig {
                mode: MemoryMode::Hybrid {
                    private_data_dir: std::path::PathBuf::from("./data/research_lab/literature_reviewer/private"),
                    shared_url: url,
                    shared_api_key: None,
                },
                ..Default::default()
            }
        } else {
            MemoryConfig {
                mode: MemoryMode::Embedded {
                    data_dir: std::path::PathBuf::from("./data/research_lab/literature_reviewer"),
                },
                ..Default::default()
            }
        };

        let agent = Agent::builder()
            .id("literature_reviewer")
            .with_memory_config(memory_config)
            .llm_provider(llm)
            .build()
            .await?;

        Ok(Self {
            agent,
            browser: BrowserTool::new().await?,
        })
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub async fn review_paper(&self, url: &str) -> Result<String> {
        let content_result = <BrowserTool as super::tools::Tool>::execute(&self.browser, serde_json::json!({
            "url": url
        })).await?;

        let content = content_result.content;
        
        let prompt = format!(
            "Summarize this research paper. Extract:\n1. Key findings\n2. Methodology\n3. Conclusions\n4. Related work mentioned\n\nPaper content:\n{}",
            &content[..content.len().min(4000)]
        );

        let llm = self.agent.llm_provider().ok_or_else(|| {
            ThymosError::Configuration("LLM provider required".to_string())
        })?;

        let config = LLMConfig {
            temperature: 0.5,
            max_tokens: 1000,
            system_prompt: Some("You are a literature reviewer that extracts key information from research papers.".to_string()),
        };

        let summary = llm.generate(&prompt, &config).await?;
        
        let memory_content = format!("Paper URL: {}\nSummary: {}", url, summary);
        self.agent.remember_shared(memory_content).await?;
        
        Ok(summary)
    }
}

/// Web Researcher - Conducts web research
pub struct WebResearcher {
    agent: Agent,
    search: WebSearchTool,
    browser: BrowserTool,
}

impl WebResearcher {
    pub async fn new(llm: Arc<dyn LLMProvider>, shared_memory_url: Option<String>) -> Result<Self> {
        let memory_config = if let Some(url) = shared_memory_url {
            MemoryConfig {
                mode: MemoryMode::Hybrid {
                    private_data_dir: std::path::PathBuf::from("./data/research_lab/web_researcher/private"),
                    shared_url: url,
                    shared_api_key: None,
                },
                ..Default::default()
            }
        } else {
            MemoryConfig {
                mode: MemoryMode::Embedded {
                    data_dir: std::path::PathBuf::from("./data/research_lab/web_researcher"),
                },
                ..Default::default()
            }
        };

        let agent = Agent::builder()
            .id("web_researcher")
            .with_memory_config(memory_config)
            .llm_provider(llm)
            .build()
            .await?;

        Ok(Self {
            agent,
            search: WebSearchTool::new().await?,
            browser: BrowserTool::new().await?,
        })
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub async fn research(&self, query: &str) -> Result<String> {
        let search_result = <WebSearchTool as super::tools::Tool>::execute(&self.search, serde_json::json!({
            "query": query,
            "max_results": 3
        })).await?;

        let mut findings = Vec::new();
        
        if let Some(results) = search_result.metadata.get("results").and_then(|v| v.as_array()) {
            for result in results.iter().take(3) {
                if let Some(url) = result.get("url").and_then(|v| v.as_str()) {
                    if let Ok(content_result) = <BrowserTool as super::tools::Tool>::execute(&self.browser, serde_json::json!({
                        "url": url
                    })).await {
                        let content = &content_result.content[..content_result.content.len().min(2000)];
                        findings.push(format!("Source: {}\nContent: {}", url, content));
                    }
                }
            }
        }

        let findings_text = if findings.is_empty() {
            return Err(ThymosError::Configuration(format!(
                "Web search returned {} results but failed to extract content from any URLs",
                search_result.metadata.get("result_count").and_then(|v| v.as_u64()).unwrap_or(0)
            )));
        } else {
            findings.join("\n\n---\n\n")
        };
        
        let prompt = format!(
            "Analyze these web search results and extract key insights related to: {}\n\nResults:\n{}",
            query, findings_text
        );

        let llm = self.agent.llm_provider().ok_or_else(|| {
            ThymosError::Configuration("LLM provider required".to_string())
        })?;

        let config = LLMConfig {
            temperature: 0.6,
            max_tokens: 800,
            system_prompt: Some("You are a web researcher that extracts insights from web sources.".to_string()),
        };

        let insights = llm.generate(&prompt, &config).await?;
        
        let memory_content = format!("Research query: {}\nInsights: {}", query, insights);
        self.agent.remember_shared(memory_content).await?;
        
        Ok(insights)
    }
}

/// Synthesis Agent - Synthesizes findings from multiple sources
pub struct SynthesisAgent {
    agent: Agent,
}

impl SynthesisAgent {
    pub async fn new(llm: Arc<dyn LLMProvider>, shared_memory_url: Option<String>) -> Result<Self> {
        let memory_config = if let Some(url) = shared_memory_url {
            MemoryConfig {
                mode: MemoryMode::Hybrid {
                    private_data_dir: std::path::PathBuf::from("./data/research_lab/synthesis/private"),
                    shared_url: url,
                    shared_api_key: None,
                },
                ..Default::default()
            }
        } else {
            MemoryConfig {
                mode: MemoryMode::Embedded {
                    data_dir: std::path::PathBuf::from("./data/research_lab/synthesis"),
                },
                ..Default::default()
            }
        };

        let agent = Agent::builder()
            .id("synthesis_agent")
            .with_memory_config(memory_config)
            .llm_provider(llm)
            .build()
            .await?;

        Ok(Self { agent })
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub async fn synthesize(&self, query: &str) -> Result<String> {
        let memories = self.agent.search_shared(query).await?;
        
        let findings: Vec<String> = memories
            .iter()
            .take(10)
            .map(|m| m.content.clone())
            .collect();

        if findings.is_empty() {
            return Ok("No findings available for synthesis. Other agents may not have completed their research yet.".to_string());
        }

        let findings_text = findings.join("\n\n---\n\n");
        
        let prompt = format!(
            "Synthesize these research findings into a comprehensive answer to: {}\n\nFindings:\n{}",
            query, findings_text
        );

        let llm = self.agent.llm_provider().ok_or_else(|| {
            ThymosError::Configuration("LLM provider required".to_string())
        })?;

        let config = LLMConfig {
            temperature: 0.7,
            max_tokens: 1500,
            system_prompt: Some("You are a synthesis agent that combines multiple research findings into comprehensive answers.".to_string()),
        };

        let synthesis = llm.generate(&prompt, &config).await?;
        
        let memory_content = format!("Synthesis for query: {}\nAnswer: {}", query, synthesis);
        self.agent.remember_shared(memory_content).await?;
        
        Ok(synthesis)
    }
}

