//! Go bindings for Thymos agent framework
//!
//! This module provides C-compatible FFI bindings for Go, allowing Go developers
//! to use Thymos agents via CGO. All functions are designed to be safe when called
//! from Go and follow C FFI conventions.
//!
//! ## Memory Management
//!
//! All pointers returned by Thymos functions must be freed using the corresponding
//! `thymos_free_*` function. Failure to do so will result in memory leaks.
//!
//! ## Error Handling
//!
//! Functions that can fail return null pointers on error. Use `thymos_get_last_error()`
//! to retrieve the error message.
//!
//! ## Thread Safety
//!
//! All functions are thread-safe and can be called from multiple goroutines.

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use once_cell::sync::Lazy;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc;
use std::sync::Mutex;
use thymos_core::agent::{Agent, AgentState, AgentStatus};
use thymos_core::config::{MemoryConfig, MemoryMode, ThymosConfig};
use thymos_core::error::{Result, ThymosError};

// ============================================================================
// Error Handling
// ============================================================================

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

fn set_error(message: impl Into<String>) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(
            CString::new(message.into())
                .unwrap_or_else(|_| CString::new("Failed to create error message").unwrap()),
        );
    });
}


/// Get the last error message.
///
/// Returns a pointer to the error message string, or null if no error occurred.
/// The returned pointer is valid until the next FFI call that may set an error.
///
/// # Safety
/// The returned pointer must not be freed by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

/// Clear the last error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_clear_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}

// ============================================================================
// Runtime Management
// ============================================================================

/// Shared Tokio runtime for all FFI calls.
/// This ensures background tasks (like SurrealDB's) persist across calls.
static RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
    Mutex::new(
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for FFI"),
    )
});

fn block_on<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let rt_guard = RUNTIME.lock().unwrap();
    let handle = rt_guard.handle().clone();
    drop(rt_guard);

    let (tx, rx) = mpsc::channel();
    handle.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });

    rx.recv().map_err(|_| {
        ThymosError::Configuration("Failed to receive result from async task".to_string())
    })?
}

fn block_on_value<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let rt_guard = RUNTIME.lock().unwrap();
    let handle = rt_guard.handle().clone();
    drop(rt_guard);

    let (tx, rx) = mpsc::channel();
    handle.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });

    rx.recv().expect("Failed to receive result from async task")
}

// ============================================================================
// String Utilities
// ============================================================================

fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string()) }
}

fn string_to_cstring(s: String) -> *mut c_char {
    CString::new(s)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}


/// Free a string allocated by Thymos.
///
/// # Safety
/// The pointer must be valid and allocated by a Thymos function, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

// ============================================================================
// Opaque Handles
// ============================================================================

/// Opaque handle for Agent
#[repr(C)]
pub struct ThymosAgent {
    inner: Agent,
}

/// Opaque handle for MemoryConfig
#[repr(C)]
pub struct ThymosMemoryConfig {
    inner: MemoryConfig,
}

/// Opaque handle for ThymosConfig
#[repr(C)]
pub struct ThymosConfigHandle {
    inner: ThymosConfig,
}

// ============================================================================
// Data Structures
// ============================================================================

/// Memory result structure for returning memory data to Go.
#[repr(C)]
pub struct ThymosMemory {
    pub id: *mut c_char,
    pub content: *mut c_char,
    pub properties_json: *mut c_char,
    pub created_at: *mut c_char,
    pub last_accessed: *mut c_char,
}

impl ThymosMemory {
    fn from_locai(memory: &locai::models::Memory) -> Self {
        Self {
            id: string_to_cstring(memory.id.clone()),
            content: string_to_cstring(memory.content.clone()),
            properties_json: serde_json::to_string(&memory.properties)
                .ok()
                .map(string_to_cstring)
                .unwrap_or(ptr::null_mut()),
            created_at: string_to_cstring(memory.created_at.to_rfc3339()),
            last_accessed: memory
                .last_accessed
                .map(|dt| string_to_cstring(dt.to_rfc3339()))
                .unwrap_or(ptr::null_mut()),
        }
    }

    unsafe fn free_fields(&mut self) {
        thymos_free_string(self.id);
        thymos_free_string(self.content);
        thymos_free_string(self.properties_json);
        thymos_free_string(self.created_at);
        thymos_free_string(self.last_accessed);
        self.id = ptr::null_mut();
        self.content = ptr::null_mut();
        self.properties_json = ptr::null_mut();
        self.created_at = ptr::null_mut();
        self.last_accessed = ptr::null_mut();
    }
}

/// Search results structure containing an array of memories.
#[repr(C)]
pub struct ThymosSearchResults {
    pub memories: *mut ThymosMemory,
    pub count: usize,
    pub capacity: usize,
}

/// Agent state structure.
#[repr(C)]
pub struct ThymosAgentState {
    pub status: *mut c_char,
    pub started_at: *mut c_char,
    pub last_active: *mut c_char,
    pub properties_json: *mut c_char,
}

impl ThymosAgentState {
    fn from_state(state: &AgentState) -> Self {
        Self {
            status: string_to_cstring(format!("{:?}", state.status)),
            started_at: state
                .started_at
                .map(|dt| string_to_cstring(dt.to_rfc3339()))
                .unwrap_or(ptr::null_mut()),
            last_active: string_to_cstring(state.last_active.to_rfc3339()),
            properties_json: serde_json::to_string(&state.properties)
                .ok()
                .map(string_to_cstring)
                .unwrap_or(ptr::null_mut()),
        }
    }
}

// ============================================================================
// Memory Deallocation
// ============================================================================

/// Free a ThymosMemory structure.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_memory(m: *mut ThymosMemory) {
    if !m.is_null() {
        let mut mem = Box::from_raw(m);
        mem.free_fields();
    }
}

/// Free a ThymosSearchResults structure.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_search_results(results: *mut ThymosSearchResults) {
    if !results.is_null() {
        let sr = Box::from_raw(results);
        if !sr.memories.is_null() && sr.count > 0 {
            let memories = Vec::from_raw_parts(sr.memories, sr.count, sr.capacity);
            for mut mem in memories {
                mem.free_fields();
            }
        }
    }
}

/// Free a ThymosAgent handle.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_agent(handle: *mut ThymosAgent) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Free a ThymosMemoryConfig handle.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_memory_config(handle: *mut ThymosMemoryConfig) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Free a ThymosConfigHandle.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_config(handle: *mut ThymosConfigHandle) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Free a ThymosAgentState structure.
///
/// # Safety
/// The pointer must be valid and allocated by Thymos, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_free_agent_state(state: *mut ThymosAgentState) {
    if !state.is_null() {
        let s = Box::from_raw(state);
        thymos_free_string(s.status);
        thymos_free_string(s.started_at);
        thymos_free_string(s.last_active);
        thymos_free_string(s.properties_json);
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Create a default memory configuration.
///
/// Returns a handle to the configuration, or null on error.
/// Must be freed with `thymos_free_memory_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_memory_config_new() -> *mut ThymosMemoryConfig {
    Box::into_raw(Box::new(ThymosMemoryConfig {
        inner: MemoryConfig::default(),
    }))
}

/// Create a memory configuration with a custom data directory.
///
/// # Safety
/// `data_dir` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_memory_config_with_data_dir(
    data_dir: *const c_char,
) -> *mut ThymosMemoryConfig {
    let Some(dir) = cstr_to_string(data_dir) else {
        set_error("Invalid data_dir: not valid UTF-8");
        return ptr::null_mut();
    };

    let mut config = MemoryConfig::default();
    config.mode = MemoryMode::Embedded {
        data_dir: PathBuf::from(dir),
    };

    Box::into_raw(Box::new(ThymosMemoryConfig { inner: config }))
}

/// Create a memory configuration for server mode (connects to Locai server).
///
/// # Arguments
/// * `server_url` - URL of the Locai server (e.g., "http://localhost:3000")
/// * `api_key` - Optional API key for authentication (can be null)
///
/// # Safety
/// `server_url` must be a valid null-terminated UTF-8 string.
/// `api_key` can be null or a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_memory_config_server(
    server_url: *const c_char,
    api_key: *const c_char,
) -> *mut ThymosMemoryConfig {
    let Some(url) = cstr_to_string(server_url) else {
        set_error("Invalid server_url: not valid UTF-8");
        return ptr::null_mut();
    };

    let api_key = cstr_to_string(api_key);

    let mut config = MemoryConfig::default();
    config.mode = MemoryMode::Server { url, api_key };

    Box::into_raw(Box::new(ThymosMemoryConfig { inner: config }))
}

/// Create a memory configuration for hybrid mode (private + shared).
///
/// # Arguments
/// * `private_data_dir` - Directory for private embedded storage
/// * `shared_url` - URL of the shared Locai server
/// * `shared_api_key` - Optional API key for shared server (can be null)
///
/// # Safety
/// `private_data_dir` and `shared_url` must be valid null-terminated UTF-8 strings.
/// `shared_api_key` can be null or a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_memory_config_hybrid(
    private_data_dir: *const c_char,
    shared_url: *const c_char,
    shared_api_key: *const c_char,
) -> *mut ThymosMemoryConfig {
    let Some(dir) = cstr_to_string(private_data_dir) else {
        set_error("Invalid private_data_dir: not valid UTF-8");
        return ptr::null_mut();
    };

    let Some(url) = cstr_to_string(shared_url) else {
        set_error("Invalid shared_url: not valid UTF-8");
        return ptr::null_mut();
    };

    let api_key = cstr_to_string(shared_api_key);

    let mut config = MemoryConfig::default();
    config.mode = MemoryMode::Hybrid {
        private_data_dir: PathBuf::from(dir),
        shared_url: url,
        shared_api_key: api_key,
    };

    Box::into_raw(Box::new(ThymosMemoryConfig { inner: config }))
}

/// Create a default Thymos configuration.
///
/// Returns a handle to the configuration, or null on error.
/// Must be freed with `thymos_free_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_config_new() -> *mut ThymosConfigHandle {
    Box::into_raw(Box::new(ThymosConfigHandle {
        inner: ThymosConfig::default(),
    }))
}

/// Load Thymos configuration from file and environment.
///
/// Searches for thymos.toml, thymos.yaml, or thymos.json in standard locations.
/// Environment variables with THYMOS_ prefix override file settings.
///
/// Returns a handle to the configuration, or null on error.
/// Must be freed with `thymos_free_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_config_load() -> *mut ThymosConfigHandle {
    match ThymosConfig::load() {
        Ok(config) => Box::into_raw(Box::new(ThymosConfigHandle { inner: config })),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Load Thymos configuration from a specific file.
///
/// # Safety
/// `path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_config_load_from_file(
    path: *const c_char,
) -> *mut ThymosConfigHandle {
    let Some(path_str) = cstr_to_string(path) else {
        set_error("Invalid path: not valid UTF-8");
        return ptr::null_mut();
    };

    match ThymosConfig::from_file(&path_str) {
        Ok(config) => Box::into_raw(Box::new(ThymosConfigHandle { inner: config })),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

// ============================================================================
// Agent Creation
// ============================================================================

/// Create a new agent with default configuration.
///
/// # Safety
/// `agent_id` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_new(agent_id: *const c_char) -> *mut ThymosAgent {
    let Some(id) = cstr_to_string(agent_id) else {
        set_error("Invalid agent_id: not valid UTF-8");
        return ptr::null_mut();
    };

    match block_on(async move { Agent::builder().id(id).build().await }) {
        Ok(agent) => Box::into_raw(Box::new(ThymosAgent { inner: agent })),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Create a new agent with custom memory configuration.
///
/// # Safety
/// `agent_id` must be a valid null-terminated UTF-8 string.
/// `config` must be a valid ThymosMemoryConfig handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_new_with_memory_config(
    agent_id: *const c_char,
    config: *const ThymosMemoryConfig,
) -> *mut ThymosAgent {
    let Some(id) = cstr_to_string(agent_id) else {
        set_error("Invalid agent_id: not valid UTF-8");
        return ptr::null_mut();
    };

    if config.is_null() {
        set_error("Memory config is null");
        return ptr::null_mut();
    }

    let memory_config = (*config).inner.clone();

    match block_on(async move {
        Agent::builder()
            .id(id)
            .with_memory_config(memory_config)
            .build()
            .await
    }) {
        Ok(agent) => Box::into_raw(Box::new(ThymosAgent { inner: agent })),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Create a new agent with full Thymos configuration.
///
/// # Safety
/// `agent_id` must be a valid null-terminated UTF-8 string.
/// `config` must be a valid ThymosConfigHandle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_new_with_config(
    agent_id: *const c_char,
    config: *const ThymosConfigHandle,
) -> *mut ThymosAgent {
    let Some(id) = cstr_to_string(agent_id) else {
        set_error("Invalid agent_id: not valid UTF-8");
        return ptr::null_mut();
    };

    if config.is_null() {
        set_error("Config is null");
        return ptr::null_mut();
    }

    let thymos_config = (*config).inner.clone();

    match block_on(async move {
        Agent::builder().id(id).config(thymos_config).build().await
    }) {
        Ok(agent) => Box::into_raw(Box::new(ThymosAgent { inner: agent })),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

// ============================================================================
// Agent Properties
// ============================================================================

/// Get the agent ID.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// The returned string must be freed with `thymos_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_id(handle: *const ThymosAgent) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    string_to_cstring((*handle).inner.id().to_string())
}

/// Get the agent description.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// The returned string must be freed with `thymos_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_description(handle: *const ThymosAgent) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    string_to_cstring((*handle).inner.description().to_string())
}

// ============================================================================
// Agent Status
// ============================================================================

/// Get agent status as a string.
///
/// Returns one of: "Active", "Listening", "Dormant", "Archived"
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// The returned string must be freed with `thymos_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_status(handle: *const ThymosAgent) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let agent = (*handle).inner.clone();
    let status = block_on_value(async move { agent.status().await });
    string_to_cstring(format!("{:?}", status))
}

/// Set agent status.
///
/// Valid statuses: "active", "listening", "dormant", "archived" (case-insensitive)
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// `status` must be a valid null-terminated UTF-8 string.
///
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_set_status(
    handle: *const ThymosAgent,
    status: *const c_char,
) -> c_int {
    if handle.is_null() {
        set_error("Agent handle is null");
        return -1;
    }

    let Some(status_str) = cstr_to_string(status) else {
        set_error("Invalid status: not valid UTF-8");
        return -1;
    };

    let agent_status = match status_str.to_lowercase().as_str() {
        "active" => AgentStatus::Active,
        "listening" => AgentStatus::Listening,
        "dormant" => AgentStatus::Dormant,
        "archived" => AgentStatus::Archived,
        _ => {
            set_error(format!(
                "Invalid status: {}. Valid values: active, listening, dormant, archived",
                status_str
            ));
            return -1;
        }
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.set_status(agent_status).await }) {
        Ok(_) => 0,
        Err(e) => {
            set_error(e.to_string());
            -1
        }
    }
}

/// Get full agent state.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// The returned state must be freed with `thymos_free_agent_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_state(handle: *const ThymosAgent) -> *mut ThymosAgentState {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let agent = (*handle).inner.clone();
    let state = block_on_value(async move { agent.state().await });

    Box::into_raw(Box::new(ThymosAgentState::from_state(&state)))
}

// ============================================================================
// Memory Operations
// ============================================================================

/// Store a memory.
///
/// Returns the memory ID on success, or null on error.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// `content` must be a valid null-terminated UTF-8 string.
/// The returned string must be freed with `thymos_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_remember(
    handle: *const ThymosAgent,
    content: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(content_str) = cstr_to_string(content) else {
        set_error("Invalid content: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.remember(content_str).await }) {
        Ok(id) => string_to_cstring(id),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Store a fact memory (durable, context-independent knowledge).
///
/// Facts are intended for knowledge like "Paris is the capital of France".
///
/// # Safety
/// Same as `thymos_agent_remember`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_remember_fact(
    handle: *const ThymosAgent,
    content: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(content_str) = cstr_to_string(content) else {
        set_error("Invalid content: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.remember_fact(content_str).await }) {
        Ok(id) => string_to_cstring(id),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Store a conversation memory (dialogue context).
///
/// Conversation memories are intended for dialogue history and ephemeral context.
///
/// # Safety
/// Same as `thymos_agent_remember`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_remember_conversation(
    handle: *const ThymosAgent,
    content: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(content_str) = cstr_to_string(content) else {
        set_error("Invalid content: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.remember_conversation(content_str).await }) {
        Ok(id) => string_to_cstring(id),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Store a memory in the private backend (hybrid mode only).
///
/// # Safety
/// Same as `thymos_agent_remember`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_remember_private(
    handle: *const ThymosAgent,
    content: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(content_str) = cstr_to_string(content) else {
        set_error("Invalid content: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.remember_private(content_str).await }) {
        Ok(id) => string_to_cstring(id),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Store a memory in the shared backend (hybrid mode only).
///
/// # Safety
/// Same as `thymos_agent_remember`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_remember_shared(
    handle: *const ThymosAgent,
    content: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(content_str) = cstr_to_string(content) else {
        set_error("Invalid content: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.remember_shared(content_str).await }) {
        Ok(id) => string_to_cstring(id),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

// ============================================================================
// Memory Search
// ============================================================================

/// Search memories.
///
/// # Arguments
/// * `handle` - Agent handle
/// * `query` - Search query string
/// * `limit` - Maximum number of results (0 = no limit)
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// `query` must be a valid null-terminated UTF-8 string.
/// The returned results must be freed with `thymos_free_search_results`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_search_memories(
    handle: *const ThymosAgent,
    query: *const c_char,
    limit: usize,
) -> *mut ThymosSearchResults {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(query_str) = cstr_to_string(query) else {
        set_error("Invalid query: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move {
        let mut memories = agent.search_memories(&query_str).await?;
        if limit > 0 {
            memories.truncate(limit);
        }
        Ok(memories)
    }) {
        Ok(memories) => {
            let mut results: Vec<ThymosMemory> = memories
                .iter()
                .map(|m| ThymosMemory::from_locai(m))
                .collect();

            let count = results.len();
            let capacity = results.capacity();
            let ptr = if count > 0 {
                let p = results.as_mut_ptr();
                std::mem::forget(results);
                p
            } else {
                ptr::null_mut()
            };

            Box::into_raw(Box::new(ThymosSearchResults {
                memories: ptr,
                count,
                capacity,
            }))
        }
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Search private memories (hybrid mode only).
///
/// # Safety
/// Same as `thymos_agent_search_memories`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_search_private(
    handle: *const ThymosAgent,
    query: *const c_char,
    limit: usize,
) -> *mut ThymosSearchResults {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(query_str) = cstr_to_string(query) else {
        set_error("Invalid query: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move {
        let mut memories = agent.search_private(&query_str).await?;
        if limit > 0 {
            memories.truncate(limit);
        }
        Ok(memories)
    }) {
        Ok(memories) => {
            let mut results: Vec<ThymosMemory> = memories
                .iter()
                .map(|m| ThymosMemory::from_locai(m))
                .collect();

            let count = results.len();
            let capacity = results.capacity();
            let ptr = if count > 0 {
                let p = results.as_mut_ptr();
                std::mem::forget(results);
                p
            } else {
                ptr::null_mut()
            };

            Box::into_raw(Box::new(ThymosSearchResults {
                memories: ptr,
                count,
                capacity,
            }))
        }
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Search shared memories (hybrid mode only).
///
/// # Safety
/// Same as `thymos_agent_search_memories`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_search_shared(
    handle: *const ThymosAgent,
    query: *const c_char,
    limit: usize,
) -> *mut ThymosSearchResults {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(query_str) = cstr_to_string(query) else {
        set_error("Invalid query: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move {
        let mut memories = agent.search_shared(&query_str).await?;
        if limit > 0 {
            memories.truncate(limit);
        }
        Ok(memories)
    }) {
        Ok(memories) => {
            let mut results: Vec<ThymosMemory> = memories
                .iter()
                .map(|m| ThymosMemory::from_locai(m))
                .collect();

            let count = results.len();
            let capacity = results.capacity();
            let ptr = if count > 0 {
                let p = results.as_mut_ptr();
                std::mem::forget(results);
                p
            } else {
                ptr::null_mut()
            };

            Box::into_raw(Box::new(ThymosSearchResults {
                memories: ptr,
                count,
                capacity,
            }))
        }
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Get a memory by ID.
///
/// Returns the memory on success, or null if not found or on error.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
/// `memory_id` must be a valid null-terminated UTF-8 string.
/// The returned memory must be freed with `thymos_free_memory`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_get_memory(
    handle: *const ThymosAgent,
    memory_id: *const c_char,
) -> *mut ThymosMemory {
    if handle.is_null() {
        set_error("Agent handle is null");
        return ptr::null_mut();
    }

    let Some(id) = cstr_to_string(memory_id) else {
        set_error("Invalid memory_id: not valid UTF-8");
        return ptr::null_mut();
    };

    let agent = (*handle).inner.clone();
    match block_on(async move { agent.get_memory(&id).await }) {
        Ok(Some(memory)) => Box::into_raw(Box::new(ThymosMemory::from_locai(&memory))),
        Ok(None) => ptr::null_mut(),
        Err(e) => {
            set_error(e.to_string());
            ptr::null_mut()
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get the Thymos library version.
///
/// # Safety
/// The returned string must be freed with `thymos_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_version() -> *mut c_char {
    string_to_cstring(thymos_core::VERSION.to_string())
}

/// Check if the memory system is in hybrid mode.
///
/// Returns 1 if hybrid mode, 0 otherwise.
///
/// # Safety
/// `handle` must be a valid ThymosAgent handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn thymos_agent_is_hybrid(handle: *const ThymosAgent) -> c_int {
    if handle.is_null() {
        set_error("Agent handle is null");
        return -1;
    }

    if (*handle).inner.memory().is_hybrid() {
        1
    } else {
        0
    }
}
