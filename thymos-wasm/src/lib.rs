//! Thymos WASM Component - WebAssembly bindings using WASI Component Model
//!
//! This crate provides Thymos agent functionality as a WASM Component.
//! It supports two modes:
//!
//! - **Server mode**: Connects to a Locai server via wasi:http for full
//!   semantic search, embeddings, and persistence
//! - **In-memory mode**: Uses a local in-memory store for offline use
//!
//! The implementation mirrors the `MemoryBackend` trait from `thymos-core`.

#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    world: "thymos",
    path: "wit",
});

use thymos::agent::types::{
    AgentId, AgentState, AgentStatus, Memory, MemoryId, MemoryType, RememberOptions, SearchOptions,
    ThymosError,
};

use wasi::http::outgoing_handler;
use wasi::http::types::{
    Fields, Method, OutgoingBody, OutgoingRequest, RequestOptions, Scheme,
};

// ============================================================================
// Backend trait (mirrors thymos-core::memory::backend::MemoryBackend)
// ============================================================================

/// Memory record structure (matches thymos-core)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryRecord {
    id: String,
    content: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_accessed: Option<String>,
    #[serde(default)]
    properties: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
}

/// Store options
#[derive(Debug, Clone, Default, Serialize)]
struct StoreOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
}

// ============================================================================
// Server backend (uses wasi:http to connect to Locai)
// ============================================================================

struct ServerBackend {
    base_url: String,
    api_key: Option<String>,
}

impl ServerBackend {
    fn new(base_url: String, api_key: Option<String>) -> Self {
        Self { base_url, api_key }
    }

    fn make_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, ThymosError> {
        // Parse the URL
        let url = format!("{}{}", self.base_url, path);
        let (scheme, authority, path_with_query) = parse_url(&url)
            .map_err(|e| ThymosError::Configuration(format!("Invalid URL: {}", e)))?;

        // Create headers
        let headers = Fields::new();
        headers
            .set(&"Content-Type".to_string(), &[b"application/json".to_vec()])
            .map_err(|_| ThymosError::Memory("Failed to set Content-Type header".to_string()))?;
        headers
            .set(&"Accept".to_string(), &[b"application/json".to_vec()])
            .map_err(|_| ThymosError::Memory("Failed to set Accept header".to_string()))?;

        if let Some(api_key) = &self.api_key {
            headers
                .set(
                    &"Authorization".to_string(),
                    &[format!("Bearer {}", api_key).into_bytes()],
                )
                .map_err(|_| ThymosError::Memory("Failed to set Authorization header".to_string()))?;
        }

        // Create outgoing request
        let request = OutgoingRequest::new(headers);
        request
            .set_method(&method)
            .map_err(|_| ThymosError::Memory("Failed to set method".to_string()))?;
        request
            .set_scheme(Some(&scheme))
            .map_err(|_| ThymosError::Memory("Failed to set scheme".to_string()))?;
        request
            .set_authority(Some(&authority))
            .map_err(|_| ThymosError::Memory("Failed to set authority".to_string()))?;
        request
            .set_path_with_query(Some(&path_with_query))
            .map_err(|_| ThymosError::Memory("Failed to set path".to_string()))?;

        // Write body if provided
        if let Some(body_bytes) = body {
            let outgoing_body = request
                .body()
                .map_err(|_| ThymosError::Memory("Failed to get request body".to_string()))?;
            let stream = outgoing_body
                .write()
                .map_err(|_| ThymosError::Memory("Failed to get body stream".to_string()))?;
            stream
                .blocking_write_and_flush(body_bytes)
                .map_err(|e| ThymosError::Memory(format!("Failed to write body: {:?}", e)))?;
            drop(stream);
            OutgoingBody::finish(outgoing_body, None)
                .map_err(|_| ThymosError::Memory("Failed to finish body".to_string()))?;
        }

        // Send request
        let options = RequestOptions::new();
        let future_response = outgoing_handler::handle(request, Some(options))
            .map_err(|e| ThymosError::Memory(format!("Failed to send request: {:?}", e)))?;

        // Wait for response
        let response = loop {
            if let Some(result) = future_response.get() {
                break result
                    .map_err(|_| ThymosError::Memory("Response error".to_string()))?
                    .map_err(|e| ThymosError::Memory(format!("HTTP error: {:?}", e)))?;
            }
            future_response.subscribe().block();
        };

        // Check status
        let status = response.status();
        if status >= 400 {
            return Err(ThymosError::Memory(format!("HTTP error: status {}", status)));
        }

        // Read response body
        let incoming_body = response
            .consume()
            .map_err(|_| ThymosError::Memory("Failed to consume response".to_string()))?;
        let stream = incoming_body
            .stream()
            .map_err(|_| ThymosError::Memory("Failed to get response stream".to_string()))?;

        let mut body_bytes = Vec::new();
        loop {
            match stream.blocking_read(65536) {
                Ok(chunk) => {
                    if chunk.is_empty() {
                        break;
                    }
                    body_bytes.extend(chunk);
                }
                Err(_) => break,
            }
        }

        Ok(body_bytes)
    }

    fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String, ThymosError> {
        let mut body = serde_json::json!({ "content": content });
        if let Some(opts) = options {
            if let Some(memory_type) = opts.memory_type {
                body["memory_type"] = serde_json::json!(memory_type);
            }
            if !opts.tags.is_empty() {
                body["tags"] = serde_json::json!(opts.tags);
            }
            if let Some(priority) = opts.priority {
                body["priority"] = serde_json::json!(priority);
            }
        }

        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| ThymosError::Memory(format!("Failed to serialize: {}", e)))?;

        let response = self.make_request(Method::Post, "/api/memories", Some(&body_bytes))?;

        let json: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ThymosError::Memory(format!("Failed to parse response: {}", e)))?;

        json.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ThymosError::Memory("No ID in response".to_string()))
    }

    fn search(
        &self,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<MemoryRecord>, ThymosError> {
        let limit_str = limit.unwrap_or(10).to_string();
        let path = format!(
            "/api/memories/search?q={}&limit={}",
            urlencoding::encode(query),
            limit_str
        );

        let response = self.make_request(Method::Get, &path, None)?;

        let results: Vec<serde_json::Value> = serde_json::from_slice(&response)
            .map_err(|e| ThymosError::Memory(format!("Failed to parse response: {}", e)))?;

        // Locai returns [{"memory": {...}, "score": ...}, ...]
        let records: Vec<MemoryRecord> = results
            .into_iter()
            .filter_map(|r| {
                let memory = r.get("memory")?;
                let score = r.get("score").and_then(|v| v.as_f64());
                Some(MemoryRecord {
                    id: memory.get("id")?.as_str()?.to_string(),
                    content: memory.get("content")?.as_str()?.to_string(),
                    created_at: memory
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z")
                        .to_string(),
                    last_accessed: memory
                        .get("last_accessed")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    properties: memory.get("properties").cloned().unwrap_or(serde_json::json!({})),
                    score,
                })
            })
            .collect();

        Ok(records)
    }

    fn get(&self, id: &str) -> Result<Option<MemoryRecord>, ThymosError> {
        let path = format!("/api/memories/{}", urlencoding::encode(id));
        match self.make_request(Method::Get, &path, None) {
            Ok(response) => {
                let json: serde_json::Value = serde_json::from_slice(&response)
                    .map_err(|e| ThymosError::Memory(format!("Failed to parse: {}", e)))?;
                Ok(Some(MemoryRecord {
                    id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    content: json.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    created_at: json
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z")
                        .to_string(),
                    last_accessed: json.get("last_accessed").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    properties: json.get("properties").cloned().unwrap_or(serde_json::json!({})),
                    score: None,
                }))
            }
            Err(e) => {
                // 404 returns None, other errors propagate
                if format!("{:?}", e).contains("404") {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn delete(&self, id: &str) -> Result<bool, ThymosError> {
        let path = format!("/api/memories/{}", urlencoding::encode(id));
        match self.make_request(Method::Delete, &path, None) {
            Ok(_) => Ok(true),
            Err(e) => {
                if format!("{:?}", e).contains("404") {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn count(&self) -> Result<u64, ThymosError> {
        let response = self.make_request(Method::Get, "/api/memories/count", None)?;
        let json: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ThymosError::Memory(format!("Failed to parse: {}", e)))?;
        json.get("count")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ThymosError::Memory("No count in response".to_string()))
    }

    fn health_check(&self) -> Result<(), ThymosError> {
        self.make_request(Method::Get, "/api/health", None)?;
        Ok(())
    }
}

fn parse_url(url: &str) -> Result<(Scheme, String, String), &'static str> {
    let (scheme, rest) = if url.starts_with("https://") {
        (Scheme::Https, &url[8..])
    } else if url.starts_with("http://") {
        (Scheme::Http, &url[7..])
    } else {
        return Err("Invalid scheme");
    };

    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    Ok((scheme, authority.to_string(), path.to_string()))
}

// ============================================================================
// In-memory backend (for offline use)
// ============================================================================

struct InMemoryBackend {
    memories: RefCell<HashMap<String, MemoryRecord>>,
    next_id: RefCell<u64>,
}

impl InMemoryBackend {
    fn new() -> Self {
        Self {
            memories: RefCell::new(HashMap::new()),
            next_id: RefCell::new(1),
        }
    }

    fn generate_id(&self) -> String {
        let mut next_id = self.next_id.borrow_mut();
        let id = format!("mem_{}", *next_id);
        *next_id += 1;
        id
    }

    fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String, ThymosError> {
        let id = self.generate_id();
        let timestamp = current_timestamp();

        let mut properties = serde_json::Map::new();
        if let Some(opts) = options {
            if let Some(memory_type) = opts.memory_type {
                properties.insert("type".to_string(), serde_json::json!(memory_type));
            }
            if !opts.tags.is_empty() {
                properties.insert("tags".to_string(), serde_json::json!(opts.tags));
            }
            if let Some(priority) = opts.priority {
                properties.insert("priority".to_string(), serde_json::json!(priority));
            }
        }

        let record = MemoryRecord {
            id: id.clone(),
            content,
            created_at: timestamp,
            last_accessed: None,
            properties: serde_json::Value::Object(properties),
            score: None,
        };

        self.memories.borrow_mut().insert(id.clone(), record);
        Ok(id)
    }

    fn search(&self, query: &str, limit: Option<u32>) -> Result<Vec<MemoryRecord>, ThymosError> {
        let limit = limit.unwrap_or(10) as usize;
        let memories = self.memories.borrow();

        let mut scored: Vec<_> = memories
            .values()
            .map(|m| {
                let score = score_match(&m.content, query);
                let mut record = m.clone();
                record.score = Some(score);
                (score, record)
            })
            .filter(|(score, _)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored.into_iter().take(limit).map(|(_, m)| m).collect())
    }

    fn get(&self, id: &str) -> Result<Option<MemoryRecord>, ThymosError> {
        Ok(self.memories.borrow().get(id).cloned())
    }

    fn delete(&self, id: &str) -> Result<bool, ThymosError> {
        Ok(self.memories.borrow_mut().remove(id).is_some())
    }

    fn count(&self) -> Result<u64, ThymosError> {
        Ok(self.memories.borrow().len() as u64)
    }

    fn clear(&self) {
        self.memories.borrow_mut().clear();
        *self.next_id.borrow_mut() = 1;
    }
}

fn current_timestamp() -> String {
    // Use WASI clocks if available, otherwise static
    "2024-01-01T00:00:00Z".to_string()
}

fn score_match(content: &str, query: &str) -> f64 {
    let content_lower = content.to_lowercase();
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

    if query_terms.is_empty() {
        return 0.0;
    }

    let mut matches = 0;
    for term in &query_terms {
        if content_lower.contains(term) {
            matches += 1;
        }
    }

    matches as f64 / query_terms.len() as f64
}

// ============================================================================
// Backend enum (unifies server and in-memory backends)
// ============================================================================

enum Backend {
    Server(ServerBackend),
    InMemory(InMemoryBackend),
}

impl Backend {
    fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String, ThymosError> {
        match self {
            Backend::Server(s) => s.store(content, options),
            Backend::InMemory(m) => m.store(content, options),
        }
    }

    fn search(&self, query: &str, limit: Option<u32>) -> Result<Vec<MemoryRecord>, ThymosError> {
        match self {
            Backend::Server(s) => s.search(query, limit),
            Backend::InMemory(m) => m.search(query, limit),
        }
    }

    fn get(&self, id: &str) -> Result<Option<MemoryRecord>, ThymosError> {
        match self {
            Backend::Server(s) => s.get(id),
            Backend::InMemory(m) => m.get(id),
        }
    }

    fn delete(&self, id: &str) -> Result<bool, ThymosError> {
        match self {
            Backend::Server(s) => s.delete(id),
            Backend::InMemory(m) => m.delete(id),
        }
    }

    fn count(&self) -> Result<u64, ThymosError> {
        match self {
            Backend::Server(s) => s.count(),
            Backend::InMemory(m) => m.count(),
        }
    }
}

// ============================================================================
// Global state
// ============================================================================

struct AgentData {
    id: String,
    description: String,
    status: AgentStatus,
    started_at: Option<String>,
    last_active: String,
    properties_json: String,
}

thread_local! {
    static BACKEND: RefCell<Backend> = RefCell::new(Backend::InMemory(InMemoryBackend::new()));
    static AGENT_DATA: RefCell<Option<AgentData>> = const { RefCell::new(None) };
}

// ============================================================================
// WIT Exports
// ============================================================================

struct ThymosComponent;

export!(ThymosComponent);

// ============================================================================
// Agent Interface
// ============================================================================

impl exports::thymos::agent::agent::Guest for ThymosComponent {
    fn create(id: AgentId) -> Result<(), ThymosError> {
        AGENT_DATA.with(|data| {
            let mut data = data.borrow_mut();
            if data.is_some() {
                return Err(ThymosError::Agent("Agent already created".to_string()));
            }

            *data = Some(AgentData {
                id: id.clone(),
                description: format!("Agent {}", id),
                status: AgentStatus::Active,
                started_at: Some(current_timestamp()),
                last_active: current_timestamp(),
                properties_json: "{}".to_string(),
            });

            Ok(())
        })
    }

    fn id() -> Result<AgentId, ThymosError> {
        AGENT_DATA.with(|data| {
            data.borrow()
                .as_ref()
                .map(|a| a.id.clone())
                .ok_or_else(|| ThymosError::Agent("Agent not created".to_string()))
        })
    }

    fn description() -> Result<String, ThymosError> {
        AGENT_DATA.with(|data| {
            data.borrow()
                .as_ref()
                .map(|a| a.description.clone())
                .ok_or_else(|| ThymosError::Agent("Agent not created".to_string()))
        })
    }

    fn status() -> Result<AgentStatus, ThymosError> {
        AGENT_DATA.with(|data| {
            data.borrow()
                .as_ref()
                .map(|a| a.status)
                .ok_or_else(|| ThymosError::Agent("Agent not created".to_string()))
        })
    }

    fn set_status(status: AgentStatus) -> Result<(), ThymosError> {
        AGENT_DATA.with(|data| {
            let mut data = data.borrow_mut();
            match data.as_mut() {
                Some(agent) => {
                    agent.status = status;
                    agent.last_active = current_timestamp();
                    Ok(())
                }
                None => Err(ThymosError::Agent("Agent not created".to_string())),
            }
        })
    }

    fn state() -> Result<AgentState, ThymosError> {
        AGENT_DATA.with(|data| {
            data.borrow()
                .as_ref()
                .map(|a| AgentState {
                    status: a.status,
                    started_at: a.started_at.clone(),
                    last_active: a.last_active.clone(),
                    properties_json: a.properties_json.clone(),
                })
                .ok_or_else(|| ThymosError::Agent("Agent not created".to_string()))
        })
    }
}

// ============================================================================
// Memory Interface
// ============================================================================

impl exports::thymos::agent::memory::Guest for ThymosComponent {
    fn remember(content: String) -> Result<MemoryId, ThymosError> {
        BACKEND.with(|backend| backend.borrow().store(content, None))
    }

    fn remember_typed(content: String, memory_type: MemoryType) -> Result<MemoryId, ThymosError> {
        let type_str = match memory_type {
            MemoryType::Episodic => "episodic",
            MemoryType::Fact => "fact",
            MemoryType::Conversation => "conversation",
        };
        let options = StoreOptions {
            memory_type: Some(type_str.to_string()),
            ..Default::default()
        };
        BACKEND.with(|backend| backend.borrow().store(content, Some(options)))
    }

    fn remember_with_options(
        content: String,
        options: RememberOptions,
    ) -> Result<MemoryId, ThymosError> {
        let store_options = StoreOptions {
            memory_type: options.memory_type.map(|mt| {
                match mt {
                    MemoryType::Episodic => "episodic",
                    MemoryType::Fact => "fact",
                    MemoryType::Conversation => "conversation",
                }
                .to_string()
            }),
            tags: options
                .tags_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default(),
            priority: options.priority,
        };
        BACKEND.with(|backend| backend.borrow().store(content, Some(store_options)))
    }

    fn search(query: String, options: Option<SearchOptions>) -> Result<Vec<Memory>, ThymosError> {
        let limit = options.and_then(|o| o.limit);
        BACKEND.with(|backend| {
            let records = backend.borrow().search(&query, limit)?;
            Ok(records
                .into_iter()
                .map(|r| Memory {
                    id: r.id,
                    content: r.content,
                    properties_json: r.properties.to_string(),
                    created_at: r.created_at,
                    last_accessed: r.last_accessed,
                })
                .collect())
        })
    }

    fn get(id: MemoryId) -> Result<Option<Memory>, ThymosError> {
        BACKEND.with(|backend| {
            let record = backend.borrow().get(&id)?;
            Ok(record.map(|r| Memory {
                id: r.id,
                content: r.content,
                properties_json: r.properties.to_string(),
                created_at: r.created_at,
                last_accessed: r.last_accessed,
            }))
        })
    }

    fn delete(id: MemoryId) -> Result<bool, ThymosError> {
        BACKEND.with(|backend| backend.borrow().delete(&id))
    }

    fn count() -> Result<u64, ThymosError> {
        BACKEND.with(|backend| backend.borrow().count())
    }
}

// ============================================================================
// Storage Interface (for persistence and mode switching)
// ============================================================================

impl exports::thymos::agent::storage::Guest for ThymosComponent {
    fn connect(server_url: String, api_key: Option<String>) -> Result<(), ThymosError> {
        let server = ServerBackend::new(server_url, api_key);
        
        // Verify connection with health check
        server.health_check()?;
        
        BACKEND.with(|backend| {
            *backend.borrow_mut() = Backend::Server(server);
        });
        
        Ok(())
    }

    fn disconnect() -> Result<(), ThymosError> {
        BACKEND.with(|backend| {
            *backend.borrow_mut() = Backend::InMemory(InMemoryBackend::new());
        });
        Ok(())
    }

    fn is_connected() -> bool {
        BACKEND.with(|backend| matches!(&*backend.borrow(), Backend::Server(_)))
    }

    fn save(path: String) -> Result<u64, ThymosError> {
        // For server mode, this is a no-op (server handles persistence)
        // For in-memory mode, save to WASI filesystem
        BACKEND.with(|backend| {
            match &*backend.borrow() {
                Backend::Server(_) => {
                    // Server handles persistence automatically
                    Ok(0)
                }
                Backend::InMemory(m) => {
                    let memories: Vec<_> = m.memories.borrow().values().cloned().collect();
                    let json = serde_json::to_string_pretty(&memories)
                        .map_err(|e| ThymosError::Memory(format!("Failed to serialize: {}", e)))?;
                    
                    write_file(&path, json.as_bytes())?;
                    Ok(memories.len() as u64)
                }
            }
        })
    }

    fn load(path: String) -> Result<u64, ThymosError> {
        BACKEND.with(|backend| {
            let data = read_file(&path)?;
            if data.is_empty() {
                return Ok(0);
            }

            let records: Vec<MemoryRecord> = serde_json::from_slice(&data)
                .map_err(|e| ThymosError::Memory(format!("Failed to deserialize: {}", e)))?;

            let count = records.len() as u64;

            // Replace in-memory backend with loaded data
            let new_backend = InMemoryBackend::new();
            for record in records {
                new_backend.memories.borrow_mut().insert(record.id.clone(), record);
            }

            *backend.borrow_mut() = Backend::InMemory(new_backend);
            Ok(count)
        })
    }

    fn exists(path: String) -> Result<bool, ThymosError> {
        file_exists(&path)
    }

    fn clear() -> Result<(), ThymosError> {
        BACKEND.with(|backend| {
            match &*backend.borrow() {
                Backend::InMemory(m) => {
                    m.clear();
                    Ok(())
                }
                Backend::Server(_) => {
                    // Can't clear server from client
                    Err(ThymosError::NotSupported(
                        "Cannot clear server storage from client".to_string(),
                    ))
                }
            }
        })
    }
}

// ============================================================================
// WASI Filesystem Helpers
// ============================================================================

use wasi::filesystem::preopens::get_directories;
use wasi::filesystem::types::{DescriptorFlags, OpenFlags, PathFlags};

fn get_root_dir() -> Result<wasi::filesystem::types::Descriptor, ThymosError> {
    let dirs = get_directories();
    dirs.into_iter()
        .next()
        .map(|(desc, _)| desc)
        .ok_or_else(|| ThymosError::Configuration("No preopened directory available".to_string()))
}

fn write_file(path: &str, data: &[u8]) -> Result<(), ThymosError> {
    let dir = get_root_dir()?;
    let file = dir
        .open_at(
            PathFlags::empty(),
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE,
            DescriptorFlags::WRITE,
        )
        .map_err(|e| ThymosError::Memory(format!("Failed to open file: {:?}", e)))?;

    let stream = file
        .write_via_stream(0)
        .map_err(|e| ThymosError::Memory(format!("Failed to get stream: {:?}", e)))?;

    stream
        .blocking_write_and_flush(data)
        .map_err(|e| ThymosError::Memory(format!("Failed to write: {:?}", e)))?;

    Ok(())
}

fn read_file(path: &str) -> Result<Vec<u8>, ThymosError> {
    let dir = get_root_dir()?;
    let file = dir
        .open_at(PathFlags::empty(), path, OpenFlags::empty(), DescriptorFlags::READ)
        .map_err(|e| ThymosError::Memory(format!("Failed to open file: {:?}", e)))?;

    let stat = file
        .stat()
        .map_err(|e| ThymosError::Memory(format!("Failed to stat: {:?}", e)))?;

    let size = stat.size as usize;
    if size == 0 {
        return Ok(Vec::new());
    }

    let stream = file
        .read_via_stream(0)
        .map_err(|e| ThymosError::Memory(format!("Failed to get stream: {:?}", e)))?;

    stream
        .blocking_read(size as u64)
        .map_err(|e| ThymosError::Memory(format!("Failed to read: {:?}", e)))
}

fn file_exists(path: &str) -> Result<bool, ThymosError> {
    let dir = get_root_dir()?;
    match dir.open_at(PathFlags::empty(), path, OpenFlags::empty(), DescriptorFlags::READ) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

