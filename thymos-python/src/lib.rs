//! Python bindings for Thymos agent framework
//!
//! This module provides Python bindings using PyO3, allowing Python developers
//! to use Thymos agents with a Pythonic API.

#![allow(unsafe_op_in_unsafe_fn)] // PyO3 macros generate safe unsafe code
#![allow(clippy::useless_conversion)] // PyO3 PyResult type alias triggers false positives

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use thymos_core::agent::{Agent, AgentState, AgentStatus};
use thymos_core::config::{MemoryConfig, ThymosConfig};
use thymos_core::error::{Result, ThymosError};

/// Python exception for Thymos errors
#[derive(Debug)]
pub struct ThymosPythonError {
    message: String,
}

impl std::fmt::Display for ThymosPythonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ThymosPythonError {}

impl From<ThymosError> for ThymosPythonError {
    fn from(err: ThymosError) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<ThymosPythonError> for PyErr {
    fn from(err: ThymosPythonError) -> PyErr {
        PyException::new_err(err.message)
    }
}

/// Helper to convert Rust Result to Python Result
fn to_py_result<T>(result: Result<T>) -> PyResult<T>
where
    ThymosPythonError: From<ThymosError>,
{
    result.map_err(|e| ThymosPythonError::from(e).into())
}

/// Shared Tokio runtime for all Python calls
/// This ensures background tasks (like SurrealDB's) persist across calls
static RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
    Mutex::new(
        tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime for Python bindings")
    )
});

/// Helper to run async code in a blocking context using the shared runtime
/// Spawns the future as a task and waits for the result, keeping the runtime alive
fn block_on<F, T>(future: F) -> PyResult<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let rt_guard = RUNTIME.lock().unwrap();
    let handle = rt_guard.handle().clone();
    drop(rt_guard); // Release lock before blocking
    
    // Spawn the future as a task and wait for it using a channel
    // This ensures the runtime stays alive and background tasks continue
    let (tx, rx) = std::sync::mpsc::channel();
    handle.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });
    
    // Wait for the result (blocking call from Python)
    rx.recv()
        .map_err(|_| PyException::new_err("Failed to receive result from async task"))?
        .map_err(|e| ThymosPythonError::from(e).into())
}

/// Helper to run async code that returns a non-Result value
fn block_on_value<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let rt_guard = RUNTIME.lock().unwrap();
    let handle = rt_guard.handle().clone();
    drop(rt_guard);
    
    let (tx, rx) = std::sync::mpsc::channel();
    handle.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });
    
    rx.recv().expect("Failed to receive result from async task")
}

/// Python wrapper for Agent
#[pyclass]
pub struct PyAgent {
    inner: Agent,
}

#[pymethods]
impl PyAgent {
    /// Create a new agent with default configuration
    #[new]
    fn new(agent_id: String) -> PyResult<Self> {
        let agent_id_clone = agent_id.clone();
        let agent = block_on(async move {
            Agent::builder()
                .id(agent_id_clone)
                .build()
                .await
        })?;
        Ok(Self { inner: agent })
    }

    /// Create a new agent with custom memory configuration
    #[staticmethod]
    #[pyo3(signature = (agent_id, config))]
    fn with_memory_config(agent_id: String, config: &PyMemoryConfig) -> PyResult<Self> {
        let agent_id_clone = agent_id.clone();
        let memory_config = config.inner.clone();
        let agent = block_on(async move {
            Agent::builder()
                .id(agent_id_clone)
                .with_memory_config(memory_config)
                .build()
                .await
        })?;
        Ok(Self { inner: agent })
    }

    /// Create a new agent with full Thymos configuration
    #[staticmethod]
    #[pyo3(signature = (agent_id, config))]
    fn with_config(agent_id: String, config: &PyThymosConfig) -> PyResult<Self> {
        let agent_id_clone = agent_id.clone();
        let thymos_config = config.inner.clone();
        let agent = block_on(async move {
            Agent::builder()
                .id(agent_id_clone)
                .config(thymos_config)
                .build()
                .await
        })?;
        Ok(Self { inner: agent })
    }

    /// Get the agent ID
    fn id(&self) -> String {
        self.inner.id().to_string()
    }

    /// Store a memory
    fn remember(&self, content: String) -> PyResult<String> {
        let agent = self.inner.clone();
        block_on(async move { agent.remember(content).await })
    }

    /// Search memories
    #[pyo3(signature = (query, limit=None))]
    fn search_memories(&self, query: String, limit: Option<usize>) -> PyResult<Vec<PyMemory>> {
        let agent = self.inner.clone();
        let query_clone = query.clone();
        let limit_clone = limit;
        let results = block_on(async move {
            let mut memories = agent.search_memories(&query_clone).await?;
            if let Some(limit) = limit_clone {
                memories.truncate(limit);
            }
            Ok(memories)
        })?;
        Ok(results.into_iter().map(PyMemory::from).collect())
    }

    /// Search private memories (hybrid mode only)
    #[pyo3(signature = (query, limit=None))]
    fn search_private(&self, query: String, limit: Option<usize>) -> PyResult<Vec<PyMemory>> {
        let agent = self.inner.clone();
        let query_clone = query.clone();
        let limit_clone = limit;
        let results = block_on(async move {
            let mut memories = agent.search_private(&query_clone).await?;
            if let Some(limit) = limit_clone {
                memories.truncate(limit);
            }
            Ok(memories)
        })?;
        Ok(results.into_iter().map(PyMemory::from).collect())
    }

    /// Search shared memories (hybrid mode only)
    #[pyo3(signature = (query, limit=None))]
    fn search_shared(&self, query: String, limit: Option<usize>) -> PyResult<Vec<PyMemory>> {
        let agent = self.inner.clone();
        let query_clone = query.clone();
        let limit_clone = limit;
        let results = block_on(async move {
            let mut memories = agent.search_shared(&query_clone).await?;
            if let Some(limit) = limit_clone {
                memories.truncate(limit);
            }
            Ok(memories)
        })?;
        Ok(results.into_iter().map(PyMemory::from).collect())
    }

    /// Get a memory by ID
    fn get_memory(&self, memory_id: String) -> PyResult<Option<PyMemory>> {
        let agent = self.inner.clone();
        let memory_id_clone = memory_id.clone();
        let memory = block_on(async move { agent.get_memory(&memory_id_clone).await })?;
        Ok(memory.map(PyMemory::from))
    }

    /// Store a memory in private backend (hybrid mode only)
    fn remember_private(&self, content: String) -> PyResult<String> {
        let agent = self.inner.clone();
        block_on(async move { agent.remember_private(content).await })
    }

    /// Store a memory in shared backend (hybrid mode only)
    fn remember_shared(&self, content: String) -> PyResult<String> {
        let agent = self.inner.clone();
        block_on(async move { agent.remember_shared(content).await })
    }

    /// Get current agent state
    fn state(&self) -> PyResult<PyAgentState> {
        let agent = self.inner.clone();
        let state = block_on_value(async move { agent.state().await });
        Ok(PyAgentState { inner: state })
    }

    /// Get current agent status
    fn status(&self) -> PyResult<String> {
        let agent = self.inner.clone();
        let status = block_on_value(async move { agent.status().await });
        Ok(format!("{:?}", status))
    }

    /// Set agent status
    fn set_status(&self, status: String) -> PyResult<()> {
        let agent_status = match status.to_lowercase().as_str() {
            "active" => AgentStatus::Active,
            "listening" => AgentStatus::Listening,
            "dormant" => AgentStatus::Dormant,
            "archived" => AgentStatus::Archived,
            _ => {
                return Err(PyException::new_err(format!(
                    "Invalid status: {}. Must be one of: active, listening, dormant, archived",
                    status
                )));
            }
        };
        let agent = self.inner.clone();
        block_on(async move { agent.set_status(agent_status).await })?;
        Ok(())
    }
}

/// Python wrapper for Memory
#[pyclass]
#[derive(Clone)]
pub struct PyMemory {
    inner: locai::models::Memory,
}

impl From<locai::models::Memory> for PyMemory {
    fn from(memory: locai::models::Memory) -> Self {
        Self { inner: memory }
    }
}

#[pymethods]
impl PyMemory {
    /// Get memory ID
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Get memory content
    #[getter]
    fn content(&self) -> String {
        self.inner.content.clone()
    }

    /// Get memory properties/metadata
    #[getter]
    fn properties(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| serde_json_to_python(py, &self.inner.properties))
    }

    /// Get memory created_at timestamp
    #[getter]
    fn created_at(&self) -> String {
        self.inner.created_at.to_rfc3339()
    }

    /// Get memory last_accessed timestamp
    #[getter]
    fn last_accessed(&self) -> Option<String> {
        self.inner.last_accessed.map(|dt| dt.to_rfc3339())
    }

    /// Convert memory to dictionary
    fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new_bound(py);
        dict.set_item("id", self.id())?;
        dict.set_item("content", self.content())?;
        dict.set_item("properties", self.properties()?)?;
        dict.set_item("created_at", self.created_at())?;
        if let Some(last_accessed) = self.last_accessed() {
            dict.set_item("last_accessed", last_accessed)?;
        }
        Ok(dict.to_object(py))
    }
}

/// Python wrapper for AgentState
#[pyclass]
pub struct PyAgentState {
    inner: AgentState,
}

#[pymethods]
impl PyAgentState {
    /// Get status as string
    #[getter]
    fn status(&self) -> String {
        format!("{:?}", self.inner.status)
    }

    /// Get started_at timestamp
    #[getter]
    fn started_at(&self) -> Option<String> {
        self.inner.started_at.map(|dt| dt.to_rfc3339())
    }

    /// Get last_active timestamp
    #[getter]
    fn last_active(&self) -> String {
        self.inner.last_active.to_rfc3339()
    }

    /// Get properties
    #[getter]
    fn properties(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| serde_json_to_python(py, &self.inner.properties))
    }
}

/// Python wrapper for MemoryConfig
#[pyclass]
pub struct PyMemoryConfig {
    inner: MemoryConfig,
}

#[pymethods]
impl PyMemoryConfig {
    /// Create default memory config (embedded mode with default data directory)
    #[new]
    fn new() -> Self {
        Self {
            inner: MemoryConfig::default(),
        }
    }

    /// Create memory config with custom data directory (embedded mode)
    #[staticmethod]
    fn with_data_dir(data_dir: String) -> Self {
        use std::path::PathBuf;
        use thymos_core::config::MemoryMode;
        
        let mut config = MemoryConfig::default();
        config.mode = MemoryMode::Embedded {
            data_dir: PathBuf::from(data_dir),
        };
        Self { inner: config }
    }

    /// Create memory config for server mode (connects to Locai server)
    ///
    /// Args:
    ///     server_url: URL of the Locai server (e.g., "http://localhost:3000")
    ///     api_key: Optional API key for authentication
    #[staticmethod]
    #[pyo3(signature = (server_url, api_key=None))]
    fn server(server_url: String, api_key: Option<String>) -> Self {
        use thymos_core::config::MemoryMode;
        
        let mut config = MemoryConfig::default();
        config.mode = MemoryMode::Server {
            url: server_url,
            api_key,
        };
        Self { inner: config }
    }

    /// Create memory config for hybrid mode (private embedded + shared server)
    ///
    /// Args:
    ///     private_data_dir: Directory for private embedded storage
    ///     shared_url: URL of the shared Locai server
    ///     shared_api_key: Optional API key for shared server
    #[staticmethod]
    #[pyo3(signature = (private_data_dir, shared_url, shared_api_key=None))]
    fn hybrid(private_data_dir: String, shared_url: String, shared_api_key: Option<String>) -> Self {
        use std::path::PathBuf;
        use thymos_core::config::MemoryMode;
        
        let mut config = MemoryConfig::default();
        config.mode = MemoryMode::Hybrid {
            private_data_dir: PathBuf::from(private_data_dir),
            shared_url,
            shared_api_key,
        };
        Self { inner: config }
    }
}

/// Python wrapper for ThymosConfig
#[pyclass]
pub struct PyThymosConfig {
    inner: ThymosConfig,
}

#[pymethods]
impl PyThymosConfig {
    /// Create default config
    #[new]
    fn new() -> Self {
        Self {
            inner: ThymosConfig::default(),
        }
    }

    /// Load configuration from file and environment
    #[staticmethod]
    fn load() -> PyResult<Self> {
        let config = to_py_result(ThymosConfig::load())?;
        Ok(Self { inner: config })
    }

    /// Load configuration from a specific file
    #[staticmethod]
    fn from_file(path: String) -> PyResult<Self> {
        let config = to_py_result(ThymosConfig::from_file(path))?;
        Ok(Self { inner: config })
    }
}

/// Helper to convert serde_json::Value to Python object
fn serde_json_to_python(py: Python, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b.to_object(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_object(py))
            } else {
                Ok(n.to_string().to_object(py))
            }
        }
        serde_json::Value::String(s) => Ok(s.to_object(py)),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty_bound(py);
            for item in arr {
                list.append(serde_json_to_python(py, item)?)?;
            }
            Ok(list.to_object(py))
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new_bound(py);
            for (key, value) in obj {
                dict.set_item(key, serde_json_to_python(py, value)?)?;
            }
            Ok(dict.to_object(py))
        }
    }
}

/// Python module definition
#[pymodule]
fn thymos(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAgent>()?;
    m.add_class::<PyMemory>()?;
    m.add_class::<PyAgentState>()?;
    m.add_class::<PyMemoryConfig>()?;
    m.add_class::<PyThymosConfig>()?;
    Ok(())
}

