# Agent Framework Design Document

**Project Name**: Thymos  
**Tagline**: *The animating spirit for intelligent agents*  
**Date**: November 6, 2025  
**Status**: 🔄 Design Phase  
**Target Language**: Rust  
**License**: MIT / Apache 2.0 (dual)

---

## Executive Summary

**Thymos** (Θυμός) is a domain-agnostic agent framework for building autonomous agents with:
- Semantic memory (via embedded Locai)
- Temporal memory decay and lifecycle management
- Concept extraction and entity tracking
- Event-driven coordination
- Automatic agent lifecycle management based on relevance
- MCP (Model Context Protocol) interface for LLM integration

### Key Features

- 🧠 **Memory-First Architecture**: Built on Locai for semantic memory
- ⏰ **Temporal Awareness**: Forgetting curves, recency, and consolidation
- 🔍 **Concept Extraction**: Domain-agnostic entity/concept identification
- 🎭 **Multi-Agent Coordination**: Event-driven agent-to-agent communication
- 🔄 **Lifecycle Management**: Automatic start/stop based on relevance criteria
- 🌐 **MCP Native**: First-class MCP server implementation
- 💾 **State Persistence**: Graceful shutdown and state restoration
- 🦀 **Rust Performance**: Efficient, safe, and embeddable

### Etymology

**Thymos** (θυμός) - Ancient Greek for "spirit," "soul," or "life-force." In Homeric psychology, thymos was the seat of emotion, thought, and motivation—the animating principle that drives action. Where **Locai** provides memory and place (*loci*), **Thymos** provides agency and animation.

> *"Locai remembers. Thymos acts."*

---

## Motivation: Lessons from Zera

This framework abstracts patterns learned from building Zera, a narrative RPG:

### Generalizable Patterns Identified

| Pattern | Zera Implementation | Generic Abstraction |
|---------|---------------------|---------------------|
| Entity tracking | Characters, Locations, Items | Concept extraction and significance scoring |
| Memory lifecycle | Forgetting curve for game events | Temporal decay with configurable parameters |
| Entity promotion | Tier 1→2→3 promotion pipeline | Importance-based concept hierarchy |
| Session consolidation | Post-session dream state | Periodic memory consolidation |
| Hook system | Memory creation/update hooks | Event-driven enrichment pipeline |
| NPC autonomy | Independent character agents | Agent relevance and lifecycle management |
| Alias resolution | "old badger" → Elder Rowan | Semantic reference resolution |

### Domain-Specific vs Generic

**Domain-Specific (stays in Zera)**:
- D&D mechanics, combat systems
- Dungeon Master narrative generation
- RPG-specific prompts and schemas
- Game-specific character attributes

**Domain-Agnostic (moves to Memoria)**:
- Memory strength calculation
- Concept significance evaluation
- Event hooks and enrichment
- Agent lifecycle management
- MCP server implementation
- State persistence patterns

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│              Application Layer                      │
│  (Domain-specific: Zera, ChatBot, Assistant, etc.) │
└─────────────────────────────────────────────────────┘
                      │ MCP / gRPC
        ┌─────────────┼─────────────┐
        │             │             │
   ┌────▼────┐   ┌───▼────┐   ┌───▼────┐
   │ Agent 1 │   │Agent 2 │   │Agent 3 │
   │   MCP   │   │  MCP   │   │  MCP   │
   └─────────┘   └────────┘   └────────┘
        │             │             │
   ┌────▼─────────────▼─────────────▼───┐
   │        Thymos Framework Core       │
   │  ┌─────────────────────────────┐  │
   │  │   Memory Lifecycle Manager  │  │
   │  ├─────────────────────────────┤  │
   │  │   Concept Extractor         │  │
   │  ├─────────────────────────────┤  │
   │  │   Event System / Hooks      │  │
   │  ├─────────────────────────────┤  │
   │  │   Consolidation Engine      │  │
   │  ├─────────────────────────────┤  │
   │  │   Relevance Evaluator       │  │
   │  └─────────────────────────────┘  │
   └────────────────────────────────────┘
        │             │             │
   ┌────▼────┐   ┌───▼──────┐  ┌──▼──────┐
   │ Locai   │   │SurrealDB │  │ Event   │
   │Embedded │   │Embedded  │  │ Stream  │
   └─────────┘   └──────────┘  └─────────┘
        │             │             │
   ┌────▼─────────────▼─────────────▼───┐
   │        Shared Services              │
   │  - Locai API (shared memory)       │
   │  - SurrealDB Live Queries (events) │
   └────────────────────────────────────┘
```

---

## Core Abstractions

### 1. Memory System

```rust
/// Core memory abstraction wrapping Locai
pub struct MemorySystem {
    locai: EmbeddedLocai,
    lifecycle: MemoryLifecycle,
}

pub struct MemoryLifecycle {
    config: LifecycleConfig,
}

#[derive(Debug, Clone)]
pub struct LifecycleConfig {
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
}

impl MemoryLifecycle {
    /// Calculate memory strength using forgetting curve
    pub fn calculate_strength(&self, memory: &Memory) -> f64 {
        // R = e^(-t/S)
        // Where t = time since last access, S = stability
        
        let hours_since_access = self.hours_since_access(memory);
        let stability = self.calculate_stability(memory);
        
        let time_decay = (-hours_since_access / stability).exp();
        let age_decay = self.age_decay(memory);
        
        (time_decay * age_decay).clamp(0.0, 1.0)
    }
    
    /// Calculate memory stability (resistance to forgetting)
    fn calculate_stability(&self, memory: &Memory) -> f64 {
        let mut stability = 1.0;
        
        // More access = more stable
        stability += memory.access_count as f64 * self.config.access_count_weight;
        
        // Emotional weight increases stability
        stability *= memory.emotional_weight.unwrap_or(1.0) 
            * self.config.emotional_weight_multiplier;
        
        // Explicit importance score
        stability *= memory.importance_score.unwrap_or(1.0);
        
        stability
    }
}
```

### 2. Concept Extraction

```rust
/// Domain-agnostic concept extraction from text
pub trait ConceptExtractor: Send + Sync {
    /// Extract concepts from text with significance scores
    async fn extract(
        &self, 
        text: &str, 
        context: Option<&Context>
    ) -> Result<Vec<Concept>>;
    
    /// Validate extracted concepts using LLM
    async fn validate(
        &self,
        concepts: &[Concept],
        text: &str,
        context: Option<&Context>
    ) -> Result<Vec<ValidatedConcept>>;
}

#[derive(Debug, Clone)]
pub struct Concept {
    /// The concept text as extracted
    pub text: String,
    
    /// Concept type (customizable per domain)
    pub concept_type: String,
    
    /// Brief contextual description
    pub context: String,
    
    /// Significance score (0.0-1.0)
    pub significance: f64,
    
    /// Whether this meets significance threshold
    pub is_significant: bool,
    
    /// Alternate names/references for this concept
    pub aliases: Vec<Alias>,
}

#[derive(Debug, Clone)]
pub struct Alias {
    pub text: String,
    pub confidence: f64,
    pub alias_type: AliasType,
    pub provenance: AliasProvenance,
}

#[derive(Debug, Clone)]
pub enum AliasType {
    Epithet,      // "the old badger"
    Alias,        // "aka John Smith"
    Title,        // "Dr.", "Captain"
    Descriptor,   // "the tall one"
}

#[derive(Debug, Clone)]
pub enum AliasProvenance {
    SelfReference,   // Entity referred to itself
    OtherReference,  // Someone else used this name
    Narrator,        // Third-person description
}

/// Hierarchical concept importance (Zera's tier system, generalized)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConceptTier {
    /// Mentioned once, low significance
    Mentioned = 1,
    
    /// Multiple mentions or medium significance
    Provisional = 2,
    
    /// High significance, tracked persistently
    Tracked = 3,
}

pub struct ConceptPromotionPipeline {
    extractor: Arc<dyn ConceptExtractor>,
    memory: Arc<MemorySystem>,
    config: PromotionConfig,
}

#[derive(Debug, Clone)]
pub struct PromotionConfig {
    /// Significance threshold for promotion (0.6 default)
    pub promotion_threshold: f64,
    
    /// Minimum mentions before considering promotion
    pub min_mentions: usize,
    
    /// Use LLM for validation
    pub use_llm_validation: bool,
}

impl ConceptPromotionPipeline {
    /// Track concept mention
    pub async fn track_mention(
        &self,
        concept_ref: &str,
        concept_type: &str,
        memory_id: &str,
    ) -> Result<()> {
        // Store mention metadata
        // Evaluate for promotion
    }
    
    /// Evaluate if concept should be promoted to higher tier
    pub async fn evaluate_promotion(
        &self,
        concept_ref: &str,
        concept_type: &str,
    ) -> Result<PromotionDecision> {
        // Calculate significance score
        // Check mention count
        // Optional LLM validation
        // Return promotion decision
    }
    
    /// Promote concept to tracked status
    pub async fn promote(
        &self,
        concept_ref: &str,
        concept_type: &str,
        tier: ConceptTier,
        metadata: ConceptMetadata,
    ) -> Result<String> {
        // Create concept profile in memory system
        // Link source memories
        // Store aliases
    }
}
```

### 3. Event System

```rust
/// Event-driven hooks for memory operations
pub trait MemoryHook: Send + Sync {
    async fn on_memory_created(&self, memory: &Memory) -> Result<()>;
    async fn on_memory_updated(&self, memory: &Memory) -> Result<()>;
    async fn on_memory_accessed(&self, memory: &Memory) -> Result<()>;
}

pub struct HookRegistry {
    hooks: Vec<Arc<dyn MemoryHook>>,
}

impl HookRegistry {
    pub fn register(&mut self, hook: Arc<dyn MemoryHook>) {
        self.hooks.push(hook);
    }
    
    pub async fn trigger_created(&self, memory: &Memory) -> Result<()> {
        for hook in &self.hooks {
            hook.on_memory_created(memory).await?;
        }
        Ok(())
    }
}

/// Example hook: Auto-extract concepts on memory creation
pub struct ConceptExtractionHook {
    pipeline: Arc<ConceptPromotionPipeline>,
}

#[async_trait]
impl MemoryHook for ConceptExtractionHook {
    async fn on_memory_created(&self, memory: &Memory) -> Result<()> {
        // Extract concepts from memory content
        let concepts = self.pipeline.extractor
            .extract(&memory.content, None)
            .await?;
        
        // Track each significant concept
        for concept in concepts.iter().filter(|c| c.is_significant) {
            self.pipeline.track_mention(
                &concept.text,
                &concept.concept_type,
                &memory.id,
            ).await?;
        }
        
        Ok(())
    }
}

/// Live query event system (SurrealDB)
pub struct EventStream {
    db: SurrealDB,
}

impl EventStream {
    /// Subscribe to events matching query
    pub async fn subscribe(
        &self,
        query: &str,
        handler: impl EventHandler + Send + 'static,
    ) -> Result<EventSubscription> {
        // Set up SurrealDB LIVE SELECT
        // Route events to handler
    }
}

pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: Event) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub source: String,
    pub tags: Vec<String>,
}
```

### 4. Consolidation Engine

```rust
/// Periodic memory consolidation system
pub struct ConsolidationEngine {
    memory: Arc<MemorySystem>,
    llm: Arc<dyn LLMProvider>,
    config: ConsolidationConfig,
}

#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Minimum memories needed to trigger consolidation
    pub min_memories: usize,
    
    /// Time period to consolidate (e.g., "last session")
    pub consolidation_window: Duration,
    
    /// Batch size for LLM processing
    pub batch_size: usize,
}

impl ConsolidationEngine {
    /// Run consolidation for a time period
    pub async fn consolidate(
        &self,
        scope: ConsolidationScope,
    ) -> Result<ConsolidationResult> {
        // 1. Fetch memories in scope
        let memories = self.fetch_memories(&scope).await?;
        
        if memories.len() < self.config.min_memories {
            return Ok(ConsolidationResult::Skipped);
        }
        
        // 2. AI-driven consolidation
        let insights = self.generate_insights(&memories).await?;
        
        // 3. Create consolidated memories
        let consolidated_ids = self.create_consolidated_memories(&insights).await?;
        
        // 4. Update importance scores
        self.update_importance_scores(&memories, &insights).await?;
        
        // 5. Identify important concepts
        let important_concepts = self.identify_important_concepts(&insights).await?;
        
        Ok(ConsolidationResult::Success {
            consolidated_count: consolidated_ids.len(),
            insights,
            important_concepts,
        })
    }
    
    async fn generate_insights(
        &self,
        memories: &[Memory],
    ) -> Result<Vec<Insight>> {
        // Use LLM to extract:
        // - Key themes
        // - Important concepts/entities
        // - Relationships
        // - Emotional moments
        // - Contradictions or inconsistencies
    }
}

#[derive(Debug)]
pub enum ConsolidationScope {
    /// Consolidate specific session/episode
    Session(String),
    
    /// Consolidate time range
    TimeRange { start: DateTime<Utc>, end: DateTime<Utc> },
    
    /// Consolidate by tag/category
    Tagged(String),
}

#[derive(Debug)]
pub struct Insight {
    pub insight_type: InsightType,
    pub summary: String,
    pub source_memory_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum InsightType {
    Theme,
    Pattern,
    Relationship,
    ImportantConcept,
    EmotionalEvent,
    Contradiction,
}
```

### 5. Agent Lifecycle Management

```rust
/// Agent lifecycle and relevance-based activation
pub struct Agent {
    pub id: String,
    pub memory: Arc<MemorySystem>,
    pub state: AgentState,
    pub config: AgentConfig,
    
    // Private state (embedded SurrealDB)
    private_db: SurrealDB,
    
    // Event subscriptions
    event_stream: EventStream,
}

#[derive(Debug, Clone)]
pub struct AgentState {
    pub status: AgentStatus,
    pub mode: AgentMode,
    pub started_at: Option<DateTime<Utc>>,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Active,      // Running and participating
    Listening,   // Running but passive
    Dormant,     // Stopped, state saved
    Archived,    // Long-term storage
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentMode {
    /// Actively responding to interactions
    Active,
    
    /// Listening to events, updating state, but not responding
    Passive,
}

/// Evaluates agent relevance based on context
pub trait RelevanceEvaluator: Send + Sync {
    /// Calculate relevance score for agent
    async fn evaluate(
        &self,
        agent_id: &str,
        context: &RelevanceContext,
    ) -> Result<RelevanceScore>;
}

#[derive(Debug, Clone)]
pub struct RelevanceContext {
    /// Domain-specific context (extensible)
    pub properties: HashMap<String, serde_json::Value>,
}

impl RelevanceContext {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }
    
    pub fn set(&mut self, key: impl Into<String>, value: impl Serialize) {
        self.properties.insert(
            key.into(),
            serde_json::to_value(value).unwrap()
        );
    }
    
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.properties.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RelevanceScore(f64);

impl RelevanceScore {
    pub fn new(score: f64) -> Self {
        Self(score.clamp(0.0, 1.0))
    }
    
    pub fn value(&self) -> f64 {
        self.0
    }
    
    pub fn to_status(&self, thresholds: &RelevanceThresholds) -> AgentStatus {
        if self.0 >= thresholds.active {
            AgentStatus::Active
        } else if self.0 >= thresholds.listening {
            AgentStatus::Listening
        } else if self.0 >= thresholds.dormant {
            AgentStatus::Dormant
        } else {
            AgentStatus::Archived
        }
    }
}

#[derive(Debug, Clone)]
pub struct RelevanceThresholds {
    pub active: f64,      // >= 0.7: must be active
    pub listening: f64,   // >= 0.4: should be listening
    pub dormant: f64,     // >= 0.1: keep in dormant state
    // < 0.1: archive
}

/// Manages agent lifecycle (start/stop/transition)
pub struct AgentLifecycleManager {
    supervisor: Arc<dyn AgentSupervisor>,
    evaluator: Arc<dyn RelevanceEvaluator>,
    thresholds: RelevanceThresholds,
}

impl AgentLifecycleManager {
    /// Reconcile agent states based on current context
    pub async fn reconcile(
        &self,
        context: &RelevanceContext,
    ) -> Result<ReconciliationReport> {
        let mut report = ReconciliationReport::default();
        
        // Get all known agents
        let agents = self.supervisor.list_agents().await?;
        
        for agent_id in agents {
            let relevance = self.evaluator.evaluate(&agent_id, context).await?;
            let current_status = self.supervisor.get_status(&agent_id).await?;
            let desired_status = relevance.to_status(&self.thresholds);
            
            if current_status != desired_status {
                self.transition_agent(
                    &agent_id,
                    current_status,
                    desired_status,
                    context,
                    &mut report,
                ).await?;
            }
        }
        
        Ok(report)
    }
    
    async fn transition_agent(
        &self,
        agent_id: &str,
        from: AgentStatus,
        to: AgentStatus,
        context: &RelevanceContext,
        report: &mut ReconciliationReport,
    ) -> Result<()> {
        match (from, to) {
            (AgentStatus::Dormant, AgentStatus::Active) => {
                self.supervisor.start(agent_id, AgentMode::Active, context).await?;
                report.started.push(agent_id.to_string());
            }
            
            (AgentStatus::Active, AgentStatus::Dormant) => {
                self.supervisor.stop(agent_id, true).await?;
                report.stopped.push(agent_id.to_string());
            }
            
            (AgentStatus::Active, AgentStatus::Listening) => {
                self.supervisor.set_mode(agent_id, AgentMode::Passive).await?;
                report.downgraded.push(agent_id.to_string());
            }
            
            (AgentStatus::Listening, AgentStatus::Active) => {
                self.supervisor.set_mode(agent_id, AgentMode::Active).await?;
                report.upgraded.push(agent_id.to_string());
            }
            
            _ => {} // Other transitions handled similarly
        }
        
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ReconciliationReport {
    pub started: Vec<String>,
    pub stopped: Vec<String>,
    pub upgraded: Vec<String>,
    pub downgraded: Vec<String>,
}
```

### 6. Agent Supervisor

```rust
/// Supervisor for managing agent processes
#[async_trait]
pub trait AgentSupervisor: Send + Sync {
    /// Start an agent
    async fn start(
        &self,
        agent_id: &str,
        mode: AgentMode,
        context: &RelevanceContext,
    ) -> Result<AgentHandle>;
    
    /// Stop an agent gracefully
    async fn stop(&self, agent_id: &str, save_state: bool) -> Result<()>;
    
    /// Get current agent status
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus>;
    
    /// Set agent mode (active/passive)
    async fn set_mode(&self, agent_id: &str, mode: AgentMode) -> Result<()>;
    
    /// List all known agents
    async fn list_agents(&self) -> Result<Vec<String>>;
    
    /// Health check on agent
    async fn health_check(&self, agent_id: &str) -> Result<HealthStatus>;
}

pub struct AgentHandle {
    pub agent_id: String,
    pub pid: u32,
    pub port: u16,
}

/// Process-based supervisor (subprocess management)
pub struct ProcessSupervisor {
    processes: Arc<RwLock<HashMap<String, Child>>>,
    config: SupervisorConfig,
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Path to agent binary
    pub agent_binary: PathBuf,
    
    /// Starting port for agents
    pub port_start: u16,
    
    /// Startup timeout
    pub startup_timeout: Duration,
    
    /// Shutdown timeout before force kill
    pub shutdown_timeout: Duration,
}

impl ProcessSupervisor {
    pub async fn new(config: SupervisorConfig) -> Result<Self> {
        Ok(Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }
    
    async fn spawn_process(
        &self,
        agent_id: &str,
        mode: AgentMode,
        context: &RelevanceContext,
    ) -> Result<Child> {
        let port = self.allocate_port().await?;
        
        // Write context to temp file
        let context_file = self.write_context(agent_id, context).await?;
        
        let mut cmd = Command::new(&self.config.agent_binary);
        cmd.arg("--agent-id").arg(agent_id)
           .arg("--port").arg(port.to_string())
           .arg("--mode").arg(mode.to_string())
           .arg("--context").arg(context_file);
        
        let child = cmd.spawn()?;
        
        // Wait for agent to be ready
        self.wait_for_ready(port, self.config.startup_timeout).await?;
        
        Ok(child)
    }
}

/// systemd-based supervisor (production)
pub struct SystemdSupervisor {
    bus: Connection,
}

// Additional supervisor implementations...
```

### 7. MCP Server Interface

```rust
/// MCP server implementation for agents
pub struct AgentMcpServer {
    agent: Arc<Agent>,
}

#[async_trait]
impl McpServer for AgentMcpServer {
    async fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: format!("talk_to_{}", self.agent.id),
                description: format!("Engage {} in dialogue", self.agent.id),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "speaker_id": {"type": "string"},
                        "message": {"type": "string"},
                        "context": {"type": "object"}
                    },
                    "required": ["speaker_id", "message"]
                }),
            },
            Tool {
                name: format!("query_{}_knowledge", self.agent.id),
                description: format!("Query {}'s knowledge", self.agent.id),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: format!("get_{}_state", self.agent.id),
                description: format!("Get {}'s current state", self.agent.id),
                input_schema: json!({"type": "object"}),
            },
        ]
    }
    
    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolResult, McpError> {
        match name {
            name if name.starts_with("talk_to_") => {
                self.handle_dialogue(arguments).await
            }
            name if name.starts_with("query_") => {
                self.handle_knowledge_query(arguments).await
            }
            name if name.starts_with("get_") && name.ends_with("_state") => {
                self.get_state().await
            }
            _ => Err(McpError::UnknownTool(name.to_string())),
        }
    }
    
    async fn list_resources(&self) -> Vec<Resource> {
        vec![
            Resource {
                uri: format!("agent://{}/state", self.agent.id),
                name: format!("{}'s State", self.agent.id),
                mime_type: Some("application/json".into()),
                description: Some("Current agent state and status".into()),
            },
            Resource {
                uri: format!("agent://{}/memories", self.agent.id),
                name: format!("{}'s Memories", self.agent.id),
                mime_type: Some("application/json".into()),
                description: Some("Recent memories".into()),
            },
        ]
    }
    
    async fn read_resource(&self, uri: &str) -> Result<ResourceContent, McpError> {
        match uri {
            uri if uri.ends_with("/state") => {
                let state = self.agent.get_state().await?;
                Ok(ResourceContent::Text(serde_json::to_string(&state)?))
            }
            uri if uri.ends_with("/memories") => {
                let memories = self.agent.memory.search("*", 10).await?;
                Ok(ResourceContent::Text(serde_json::to_string(&memories)?))
            }
            _ => Err(McpError::UnknownResource(uri.to_string())),
        }
    }
}

impl AgentMcpServer {
    async fn handle_dialogue(&self, args: Value) -> Result<ToolResult, McpError> {
        let req: DialogueRequest = serde_json::from_value(args)?;
        
        // Agent processes dialogue
        let response = self.agent.process_dialogue(req).await?;
        
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: response.message,
            }],
            is_error: false,
        })
    }
}
```

---

## API Surface

### Application-Facing API

Applications interact with Memoria in three ways:

#### 1. MCP Client (Primary Interface)

```python
# Connect to agent via MCP
from mcp import Client

client = Client("http://localhost:3000")
await client.initialize()

# Call agent tools
response = await client.call_tool(
    "talk_to_agent",
    {
        "speaker_id": "user_123",
        "message": "Hello!",
        "context": {"session_id": "sess_1"}
    }
)

# Read agent resources
state = await client.read_resource("agent://my_agent/state")
```

#### 2. gRPC API (Optional, for non-MCP clients)

```protobuf
service AgentService {
    rpc Talk(DialogueRequest) returns (DialogueResponse);
    rpc QueryKnowledge(KnowledgeQuery) returns (KnowledgeResponse);
    rpc GetState(Empty) returns (AgentState);
}

service LifecycleService {
    rpc Reconcile(RelevanceContext) returns (ReconciliationReport);
    rpc StartAgent(StartRequest) returns (AgentHandle);
    rpc StopAgent(StopRequest) returns (Empty);
}
```

#### 3. Event Subscription (for coordination)

```rust
// Subscribe to agent events via SurrealDB live queries
let subscription = event_stream
    .subscribe(
        "LIVE SELECT * FROM events WHERE agent_id = $id",
        MyEventHandler,
    )
    .await?;
```

### Agent Implementation API

Developers building domain-specific agents implement:

```rust
#[async_trait]
pub trait AgentBehavior: Send + Sync {
    /// Process incoming dialogue
    async fn process_dialogue(
        &self,
        request: DialogueRequest,
    ) -> Result<DialogueResponse>;
    
    /// Query agent's knowledge
    async fn query_knowledge(
        &self,
        query: &str,
    ) -> Result<KnowledgeResponse>;
    
    /// Handle incoming event
    async fn handle_event(
        &self,
        event: Event,
    ) -> Result<EventResponse>;
    
    /// Custom relevance evaluation (optional)
    async fn evaluate_relevance(
        &self,
        context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        // Default implementation
        Ok(RelevanceScore::new(0.5))
    }
}

// Agents are created using builder pattern
let agent = Agent::builder()
    .id("my_agent")
    .behavior(MyAgentBehavior::new())
    .memory_config(MemoryConfig::default())
    .relevance_evaluator(MyRelevanceEvaluator)
    .build()
    .await?;
```

---

## Search Strategy: BM25 vs Vector/Semantic Search

### Hybrid Approach (Recommended)

**Decision**: Support both BM25 and vector search through an abstraction layer, with BM25 as the **pragmatic default**.

### Why Not Just BM25?

While BM25 is excellent for many cases, it has limitations:

| Search Type | Strengths | Weaknesses | Use Cases |
|------------|-----------|------------|-----------|
| **BM25** | Fast, no embeddings needed, keyword matching | Misses semantic similarity, typos hurt | Exact entity lookups, structured queries |
| **Vector/Semantic** | Understands meaning, fuzzy matching, multilingual | Slower, requires embedding model, costs | "What did X say about trust?", concept similarity |

### Real-World Example from Zera

```python
# BM25 works great:
"Find memories mentioning Elder Rowan"
→ Direct keyword match, fast

# BM25 struggles:
"Find memories about trust and betrayal"
→ Needs to match concept, not just words
→ "He seemed dishonest" won't match "betrayal" keyword

# Vector search excels:
query_embedding = embed("trust and betrayal")
→ Finds "he seemed dishonest", "I don't believe him", "lying about..."
```

### Architecture: Pluggable Search Backends

```rust
/// Search abstraction that supports multiple backends
#[async_trait]
pub trait MemorySearchBackend: Send + Sync {
    /// Search memories by query
    async fn search(
        &self,
        query: &str,
        filters: Option<&HashMap<String, Value>>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>>;
    
    /// Search by vector similarity (optional)
    async fn search_by_vector(
        &self,
        vector: &[f32],
        filters: Option<&HashMap<String, Value>>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>> {
        Err(anyhow!("Vector search not supported by this backend"))
    }
    
    /// Hybrid search (combines keyword + semantic)
    async fn hybrid_search(
        &self,
        query: &str,
        filters: Option<&HashMap<String, Value>>,
        limit: usize,
        semantic_weight: f64,  // 0.0 = pure BM25, 1.0 = pure vector
    ) -> Result<Vec<ScoredMemory>> {
        // Default implementation: just use keyword search
        self.search(query, filters, limit).await
    }
}

#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f64,
    pub score_breakdown: Option<ScoreBreakdown>,
}

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub bm25_score: f64,
    pub vector_score: Option<f64>,
    pub recency_boost: f64,
    pub importance_boost: f64,
}
```

### Implementation 1: BM25-Only Backend (Default)

```rust
/// Default backend using Locai's BM25 search
pub struct BM25SearchBackend {
    locai: Arc<EmbeddedLocai>,
    lifecycle: Arc<MemoryLifecycle>,
}

#[async_trait]
impl MemorySearchBackend for BM25SearchBackend {
    async fn search(
        &self,
        query: &str,
        filters: Option<&HashMap<String, Value>>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>> {
        // Use Locai's BM25 search
        let memories = self.locai.search(query, limit, filters).await?;
        
        // Enhance with lifecycle scoring
        let scored = memories.into_iter()
            .map(|mem| {
                let strength = self.lifecycle.calculate_strength(&mem);
                let bm25_score = mem.score.unwrap_or(1.0);
                
                ScoredMemory {
                    memory: mem,
                    score: bm25_score * strength,
                    score_breakdown: Some(ScoreBreakdown {
                        bm25_score,
                        vector_score: None,
                        recency_boost: strength,
                        importance_boost: 1.0,
                    }),
                }
            })
            .collect();
        
        Ok(scored)
    }
}
```

### Implementation 2: Hybrid Backend (Advanced)

```rust
/// Hybrid backend supporting both BM25 and vector search
pub struct HybridSearchBackend {
    locai: Arc<EmbeddedLocai>,
    embedding_model: Arc<dyn EmbeddingModel>,
    lifecycle: Arc<MemoryLifecycle>,
    config: HybridSearchConfig,
}

#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// Default semantic weight (0.0-1.0)
    pub default_semantic_weight: f64,
    
    /// Embedding model to use
    pub embedding_model_name: String,
    
    /// Cache embeddings
    pub cache_embeddings: bool,
}

#[async_trait]
impl MemorySearchBackend for HybridSearchBackend {
    async fn hybrid_search(
        &self,
        query: &str,
        filters: Option<&HashMap<String, Value>>,
        limit: usize,
        semantic_weight: f64,
    ) -> Result<Vec<ScoredMemory>> {
        // Get BM25 results
        let bm25_results = self.locai.search(query, limit * 2, filters).await?;
        
        // Get vector results
        let query_embedding = self.embedding_model.embed(query).await?;
        let vector_results = self.locai
            .search_by_vector(&query_embedding, limit * 2, filters)
            .await?;
        
        // Combine scores using RRF (Reciprocal Rank Fusion)
        let combined = self.reciprocal_rank_fusion(
            bm25_results,
            vector_results,
            semantic_weight,
        )?;
        
        // Apply lifecycle scoring
        let scored = combined.into_iter()
            .map(|(mem, bm25_score, vector_score)| {
                let strength = self.lifecycle.calculate_strength(&mem);
                let final_score = ((1.0 - semantic_weight) * bm25_score 
                                 + semantic_weight * vector_score.unwrap_or(0.0))
                                 * strength;
                
                ScoredMemory {
                    memory: mem,
                    score: final_score,
                    score_breakdown: Some(ScoreBreakdown {
                        bm25_score,
                        vector_score,
                        recency_boost: strength,
                        importance_boost: 1.0,
                    }),
                }
            })
            .collect();
        
        Ok(scored)
    }
}

/// Embedding model abstraction
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
}

/// Example: OpenAI embeddings
pub struct OpenAIEmbeddings {
    client: OpenAIClient,
    model: String,  // "text-embedding-3-small"
}

/// Example: Local embeddings (fast-embed, etc.)
pub struct LocalEmbeddings {
    model: fastembed::TextEmbedding,
}
```

### Configuration

```toml
[search]
# Search backend: "bm25" (default), "hybrid", or "custom"
backend = "bm25"

# Hybrid search settings (when backend = "hybrid")
[search.hybrid]
enabled = false
semantic_weight = 0.3  # 0.0 = pure BM25, 1.0 = pure vector
embedding_model = "text-embedding-3-small"
embedding_provider = "openai"  # or "local", "ollama"
cache_embeddings = true

# Local embedding settings (when provider = "local")
[search.local_embeddings]
model_path = "./models/all-MiniLM-L6-v2"
```

```pkl
// zera-schema.pkl

/// Define when to use semantic vs keyword search
searchStrategies {
  ["concept_lookup"] {
    strategy = "bm25"
    reason = "Exact entity names, fast"
  }
  
  ["thematic_recall"] {
    strategy = "hybrid"
    semantic_weight = 0.7
    reason = "Need semantic similarity for abstract concepts"
  }
  
  ["relationship_context"] {
    strategy = "hybrid"
    semantic_weight = 0.5
    reason = "Balance between specific mentions and semantic relatedness"
  }
}
```

### Recommendation for Framework

**Ship with BM25 as default, make vector search opt-in:**

1. **BM25 by default** ✅
   - Zero dependencies (no embedding models)
   - Fast, good enough for 80% of use cases
   - Lower resource requirements

2. **Vector search opt-in** ✅
   - Enable via feature flag: `--features vector-search`
   - Requires embedding model (user provides)
   - Document when it's worth the complexity

3. **Provide abstraction** ✅
   - `MemorySearchBackend` trait
   - Users can bring their own implementations
   - Framework doesn't force a choice

### Migration Path

```rust
// Start simple (BM25 only)
let memory = MemorySystem::builder()
    .backend(BM25SearchBackend::new(locai))
    .build()?;

// Upgrade to hybrid when needed
let memory = MemorySystem::builder()
    .backend(HybridSearchBackend::new(
        locai,
        OpenAIEmbeddings::new(api_key),
        HybridSearchConfig::default(),
    ))
    .build()?;
```

---

## LLM Interface: Agent Thought and Generation

### Generic LLM Provider Abstraction

**Decision**: Provide a generic LLM trait, but **don't bundle** any implementations.

### Why Generic Interface?

Agents need LLMs for multiple purposes:

| Use Case | Example | Requirements |
|----------|---------|--------------|
| **Dialogue Generation** | NPC responds to player | Streaming, context window |
| **Concept Extraction** | Extract entities from text | Structured output (JSON) |
| **Thought Process** | Internal reasoning | Chain-of-thought, multi-step |
| **Memory Consolidation** | Summarize memories | Long context, summarization |
| **Validation** | Validate aliases | Binary decision + confidence |

### LLM Provider Trait

```rust
/// Generic LLM provider abstraction
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate text from prompt
    async fn generate(
        &self,
        request: LLMRequest,
    ) -> Result<LLMResponse>;
    
    /// Generate with streaming response
    async fn generate_stream(
        &self,
        request: LLMRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;
    
    /// Generate structured output (JSON)
    async fn generate_structured<T: DeserializeOwned>(
        &self,
        request: LLMRequest,
        schema: Option<serde_json::Value>,
    ) -> Result<T> {
        let response = self.generate(request).await?;
        serde_json::from_str(&response.content)
            .map_err(|e| anyhow!("Failed to parse structured output: {}", e))
    }
    
    /// Get provider capabilities
    fn capabilities(&self) -> LLMCapabilities;
    
    /// Get model info
    fn model_info(&self) -> ModelInfo;
}

#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub messages: Vec<Message>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<usize>,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<Tool>,
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub finish_reason: FinishReason,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct LLMCapabilities {
    pub streaming: bool,
    pub function_calling: bool,
    pub json_mode: bool,
    pub vision: bool,
    pub max_context_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub provider: String,
    pub model_name: String,
    pub version: Option<String>,
}
```

### Agent Thought Process Abstraction

Agents need structured reasoning, not just text generation:

```rust
/// Agent reasoning and thought process
pub trait AgentReasoning: Send + Sync {
    /// Generate internal thought process
    async fn think(
        &self,
        situation: &Situation,
        options: &[Action],
    ) -> Result<Thought>;
    
    /// Make decision based on reasoning
    async fn decide(
        &self,
        thought: &Thought,
    ) -> Result<Decision>;
}

#[derive(Debug, Clone)]
pub struct Situation {
    /// Current context
    pub context: HashMap<String, Value>,
    
    /// Relevant memories
    pub memories: Vec<Memory>,
    
    /// Agent's current goals
    pub goals: Vec<Goal>,
    
    /// Recent events
    pub recent_events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct Thought {
    /// Internal reasoning (chain of thought)
    pub reasoning: Vec<ReasoningStep>,
    
    /// Evaluated options
    pub evaluations: Vec<OptionEvaluation>,
    
    /// Confidence in reasoning
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct ReasoningStep {
    pub step: usize,
    pub description: String,
    pub observation: Option<String>,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub chosen_action: Action,
    pub reasoning: String,
    pub confidence: f64,
    pub alternatives: Vec<Action>,
}

/// Default implementation using LLM chain-of-thought
pub struct LLMReasoningEngine {
    llm: Arc<dyn LLMProvider>,
    config: ReasoningConfig,
}

#[derive(Debug, Clone)]
pub struct ReasoningConfig {
    /// Use chain-of-thought prompting
    pub use_cot: bool,
    
    /// Number of reasoning steps
    pub max_steps: usize,
    
    /// Temperature for reasoning
    pub temperature: f64,
    
    /// Reasoning prompt template
    pub prompt_template: String,
}

impl LLMReasoningEngine {
    pub async fn think(
        &self,
        situation: &Situation,
    ) -> Result<Thought> {
        let prompt = self.build_reasoning_prompt(situation)?;
        
        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: self.config.prompt_template.clone(),
                    name: None,
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                    name: None,
                },
            ],
            temperature: Some(self.config.temperature),
            max_tokens: Some(1000),
            response_format: Some(ResponseFormat::Json),
            ..Default::default()
        };
        
        // Get structured reasoning from LLM
        let thought: Thought = self.llm
            .generate_structured(request, None)
            .await?;
        
        Ok(thought)
    }
}
```

### Dialogue Generation Interface

Specialized interface for conversational agents:

```rust
/// Dialogue generation for conversational agents
#[async_trait]
pub trait DialogueGenerator: Send + Sync {
    /// Generate dialogue response
    async fn generate_response(
        &self,
        context: &DialogueContext,
    ) -> Result<DialogueResponse>;
    
    /// Generate response with streaming
    async fn generate_response_stream(
        &self,
        context: &DialogueContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;
}

#[derive(Debug, Clone)]
pub struct DialogueContext {
    /// Agent's personality/character
    pub personality: Personality,
    
    /// Conversation history
    pub conversation: Vec<ConversationTurn>,
    
    /// Relevant memories
    pub memories: Vec<Memory>,
    
    /// Current emotional state
    pub emotional_state: EmotionalState,
    
    /// Current goals/motivations
    pub goals: Vec<Goal>,
}

#[derive(Debug, Clone)]
pub struct Personality {
    /// Core traits
    pub traits: HashMap<String, f64>,  // e.g., "friendly": 0.8
    
    /// Speech patterns
    pub speech_style: SpeechStyle,
    
    /// Background/lore
    pub background: String,
}

#[derive(Debug, Clone)]
pub struct DialogueResponse {
    pub message: String,
    pub emotional_tone: EmotionalTone,
    pub actions: Vec<Action>,
    pub internal_state_changes: Vec<StateChange>,
}

/// Default implementation using LLM
pub struct LLMDialogueGenerator {
    llm: Arc<dyn LLMProvider>,
    prompt_builder: Arc<dyn PromptBuilder>,
}

impl LLMDialogueGenerator {
    async fn generate_response(
        &self,
        context: &DialogueContext,
    ) -> Result<DialogueResponse> {
        // Build rich prompt with personality, memories, etc.
        let prompt = self.prompt_builder.build_dialogue_prompt(context)?;
        
        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: self.build_system_prompt(&context.personality),
                    name: None,
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                    name: None,
                },
            ],
            temperature: Some(0.8),  // More creative for dialogue
            max_tokens: Some(500),
            ..Default::default()
        };
        
        let response = self.llm.generate(request).await?;
        
        // Parse response into structured format
        self.parse_dialogue_response(response)
    }
}

/// Prompt building abstraction
pub trait PromptBuilder: Send + Sync {
    fn build_dialogue_prompt(&self, context: &DialogueContext) -> Result<String>;
    fn build_reasoning_prompt(&self, situation: &Situation) -> Result<String>;
    fn build_extraction_prompt(&self, text: &str, schema: &DomainSchema) -> Result<String>;
}
```

### Configuration

```toml
[llm]
# LLM provider: user brings their own implementation
# Framework provides trait, not implementation
provider = "custom"  # Framework doesn't assume

# Agent reasoning settings
[reasoning]
use_chain_of_thought = true
max_reasoning_steps = 5
reasoning_temperature = 0.7

# Dialogue generation settings
[dialogue]
temperature = 0.8
max_tokens = 500
streaming = true
```

```pkl
// zera-schema.pkl

/// LLM usage per agent type
llmUsage {
  ["dialogue"] {
    provider = "groq"
    model = "llama-3.3-70b-versatile"
    temperature = 0.8
    maxTokens = 500
  }
  
  ["reasoning"] {
    provider = "ollama"  // Use local for cost savings
    model = "gemma2:27b"
    temperature = 0.7
    maxTokens = 1000
  }
  
  ["extraction"] {
    provider = "ollama"  // Cheap local operations
    model = "gemma3:12b"
    temperature = 0.3
    maxTokens = 2000
  }
}

/// Personality definitions
personalities {
  ["elder_rowan"] {
    traits = new Mapping {
      ["wisdom"] = 0.9
      ["patience"] = 0.8
      ["directness"] = 0.6
      ["humor"] = 0.4
    }
    
    speechStyle {
      formality = "moderate"
      vocabulary = "archaic"
      speakingPatterns = new Listing {
        "Often uses metaphors from nature"
        "Speaks in measured, thoughtful sentences"
        "Occasionally uses old sayings"
      }
    }
  }
}
```

### Example Usage: Agent with LLM

```rust
// User provides their own LLM implementation
pub struct GroqLLMProvider {
    client: GroqClient,
    model: String,
}

#[async_trait]
impl LLMProvider for GroqLLMProvider {
    async fn generate(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Implementation...
    }
    // ... other methods
}

// Create agent with custom LLM
let llm = Arc::new(GroqLLMProvider::new(api_key, "llama-3.3-70b"));

let dialogue_generator = LLMDialogueGenerator::new(
    llm.clone(),
    prompt_builder,
);

let reasoning_engine = LLMReasoningEngine::new(
    llm.clone(),
    ReasoningConfig::default(),
);

let agent = Agent::builder()
    .id("elder_rowan")
    .dialogue_generator(dialogue_generator)
    .reasoning_engine(reasoning_engine)
    .build()
    .await?;

// Agent can now think and speak
let thought = agent.think_about_situation(&situation).await?;
let response = agent.generate_dialogue(&context).await?;
```

---

## Recommendations Summary

### Search Strategy ✅

**Ship BM25 by default, make vector search opt-in:**
- ✅ BM25 covers 80% of use cases
- ✅ Zero embedding dependencies
- ✅ Fast and efficient
- ✅ Opt-in vector search via feature flag
- ✅ Abstraction allows custom implementations

**When to recommend vector search:**
- Thematic/conceptual queries ("memories about trust")
- Cross-lingual applications
- Fuzzy matching needed
- User willing to manage embeddings

### LLM Interface ✅

**Provide traits, don't bundle implementations:**
- ✅ `LLMProvider` trait for text generation
- ✅ `AgentReasoning` trait for thought processes
- ✅ `DialogueGenerator` trait for conversations
- ✅ Users bring their own LLM providers (Groq, Ollama, OpenAI, etc.)
- ✅ Framework handles orchestration, not inference

**Benefits:**
- No lock-in to specific LLM provider
- Users can optimize cost/performance
- Can mix providers (cheap local for extraction, expensive cloud for dialogue)
- Framework stays lean and focused

### What Thymos Provides

| Component | Thymos Provides | User Provides |
|-----------|-----------------|---------------|
| **Search** | BM25 backend + trait | Optional: Vector embeddings |
| **LLM** | Trait + orchestration | Implementation (Groq, Ollama, etc.) |
| **Reasoning** | Structure + patterns | LLM provider |
| **Dialogue** | Context building | LLM provider + personality |
| **Memory** | Storage + lifecycle | Domain schema |

This keeps Thymos **domain-agnostic and composable** while providing structure for common patterns.

---

## Implementation Considerations

### State Management

**Shared State (Locai API)**:
- Campaign/world knowledge
- Shared events and observations
- Cross-agent relationships
- Public facts

**Private State (Embedded SurrealDB)**:
- Agent goals and motivations
- Internal dialogue state
- Decision-making history
- Subjective opinions

**State Persistence on Shutdown**:

```rust
pub struct AgentSnapshot {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub state: serde_json::Value,
    pub conversation_context: Vec<Message>,
    pub active_goals: Vec<Goal>,
}

impl Agent {
    pub async fn save_snapshot(&self) -> Result<PathBuf> {
        let snapshot = AgentSnapshot {
            agent_id: self.id.clone(),
            timestamp: Utc::now(),
            state: self.serialize_state()?,
            conversation_context: self.get_recent_context(20).await?,
            active_goals: self.get_active_goals().await?,
        };
        
        let path = format!("/var/lib/memoria/agents/{}.snapshot", self.id);
        tokio::fs::write(&path, serde_json::to_vec(&snapshot)?).await?;
        
        Ok(PathBuf::from(path))
    }
    
    pub async fn restore_snapshot(path: &Path) -> Result<Agent> {
        let data = tokio::fs::read(path).await?;
        let snapshot: AgentSnapshot = serde_json::from_slice(&data)?;
        
        // Rebuild agent from snapshot
        // ...
    }
}
```

### Performance Considerations

**Memory Efficiency**:
- Lazy-load agent state
- LRU cache for frequently accessed memories
- Batch memory operations
- Stream large result sets

**Process Efficiency**:
- Fast startup (< 500ms target)
- Graceful shutdown (< 2s)
- Low idle memory (< 50MB per agent)
- Efficient event filtering

**Scalability**:
- Support 10-50 concurrent agents on single machine
- Horizontal scaling via supervisor coordination
- Shared Locai reduces memory duplication

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ThymosError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    
    #[error("Memory operation failed: {0}")]
    MemoryError(#[from] LocaiError),
    
    #[error("Agent startup timeout")]
    StartupTimeout,
    
    #[error("Invalid relevance context: {0}")]
    InvalidContext(String),
    
    #[error("LLM provider error: {0}")]
    LLMError(String),
}

pub type Result<T> = std::result::Result<T, ThymosError>;
```

### Observability

```rust
/// Metrics exposed via Prometheus
pub struct Metrics {
    pub agent_count: IntGauge,
    pub active_agents: IntGauge,
    pub memory_operations: IntCounter,
    pub consolidation_duration: Histogram,
    pub relevance_evaluation_duration: Histogram,
    pub agent_startup_duration: Histogram,
}

/// Structured logging
#[instrument(skip(self))]
pub async fn start_agent(&self, agent_id: &str) -> Result<AgentHandle> {
    info!(agent_id = %agent_id, "Starting agent");
    // ...
}
```

---

## Configuration Architecture

### Hybrid Configuration Approach

Memoria uses a **three-tier configuration system** to balance simplicity, type safety, and flexibility:

| Tier | Format | Use Case | Examples |
|------|--------|----------|----------|
| **1. Static Config** | TOML | Simple settings, runtime behavior | Ports, timeouts, feature flags |
| **2. Typed Schema** | Pkl | Domain schemas, validation rules | Relationship types, concept types |
| **3. Custom Code** | Rust Traits | Complex logic, algorithms | Relevance evaluators, extractors |

### Why This Approach?

**Problem**: Configuration needs vary widely by complexity:
- ✅ Simple: Port numbers, timeouts → TOML is perfect
- ⚠️ Medium: Relationship types with rules → TOML gets verbose and error-prone
- ❌ Complex: Relevance evaluation logic → Can't express in static config

**Solution**: Use the right tool for each complexity level.

---

## Tier 1: Static Configuration (TOML)

For runtime settings and feature flags:

```toml
# thymos.toml

[memory]
# Locai configuration
locai_mode = "embedded"  # or "api"
locai_url = "http://localhost:3000"
locai_api_key = ""

# Lifecycle settings
forgetting_curve_enabled = true
recency_decay_hours = 168.0  # 1 week
access_count_weight = 0.1
emotional_weight_multiplier = 1.5
base_decay_rate = 0.01

[concepts]
# Concept extraction
extraction_enabled = true
significance_threshold = 0.6
promotion_threshold = 0.7
min_mentions = 3
use_llm_validation = true

# Alias system
aliases_enabled = true
alias_store_threshold = 0.70
alias_auto_resolve_threshold = 0.85

[consolidation]
enabled = true
min_memories = 10
consolidation_window = "1h"
batch_size = 50

[lifecycle]
# Agent lifecycle thresholds
threshold_active = 0.7
threshold_listening = 0.4
threshold_dormant = 0.1

# Supervisor settings
agent_binary = "./target/release/thymos-agent"
port_start = 3000
startup_timeout = "10s"
shutdown_timeout = "5s"

[events]
# Event stream settings
event_db_url = "ws://localhost:8000"
event_buffer_size = 100

[mcp]
# MCP server settings
enabled = true
host = "127.0.0.1"
port = 3000

[observability]
metrics_enabled = true
metrics_port = 9090
log_level = "info"

# Import domain-specific schema
[domain]
schema_path = "./zera-schema.pkl"

# Project metadata
[project]
name = "thymos"
version = "0.1.0"
```

---

## Tier 2: Typed Schema Configuration (Pkl)

For domain-specific types, validation, and structured data:

### Why Pkl?

1. **Type Safety**: Catch errors before runtime
2. **Validation**: Built-in constraints and checks
3. **Composition**: Import and extend schemas
4. **IDE Support**: Autocomplete and type checking
5. **Documentation**: Self-documenting schemas
6. **Generation**: Can output to TOML/JSON/YAML if needed

### Example: Zera Domain Schema

```pkl
// zera-schema.pkl
amends "package://thymos.dev/thymos-schema@1.0.0#/DomainSchema.pkl"

/// Zera RPG domain configuration
module zera.schema

import "thymos:schema/ConceptType.pkl"
import "thymos:schema/RelationType.pkl"
import "thymos:schema/RelevanceCriteria.pkl"

/// Concept types for Zera
conceptTypes {
  ["character"] {
    description = "NPCs and player characters"
    defaultSignificance = 0.9  // Characters are important by default
    promotionThreshold = 0.7
    
    // Required attributes for tracked concepts
    requiredAttributes = new Listing {
      "name"
      "character_type"
    }
    
    // Optional attributes
    optionalAttributes = new Listing {
      "location"
      "status"
      "faction"
      "class"
    }
    
    // Validation rules
    validation {
      ["name"] = (value) -> value.length > 0 && value.length < 100
      ["character_type"] = (value) -> 
        List("pc", "npc", "monster", "companion").contains(value)
    }
  }
  
  ["location"] {
    description = "Places in the game world"
    defaultSignificance = 0.8
    promotionThreshold = 0.6
    
    requiredAttributes = new Listing {
      "name"
      "location_type"
    }
    
    validation {
      ["location_type"] = (value) ->
        List("town", "dungeon", "wilderness", "building").contains(value)
    }
  }
  
  ["item"] {
    description = "Objects and equipment"
    defaultSignificance = 0.5  // Most items are mundane
    promotionThreshold = 0.8    // Only significant items get promoted
    
    requiredAttributes = new Listing {
      "name"
    }
    
    optionalAttributes = new Listing {
      "rarity"
      "magical"
      "owner"
    }
  }
  
  ["faction"] {
    description = "Groups and organizations"
    defaultSignificance = 0.85
    promotionThreshold = 0.65
    
    requiredAttributes = new Listing {
      "name"
    }
  }
}

/// Relationship types for Zera
relationshipTypes {
  ["knows"] {
    description = "One character knows another"
    symmetric = false
    transitive = false
    reflexive = false
    
    // Valid source/target types
    sourceTypes = new Listing { "character" }
    targetTypes = new Listing { "character" }
    
    // Relationship properties schema
    properties {
      "familiarity" {
        type = "number"
        min = 0.0
        max = 1.0
        default = 0.5
      }
      "trust" {
        type = "number"
        min = -1.0
        max = 1.0
        default = 0.0
      }
    }
  }
  
  ["member_of"] {
    description = "Character is member of faction"
    symmetric = false
    transitive = false
    
    sourceTypes = new Listing { "character" }
    targetTypes = new Listing { "faction" }
    
    properties {
      "rank" {
        type = "string"
        enum = new Listing { "leader", "officer", "member", "recruit" }
        default = "member"
      }
      "loyalty" {
        type = "number"
        min = 0.0
        max = 1.0
        default = 0.7
      }
    }
  }
  
  ["located_in"] {
    description = "Entity is at a location"
    symmetric = false
    transitive = false
    
    sourceTypes = new Listing { "character", "item" }
    targetTypes = new Listing { "location" }
  }
  
  ["allied_with"] {
    description = "Factions are allied"
    symmetric = true  // Bidirectional alliance
    transitive = false
    
    sourceTypes = new Listing { "faction", "character" }
    targetTypes = new Listing { "faction", "character" }
    
    properties {
      "strength" {
        type = "number"
        min = 0.0
        max = 1.0
        default = 0.5
      }
    }
  }
  
  ["enemy_of"] {
    description = "Hostile relationship"
    symmetric = true
    transitive = false
    
    sourceTypes = new Listing { "faction", "character" }
    targetTypes = new Listing { "faction", "character" }
  }
}

/// Event filters (what events each concept type should listen to)
eventFilters {
  ["character"] = (agentId, event) ->
    // Character agents listen to events where they're mentioned
    event.tags.contains(agentId) ||
    event.location == this.currentLocation ||
    event.targetNpc == agentId
    
  ["location"] = (agentId, event) ->
    // Location agents listen to events at that location
    event.location == agentId
}

/// Alias patterns for extraction
aliasPatterns {
  ["character"] = new Listing {
    raw#"(?:known as|called|nicknamed)\s+['"]([^'"]+)['"]"#
    raw#"they call (?:him|her|them)\s+['"]([^'"]+)['"]"#
    raw#"I am\s+['"]?([^'",\.]+)['"]?"#
  }
}

/// LLM prompts (can be parameterized)
prompts {
  ["concept_extraction"] = """
    Extract significant concepts from this narrative.
    Focus on: ${conceptTypes.keys.join(", ")}
    
    Text: ${text}
    
    Return JSON with extracted concepts.
    """
  
  ["alias_validation"] = """
    Validate if "${alias}" is a true alias for "${conceptName}" (${conceptType}).
    
    Context: ${context}
    
    Return confidence score (0.0-1.0) and reasoning.
    """
}
```

### Example: Customer Support Domain

```pkl
// support-schema.pkl
amends "package://thymos.dev/thymos-schema@1.0.0#/DomainSchema.pkl"

module support.schema

conceptTypes {
  ["customer"] {
    description = "Customer accounts"
    defaultSignificance = 1.0
    promotionThreshold = 0.9
    
    requiredAttributes = new Listing {
      "customer_id"
      "name"
      "email"
    }
    
    optionalAttributes = new Listing {
      "tier"  // "free", "pro", "enterprise"
      "account_age_days"
    }
  }
  
  ["issue"] {
    description = "Support issues/tickets"
    defaultSignificance = 0.9
    promotionThreshold = 0.8
    
    requiredAttributes = new Listing {
      "issue_id"
      "category"
      "severity"
    }
    
    validation {
      ["category"] = (value) ->
        List("technical", "billing", "feature_request", "bug").contains(value)
      ["severity"] = (value) ->
        List("low", "medium", "high", "critical").contains(value)
    }
  }
  
  ["product"] {
    description = "Products and features"
    defaultSignificance = 0.7
    promotionThreshold = 0.6
  }
}

relationshipTypes {
  ["has_issue"] {
    description = "Customer has open issue"
    symmetric = false
    sourceTypes = new Listing { "customer" }
    targetTypes = new Listing { "issue" }
    
    properties {
      "status" {
        type = "string"
        enum = new Listing { "open", "in_progress", "waiting", "resolved" }
      }
      "assigned_agent" {
        type = "string"
        optional = true
      }
    }
  }
}
```

### Loading Pkl Configuration in Rust

```rust
use pkl_rs::{PklMod, Evaluator};

pub struct DomainSchema {
    pub concept_types: HashMap<String, ConceptTypeConfig>,
    pub relationship_types: HashMap<String, RelationshipTypeConfig>,
    pub event_filters: HashMap<String, String>,
    pub prompts: HashMap<String, String>,
}

impl DomainSchema {
    pub async fn load_from_pkl(path: &Path) -> Result<Self> {
        let evaluator = Evaluator::new()?;
        let module = evaluator.evaluate_module::<DomainSchemaModule>(path)?;
        
        // Convert Pkl types to Rust types
        let schema = Self {
            concept_types: module.concept_types.into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            relationship_types: module.relationship_types.into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            event_filters: module.event_filters,
            prompts: module.prompts,
        };
        
        // Validate schema
        schema.validate()?;
        
        Ok(schema)
    }
    
    pub fn validate(&self) -> Result<()> {
        // Check that relationship source/target types exist
        for (rel_name, rel_type) in &self.relationship_types {
            for source_type in &rel_type.source_types {
                if !self.concept_types.contains_key(source_type) {
                    return Err(anyhow!(
                        "Relationship '{}' references unknown source type '{}'",
                        rel_name, source_type
                    ));
                }
            }
            // Similar for target_types...
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConceptTypeConfig {
    pub description: String,
    pub default_significance: f64,
    pub promotion_threshold: f64,
    pub required_attributes: Vec<String>,
    pub optional_attributes: Vec<String>,
    
    // Validation rules are compiled to Rust closures
    #[serde(skip)]
    pub validators: HashMap<String, Box<dyn Fn(&str) -> bool + Send + Sync>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelationshipTypeConfig {
    pub description: String,
    pub symmetric: bool,
    pub transitive: bool,
    pub reflexive: bool,
    pub source_types: Vec<String>,
    pub target_types: Vec<String>,
    pub properties: HashMap<String, PropertySchema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PropertySchema {
    pub property_type: PropertyType,
    pub default: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertyType {
    String,
    Number,
    Boolean,
}
```

---

## Tier 3: Custom Code (Rust Traits)

For complex logic that can't be expressed declaratively:

### Example: Domain-Specific Relevance Evaluator

```rust
// In your Zera application code

pub struct ZeraRelevanceEvaluator {
    schema: Arc<DomainSchema>,
}

#[async_trait]
impl RelevanceEvaluator for ZeraRelevanceEvaluator {
    async fn evaluate(
        &self,
        agent_id: &str,
        context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        // Extract Zera-specific context
        let in_party: bool = context.get("in_party").unwrap_or(false);
        let zones_away: i32 = context.get("zones_away").unwrap_or(999);
        let last_interaction_turns: Option<i32> = context.get("last_interaction_turns");
        let in_active_quest: bool = context.get("in_active_quest").unwrap_or(false);
        let mentioned_recently: bool = context.get("mentioned_recently").unwrap_or(false);
        
        // Complex relevance calculation
        let score = if in_party {
            1.0  // Always active if in party
        } else if let Some(turns) = last_interaction_turns {
            if turns < 3 {
                1.0  // Recent interaction
            } else {
                0.5 - (turns as f64 * 0.05)  // Decay over time
            }
        } else if zones_away == 0 {
            0.8  // Same location
        } else if in_active_quest && zones_away <= 2 {
            0.7  // Quest-relevant and nearby
        } else if zones_away <= 3 || mentioned_recently {
            0.4  // Nearby or mentioned
        } else if in_active_quest {
            0.2  // Quest-relevant but distant
        } else {
            0.05  // Not relevant
        };
        
        Ok(RelevanceScore::new(score))
    }
}

// Register with framework
let agent = Agent::builder()
    .id("elder_rowan")
    .relevance_evaluator(Arc::new(ZeraRelevanceEvaluator::new(schema)))
    .build()
    .await?;
```

### Example: Custom Concept Extractor

```rust
pub struct ZeraConceptExtractor {
    llm: Arc<dyn LLMProvider>,
    schema: Arc<DomainSchema>,
}

#[async_trait]
impl ConceptExtractor for ZeraConceptExtractor {
    async fn extract(
        &self,
        text: &str,
        context: Option<&Context>,
    ) -> Result<Vec<Concept>> {
        // Use schema-defined prompt template
        let prompt = self.schema.prompts.get("concept_extraction")
            .ok_or_else(|| anyhow!("Missing extraction prompt"))?;
        
        // Substitute variables
        let prompt = prompt
            .replace("${text}", text)
            .replace(
                "${conceptTypes}", 
                &self.schema.concept_types.keys()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        
        // Call LLM
        let response = self.llm.generate(&prompt).await?;
        
        // Parse response and validate against schema
        let concepts: Vec<Concept> = serde_json::from_str(&response)?;
        
        // Validate each concept
        for concept in &concepts {
            self.validate_concept(concept)?;
        }
        
        Ok(concepts)
    }
    
    fn validate_concept(&self, concept: &Concept) -> Result<()> {
        let type_config = self.schema.concept_types.get(&concept.concept_type)
            .ok_or_else(|| anyhow!("Unknown concept type: {}", concept.concept_type))?;
        
        // Check required attributes
        for required in &type_config.required_attributes {
            if !concept.attributes.contains_key(required) {
                return Err(anyhow!(
                    "Concept '{}' missing required attribute '{}'",
                    concept.text, required
                ));
            }
        }
        
        // Run validation rules
        for (attr, value) in &concept.attributes {
            if let Some(validator) = type_config.validators.get(attr) {
                if !validator(value) {
                    return Err(anyhow!(
                        "Validation failed for '{}' attribute '{}' = '{}'",
                        concept.text, attr, value
                    ));
                }
            }
        }
        
        Ok(())
    }
}
```

---

## Configuration Loading Flow

```rust
pub struct ThymosFramework {
    config: RuntimeConfig,
    schema: DomainSchema,
}

impl ThymosFramework {
    pub async fn initialize(config_path: &Path) -> Result<Self> {
        // 1. Load static TOML config
        let config: RuntimeConfig = {
            let contents = tokio::fs::read_to_string(config_path).await?;
            toml::from_str(&contents)?
        };
        
        // 2. Load Pkl schema if specified
        let schema = if let Some(schema_path) = &config.domain.schema_path {
            DomainSchema::load_from_pkl(schema_path).await?
        } else {
            DomainSchema::default()
        };
        
        // 3. Validate combined configuration
        Self::validate_config(&config, &schema)?;
        
        Ok(Self { config, schema })
    }
    
    pub fn create_agent(&self, behavior: impl AgentBehavior) -> AgentBuilder {
        AgentBuilder::new()
            .config(self.config.clone())
            .schema(self.schema.clone())
            .behavior(behavior)
    }
}
```

---

## Configuration Best Practices

### When to Use Each Tier

| Configuration Need | Use | Example |
|-------------------|-----|---------|
| Port numbers, URLs | TOML | `mcp_port = 3000` |
| Feature flags | TOML | `forgetting_curve_enabled = true` |
| Thresholds, weights | TOML | `promotion_threshold = 0.7` |
| Type definitions | Pkl | `conceptTypes { ["character"] { ... } }` |
| Validation rules | Pkl | `validation { ["status"] = ... }` |
| Prompts, templates | Pkl | `prompts { ["extraction"] = """...""" }` |
| Complex algorithms | Rust | `impl RelevanceEvaluator for MyEvaluator` |
| Custom extractors | Rust | `impl ConceptExtractor for MyExtractor` |
| Business logic | Rust | Custom agent behaviors |

### Schema Composition

Pkl allows schema composition and inheritance:

```pkl
// base-rpg-schema.pkl
module base.rpg

abstract class RPGSchema {
  conceptTypes: Mapping<String, ConceptType>
  relationshipTypes: Mapping<String, RelationType>
}

// zera-schema.pkl extends base
amends "base-rpg-schema.pkl"

module zera.schema

// Extend base RPG schema
conceptTypes {
  ...base.conceptTypes  // Include all base types
  
  // Add Zera-specific types
  ["quest"] {
    description = "Quests and missions"
    defaultSignificance = 0.95
  }
}
```

### Environment-Specific Configuration

Pkl supports generating different configs for different environments:

```pkl
// config-dev.pkl
amends "memoria.toml.pkl"

memory {
  locai_mode = "embedded"
  locai_url = "http://localhost:3000"
}

observability {
  log_level = "debug"
}

// config-prod.pkl
amends "memoria.toml.pkl"

memory {
  locai_mode = "api"
  locai_url = "https://locai.production.internal:3000"
}

observability {
  log_level = "info"
  metrics_enabled = true
}
```

---

## Migration Path

For existing applications (like Zera):

### Phase 1: Start with TOML
```toml
# Simple migration
[domain]
concept_types = ["character", "location", "item", "faction"]
relationship_types = ["knows", "member_of", "located_in"]
```

### Phase 2: Add Pkl Schema
```pkl
// Gradually move to typed schema
conceptTypes {
  ["character"] {
    defaultSignificance = 0.9
    promotionThreshold = 0.7
  }
  // ... more detailed config
}
```

### Phase 3: Custom Implementations
```rust
// Keep complex logic in Rust
impl RelevanceEvaluator for ZeraEvaluator {
    // Domain-specific evaluation
}
```

---

## Recommendation for Zera

**Use Pkl for domain schema** ✅

Why Pkl specifically:
1. **Type Safety**: Catch config errors early
2. **Validation**: Ensures relationship types reference valid concept types
3. **Documentation**: Schema is self-documenting
4. **Flexibility**: Can add complex validation without custom code
5. **IDE Support**: Great developer experience
6. **Not Overkill**: Pkl is designed for configuration, not general programming

**What goes where**:
- TOML: Port, URLs, feature flags, simple thresholds
- Pkl: Concept types, relationship types, prompts, validation rules
- Rust: Relevance evaluation, custom extraction logic, game mechanics

This gives you flexibility without overwhelming complexity.

---

## Integration Patterns

### Pattern 1: Single Agent Application

```rust
// Simple chatbot using Thymos
#[tokio::main]
async fn main() -> Result<()> {
    let agent = Agent::builder()
        .id("assistant")
        .behavior(ChatbotBehavior::new())
        .build()
        .await?;
    
    // Start MCP server
    let mcp_server = AgentMcpServer::new(agent);
    mcp_server.listen("127.0.0.1:3000").await?;
    
    Ok(())
}
```

### Pattern 2: Multi-Agent with Lifecycle Management

```rust
// Game with multiple NPCs (Zera use case)
#[tokio::main]
async fn main() -> Result<()> {
    // Create supervisor
    let supervisor = ProcessSupervisor::new(config).await?;
    
    // Create lifecycle manager with domain-specific relevance
    let lifecycle = AgentLifecycleManager::new(
        supervisor,
        GameRelevanceEvaluator::new(),
    );
    
    // Game loop
    loop {
        let game_state = get_game_state().await?;
        
        // Reconcile agent states based on game state
        let report = lifecycle.reconcile(&game_state.into()).await?;
        
        info!("Reconciliation: {:?}", report);
        
        // Process turn...
    }
}
```

### Pattern 3: Event-Driven Coordination

```rust
// Agents react to shared events
impl Agent {
    pub async fn start_event_listener(&self) -> Result<()> {
        let agent_id = self.id.clone();
        
        self.event_stream
            .subscribe(
                &format!("LIVE SELECT * FROM events WHERE $agent_id IN mentions"),
                move |event| {
                    // Agent decides how to react
                    self.process_event(event).await
                },
            )
            .await?;
        
        Ok(())
    }
}
```

---

## Development Roadmap

### Phase 1: Core Foundation (Weeks 1-3)

**Week 1: Memory System**
- [ ] Locai wrapper and embedded mode
- [ ] Memory lifecycle manager
- [ ] Forgetting curve implementation
- [ ] Basic memory operations (CRUD)

**Week 2: Concept Extraction**
- [ ] Concept extractor trait and basic implementation
- [ ] Significance scoring
- [ ] Alias extraction and validation
- [ ] Concept promotion pipeline

**Week 3: Event System**
- [ ] Hook registry and basic hooks
- [ ] SurrealDB event stream wrapper
- [ ] Event subscription system

### Phase 2: Agent Infrastructure (Weeks 4-6)

**Week 4: Agent Core**
- [ ] Agent struct and state management
- [ ] Agent behavior trait
- [ ] Private state (embedded SurrealDB)
- [ ] State persistence (snapshots)

**Week 5: Lifecycle Management**
- [ ] Relevance evaluator trait
- [ ] Agent lifecycle manager
- [ ] Reconciliation logic
- [ ] State transitions

**Week 6: Supervisor**
- [ ] Process supervisor implementation
- [ ] Health monitoring
- [ ] Graceful shutdown
- [ ] Port allocation

### Phase 3: Interfaces (Weeks 7-8)

**Week 7: MCP Server**
- [ ] MCP server implementation
- [ ] Tool definitions
- [ ] Resource definitions
- [ ] Error handling

**Week 8: Additional APIs**
- [ ] gRPC service definitions
- [ ] REST API (optional)
- [ ] CLI for agent management

### Phase 4: Consolidation & Polish (Weeks 9-10)

**Week 9: Consolidation Engine**
- [ ] LLM integration trait
- [ ] Consolidation logic
- [ ] Insight generation
- [ ] Batch operations

**Week 10: Testing & Documentation**
- [ ] Unit tests for all components
- [ ] Integration tests
- [ ] Example applications
- [ ] API documentation
- [ ] User guide

### Phase 5: Zera Integration (Weeks 11-12)

**Week 11: Zera Integration**
- [ ] Game relevance evaluator
- [ ] Replace existing lifecycle code
- [ ] Migrate consolidation system
- [ ] NPC agents as Memoria agents

**Week 12: Testing & Refinement**
- [ ] End-to-end testing in Zera
- [ ] Performance tuning
- [ ] Bug fixes
- [ ] Documentation updates

---

## Success Criteria

### Functional Requirements

- ✅ **Memory Management**: Store, retrieve, and score memories with temporal decay
- ✅ **Concept Extraction**: Extract and track domain-agnostic concepts
- ✅ **Agent Lifecycle**: Start/stop agents based on relevance criteria
- ✅ **Event Coordination**: Agents react to shared events
- ✅ **MCP Interface**: Standard MCP server for each agent
- ✅ **State Persistence**: Graceful shutdown with state restoration

### Performance Requirements

- ⚡ **Agent Startup**: < 500ms from cold start
- ⚡ **Memory Operations**: < 50ms for common operations
- ⚡ **Concurrent Agents**: Support 10-50 agents on single machine
- ⚡ **Memory Footprint**: < 50MB per idle agent

### Quality Requirements

- 📊 **Test Coverage**: > 80% code coverage
- 📚 **Documentation**: All public APIs documented
- 🔍 **Observability**: Metrics and structured logging
- 🛡️ **Error Handling**: Graceful degradation

---

## Non-Goals (Out of Scope)

- ❌ Domain-specific logic (stays in application layer)
- ❌ LLM implementations (use trait, bring your own)
- ❌ Distributed consensus (single-machine first)
- ❌ Web UI for agent management (CLI only)
- ❌ Built-in authentication/authorization
- ❌ Multi-tenancy (single application deployment)

---

## Future Enhancements (Post-MVP)

### Phase 6+: Advanced Features

- **Distributed Agents**: Multi-machine deployment with coordination
- **Advanced Relevance**: Machine learning-based relevance prediction
- **Memory Compression**: Automatic archival and compression of old memories
- **Multi-Modal Memory**: Support for image/audio memories
- **Relationship Graphs**: First-class relationship tracking
- **Agent Communication Protocol**: Direct agent-to-agent messaging
- **Python Bindings**: PyO3 bindings for Python applications
- **WASM Support**: Run agents in browser
- **Agent Templates**: Pre-built agent types (assistant, character, etc.)

---

## Appendix A: Key Dependencies

### Core Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Memory
locai-sdk = "0.2"  # When available, or embed directly

# Database
surrealdb = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# HTTP/gRPC
axum = "0.7"
tonic = "0.11"

# MCP
mcp-sdk = "0.1"  # When available

# Metrics
prometheus = "0.13"

# Time
chrono = "0.4"
```

---

## Appendix B: Comparison to Existing Solutions

| Feature | Thymos | LangChain | AutoGPT | CrewAI |
|---------|--------|-----------|---------|--------|
| Memory System | ✅ Built-in (Locai) | Basic | Basic | Basic |
| Temporal Decay | ✅ Forgetting curve | ❌ | ❌ | ❌ |
| Agent Lifecycle | ✅ Relevance-based | ❌ | ❌ | ❌ |
| Multi-Agent | ✅ Event-driven | Limited | Limited | ✅ |
| MCP Native | ✅ First-class | ❌ | ❌ | ❌ |
| Language | Rust | Python | Python | Python |
| Embedding | ✅ Locai embedded | External DB | External DB | External DB |
| Domain-Agnostic | ✅ | ✅ | ❌ (automation focus) | ❌ (crew focus) |

---

## Appendix C: Example Domain Implementations

### Example 1: Customer Support Agent

```rust
struct SupportAgent {
    ticket_history: Vec<Ticket>,
    knowledge_base: KnowledgeBase,
}

#[async_trait]
impl RelevanceEvaluator for SupportRelevanceEvaluator {
    async fn evaluate(
        &self,
        agent_id: &str,
        context: &RelevanceContext,
    ) -> Result<RelevanceScore> {
        let open_tickets: usize = context.get("open_tickets").unwrap_or(0);
        let assigned_to_agent = context.get::<bool>("assigned").unwrap_or(false);
        
        let score = if assigned_to_agent {
            1.0  // Active if has assigned tickets
        } else if open_tickets > 10 {
            0.5  // Listening if queue is busy
        } else {
            0.1  // Dormant otherwise
        };
        
        Ok(RelevanceScore::new(score))
    }
}
```

### Example 2: Research Assistant

```rust
struct ResearchAgent {
    papers_read: Vec<Paper>,
    current_project: Option<Project>,
}

#[async_trait]
impl AgentBehavior for ResearchAgent {
    async fn process_dialogue(
        &self,
        request: DialogueRequest,
    ) -> Result<DialogueResponse> {
        // Query memory for relevant papers
        let relevant_papers = self.memory
            .search(&request.message, 5)
            .await?;
        
        // Generate response using LLM + context
        // ...
    }
}
```

---

## References

- [Ebbinghaus Forgetting Curve](https://en.wikipedia.org/wiki/Forgetting_curve)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [Locai Documentation](https://github.com/StructuredLabs/locai)
- [SurrealDB Live Queries](https://surrealdb.com/docs/surrealql/statements/live)

---

## Project Structure

```
thymos/
├── thymos-core/          # Core framework
├── thymos-supervisor/    # Agent process supervisor
├── thymos-mcp/          # MCP server implementation
├── thymos-cli/          # CLI tools
└── examples/
    ├── zera-agent/      # Zera NPC agent example
    └── chatbot/         # Simple chatbot example
```

---

**Document Status**: Draft  
**Last Updated**: November 6, 2025  
**Next Review**: After Phase 1 implementation  
**Companion Project**: [Locai](https://github.com/StructuredLabs/locai) - Semantic memory system

---

