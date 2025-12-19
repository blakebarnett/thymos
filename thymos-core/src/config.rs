//! Configuration types for Thymos framework

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration for Thymos framework
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThymosConfig {
    /// Memory system configuration
    pub memory: MemoryConfig,

    /// Lifecycle management configuration
    pub lifecycle: LifecycleConfig,

    /// Event system configuration
    pub events: EventConfig,

    /// LLM provider configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LLMProviderConfig>,

    /// Embeddings provider configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<EmbeddingsConfig>,

    /// Pub/sub configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pubsub: Option<PubSubConfig>,
}

/// Memory system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Memory mode (embedded or server)
    pub mode: MemoryMode,

    /// Enable forgetting curve calculations
    pub forgetting_curve_enabled: bool,

    /// Hours for recency decay (Ebbinghaus curve)
    pub recency_decay_hours: f64,

    /// Weight given to access count in stability
    pub access_count_weight: f64,

    /// Multiplier for emotional/important memories
    pub emotional_weight_multiplier: f64,

    /// Base decay rate for old memories
    pub base_decay_rate: f64,

    /// Hybrid search configuration (optional)
    #[serde(default)]
    pub hybrid_search: Option<HybridSearchConfig>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            mode: MemoryMode::Embedded {
                data_dir: PathBuf::from("./data/memory"),
            },
            forgetting_curve_enabled: true,
            recency_decay_hours: 168.0, // 1 week
            access_count_weight: 0.1,
            emotional_weight_multiplier: 1.5,
            base_decay_rate: 0.01,
            hybrid_search: None,
        }
    }
}

/// Memory backend mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MemoryMode {
    /// Embedded Locai with local storage
    Embedded {
        /// Data directory for embedded storage
        data_dir: PathBuf,
    },

    /// Connect to Locai server
    Server {
        /// Server URL
        url: String,
        /// Optional API key
        api_key: Option<String>,
    },

    /// Hybrid mode: private embedded + shared server
    Hybrid {
        /// Private embedded storage
        private_data_dir: PathBuf,
        /// Shared server URL
        shared_url: String,
        /// Optional API key for shared server
        shared_api_key: Option<String>,
    },
}

/// Lifecycle management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Relevance threshold for active status (>= 0.7)
    pub threshold_active: f64,

    /// Relevance threshold for listening status (>= 0.4)
    pub threshold_listening: f64,

    /// Relevance threshold for dormant status (>= 0.1)
    pub threshold_dormant: f64,

    /// Startup timeout duration
    #[serde(with = "humantime_serde")]
    pub startup_timeout: Duration,

    /// Shutdown timeout duration
    #[serde(with = "humantime_serde")]
    pub shutdown_timeout: Duration,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            threshold_active: 0.7,
            threshold_listening: 0.4,
            threshold_dormant: 0.1,
            startup_timeout: Duration::from_secs(10),
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

/// Event system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// Enable event system
    pub enabled: bool,

    /// Event buffer size
    pub buffer_size: usize,

    /// Event database URL (SurrealDB)
    pub db_url: Option<String>,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 100,
            db_url: None, // Use embedded by default
        }
    }
}

/// Hybrid search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    /// Enable hybrid search (requires embeddings)
    pub enabled: bool,

    /// Semantic weight (0.0 = pure BM25, 1.0 = pure vector)
    /// Default: 0.3 (balanced)
    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f64,

    /// Embedding mode
    #[serde(default)]
    pub embedding_mode: EmbeddingMode,
}

fn default_semantic_weight() -> f64 {
    0.3
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            semantic_weight: 0.3,
            embedding_mode: EmbeddingMode::Auto,
        }
    }
}

/// Embedding generation mode
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EmbeddingMode {
    /// Auto-generate via server (requires locai-server with ML service)
    #[default]
    Auto,

    /// Manual provision (client provides embeddings)
    Manual {
        /// Optional embedding provider configuration for automatic generation
        /// If None, user must provide embeddings when creating memories
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<EmbeddingProviderConfig>,
    },
}

/// Embedding provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProviderConfig {
    /// Provider type
    pub provider: EmbeddingProvider,

    /// Model name
    pub model: String,

    /// API key (if needed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Embedding provider type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    OpenAI,
    Ollama,
    Local,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProviderConfig {
    /// Provider type
    pub provider: LLMProvider,

    /// Model name
    pub model: String,

    /// API key (if needed, prefer env vars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Base URL (for custom endpoints, e.g., Ollama)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// LLM provider type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    Groq,
    OpenAI,
    Ollama,
    Anthropic,
}

/// Embeddings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    /// Provider type
    pub provider: EmbeddingProvider,

    /// Model name
    pub model: String,

    /// API key (if needed, prefer env vars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Base URL (for custom endpoints, e.g., Ollama)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Pub/sub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubConfig {
    /// Pub/sub mode
    pub mode: PubSubMode,

    /// SurrealDB URL (for distributed/hybrid modes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_url: Option<String>,

    /// SurrealDB namespace
    #[serde(default = "default_pubsub_namespace")]
    pub namespace: String,

    /// SurrealDB database
    #[serde(default = "default_pubsub_database")]
    pub database: String,
}

fn default_pubsub_namespace() -> String {
    "thymos".to_string()
}

fn default_pubsub_database() -> String {
    "pubsub".to_string()
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            mode: PubSubMode::Local,
            distributed_url: None,
            namespace: default_pubsub_namespace(),
            database: default_pubsub_database(),
        }
    }
}

/// Pub/sub mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PubSubMode {
    /// Local-only (in-memory channels)
    Local,

    /// Distributed-only (SurrealDB)
    #[cfg(feature = "pubsub-distributed")]
    Distributed,

    /// Hybrid (both local and distributed)
    #[cfg(feature = "pubsub-distributed")]
    Hybrid,
}

/// Builder for ThymosConfig
pub struct ConfigBuilder {
    config: ThymosConfig,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            config: ThymosConfig::default(),
        }
    }

    /// Set memory configuration
    pub fn memory(mut self, config: MemoryConfig) -> Self {
        self.config.memory = config;
        self
    }

    /// Set lifecycle configuration
    pub fn lifecycle(mut self, config: LifecycleConfig) -> Self {
        self.config.lifecycle = config;
        self
    }

    /// Set event configuration
    pub fn events(mut self, config: EventConfig) -> Self {
        self.config.events = config;
        self
    }

    /// Build the configuration
    pub fn build(self) -> ThymosConfig {
        self.config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ThymosConfig {
    /// Load configuration from file and environment variables.
    ///
    /// Loads in this order:
    /// 1. Default configuration
    /// 2. Configuration file (thymos.toml or path from THYMOS_CONFIG_PATH)
    /// 3. Environment variable overrides
    ///
    /// # Errors
    ///
    /// Returns an error if configuration file is invalid or required env vars are missing.
    pub fn load() -> crate::error::Result<Self> {
        use figment::{
            Figment,
            providers::{Env, Format, Toml},
        };

        let mut figment = Figment::new()
            .merge(Toml::file("thymos.toml"))
            .merge(Env::prefixed("THYMOS_").split("_"));

        // Check for custom config path
        if let Ok(path) = std::env::var("THYMOS_CONFIG_PATH") {
            figment = figment.merge(Toml::file(path));
        }

        let config: ThymosConfig = figment.extract().map_err(|e| {
            crate::error::ThymosError::Configuration(format!("Failed to load configuration: {}", e))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a specific file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> crate::error::Result<Self> {
        use figment::{
            Figment,
            providers::{Format, Toml},
        };

        let config: ThymosConfig =
            Figment::new()
                .merge(Toml::file(path))
                .extract()
                .map_err(|e| {
                    crate::error::ThymosError::Configuration(format!(
                        "Failed to load configuration file: {}",
                        e
                    ))
                })?;

        config.validate()?;
        Ok(config)
    }

    /// Apply environment variable overrides to the configuration.
    #[allow(dead_code)]
    fn apply_env_overrides(&mut self) -> crate::error::Result<()> {
        // Override LLM provider from env
        if let Ok(provider_str) = std::env::var("THYMOS_LLM_PROVIDER") {
            let provider = match provider_str.to_lowercase().as_str() {
                "groq" => LLMProvider::Groq,
                "openai" => LLMProvider::OpenAI,
                "ollama" => LLMProvider::Ollama,
                "anthropic" => LLMProvider::Anthropic,
                _ => {
                    return Err(crate::error::ThymosError::Configuration(format!(
                        "Invalid LLM provider: {}",
                        provider_str
                    )));
                }
            };

            // For Groq, prefer GROQ_MODEL, fallback to THYMOS_LLM_MODEL, then default
            let model = match provider {
                LLMProvider::Groq => {
                    std::env::var("GROQ_MODEL")
                        .or_else(|_| std::env::var("THYMOS_LLM_MODEL"))
                        .unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string())
                }
                _ => {
                    std::env::var("THYMOS_LLM_MODEL")
                        .unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string())
                }
            };

            self.llm = Some(LLMProviderConfig {
                provider,
                model,
                api_key: None, // API keys should come from provider-specific env vars
                base_url: std::env::var("THYMOS_LLM_BASE_URL").ok(),
            });
        }

        // Override embeddings provider from env
        if let Ok(provider_str) = std::env::var("THYMOS_EMBEDDINGS_PROVIDER") {
            let provider = match provider_str.to_lowercase().as_str() {
                "openai" => EmbeddingProvider::OpenAI,
                "ollama" => EmbeddingProvider::Ollama,
                "local" => EmbeddingProvider::Local,
                _ => {
                    return Err(crate::error::ThymosError::Configuration(format!(
                        "Invalid embeddings provider: {}",
                        provider_str
                    )));
                }
            };

            let model = std::env::var("THYMOS_EMBEDDINGS_MODEL")
                .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());

            self.embeddings = Some(EmbeddingsConfig {
                provider,
                model,
                api_key: None,
                base_url: std::env::var("THYMOS_EMBEDDINGS_BASE_URL").ok(),
            });
        }

        Ok(())
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    fn validate(&self) -> crate::error::Result<()> {
        // Add validation logic here
        Ok(())
    }
}
