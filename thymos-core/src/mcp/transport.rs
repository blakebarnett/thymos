//! MCP Transport Implementations
//!
//! Transports handle the I/O for MCP communication.

use super::protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError, RequestId};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};

/// Transport trait for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Receive a request from the transport
    async fn receive(&mut self) -> crate::error::Result<Option<JsonRpcRequest>>;

    /// Send a response through the transport
    async fn send(&mut self, response: JsonRpcResponse) -> crate::error::Result<()>;
}

/// Stdio transport for MCP (used by Claude Desktop)
///
/// Messages are sent as newline-delimited JSON on stdin/stdout.
pub struct StdioTransport {
    stdin: BufReader<Stdin>,
    stdout: Stdout,
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new() -> Self {
        Self {
            stdin: BufReader::new(tokio::io::stdin()),
            stdout: tokio::io::stdout(),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn receive(&mut self) -> crate::error::Result<Option<JsonRpcRequest>> {
        let mut line = String::new();

        match self.stdin.read_line(&mut line).await {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    // Skip empty lines, try to read next
                    return self.receive().await;
                }

                match serde_json::from_str(trimmed) {
                    Ok(request) => Ok(Some(request)),
                    Err(e) => {
                        // Send parse error response
                        let error_response = JsonRpcResponse::error(
                            RequestId::Null,
                            JsonRpcError::parse_error(),
                        );
                        let json = serde_json::to_string(&error_response).unwrap_or_default();
                        let _ = self.stdout.write_all(format!("{}\n", json).as_bytes()).await;
                        let _ = self.stdout.flush().await;

                        Err(crate::error::ThymosError::Configuration(format!(
                            "Failed to parse JSON-RPC request: {}",
                            e
                        )))
                    }
                }
            }
            Err(e) => Err(crate::error::ThymosError::Configuration(format!(
                "Failed to read from stdin: {}",
                e
            ))),
        }
    }

    async fn send(&mut self, response: JsonRpcResponse) -> crate::error::Result<()> {
        let json = serde_json::to_string(&response).map_err(|e| {
            crate::error::ThymosError::Configuration(format!("Failed to serialize response: {}", e))
        })?;

        self.stdout
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| {
                crate::error::ThymosError::Configuration(format!("Failed to write to stdout: {}", e))
            })?;

        self.stdout.flush().await.map_err(|e| {
            crate::error::ThymosError::Configuration(format!("Failed to flush stdout: {}", e))
        })?;

        Ok(())
    }
}

/// In-memory transport for testing
pub struct MemoryTransport {
    requests: std::collections::VecDeque<JsonRpcRequest>,
    responses: Vec<JsonRpcResponse>,
}

impl MemoryTransport {
    /// Create a new memory transport
    pub fn new() -> Self {
        Self {
            requests: std::collections::VecDeque::new(),
            responses: Vec::new(),
        }
    }

    /// Add a request to be received
    pub fn push_request(&mut self, request: JsonRpcRequest) {
        self.requests.push_back(request);
    }

    /// Get all sent responses
    pub fn responses(&self) -> &[JsonRpcResponse] {
        &self.responses
    }

    /// Take the last response
    pub fn pop_response(&mut self) -> Option<JsonRpcResponse> {
        self.responses.pop()
    }
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MemoryTransport {
    async fn receive(&mut self) -> crate::error::Result<Option<JsonRpcRequest>> {
        Ok(self.requests.pop_front())
    }

    async fn send(&mut self, response: JsonRpcResponse) -> crate::error::Result<()> {
        self.responses.push(response);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::RequestId;

    #[tokio::test]
    async fn test_memory_transport() {
        let mut transport = MemoryTransport::new();

        // Push a request
        transport.push_request(JsonRpcRequest::new(1i64, "tools/list"));

        // Receive it
        let request = transport.receive().await.unwrap();
        assert!(request.is_some());
        assert_eq!(request.unwrap().method, "tools/list");

        // Send a response
        let response = JsonRpcResponse::success(RequestId::Number(1), serde_json::json!({}));
        transport.send(response).await.unwrap();

        assert_eq!(transport.responses().len(), 1);
    }

    #[tokio::test]
    async fn test_memory_transport_empty() {
        let mut transport = MemoryTransport::new();
        let request = transport.receive().await.unwrap();
        assert!(request.is_none());
    }
}
