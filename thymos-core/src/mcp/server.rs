//! MCP Server Implementation
//!
//! The main server that handles MCP requests and dispatches to tools and resources.

use super::protocol::*;
use super::resources::ResourceProvider;
use super::transport::Transport;
use crate::skills::{PromptTemplate, Skill};
use crate::tools::{CapabilityPolicy, Tool, ToolExecutionContext, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Whether to expose tools
    pub enable_tools: bool,
    /// Whether to expose resources
    pub enable_resources: bool,
    /// Whether to expose prompts
    pub enable_prompts: bool,
    /// Tool allowlist (None = allow all registered tools)
    pub tool_allowlist: Option<Vec<String>>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "thymos-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            enable_tools: true,
            enable_resources: true,
            enable_prompts: true,
            tool_allowlist: None,
        }
    }
}

/// MCP Server state
pub struct McpServer {
    config: McpServerConfig,
    tools: Arc<RwLock<ToolRegistry>>,
    resources: Arc<RwLock<Vec<Box<dyn ResourceProvider>>>>,
    prompts: Arc<RwLock<HashMap<String, PromptTemplate>>>,
    policy: Option<CapabilityPolicy>,
    initialized: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("config", &self.config)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl McpServer {
    /// Create a new MCP server builder
    pub fn builder() -> McpServerBuilder {
        McpServerBuilder::new()
    }

    /// Create a new MCP server with default config
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Handle an incoming JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => {
                // Notification, no response needed but we return success
                JsonRpcResponse::success(request.id, Value::Null)
            }
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request).await,
            "resources/read" => self.handle_resources_read(request).await,
            "prompts/list" => self.handle_prompts_list(request).await,
            "prompts/get" => self.handle_prompts_get(request).await,
            _ => JsonRpcResponse::error(request.id, JsonRpcError::method_not_found()),
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params: InitializeParams = match request.params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        JsonRpcError::invalid_params(format!("Invalid initialize params: {}", e)),
                    );
                }
            },
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing initialize params"),
                );
            }
        };

        // Store client info if needed (for future use)
        let _ = params.client_info;

        // Build server capabilities based on config
        let mut capabilities = ServerCapabilities::default();

        if self.config.enable_tools {
            capabilities.tools = Some(ToolsCapability { list_changed: false });
        }

        if self.config.enable_resources {
            capabilities.resources = Some(ResourcesCapability {
                subscribe: false,
                list_changed: false,
            });
        }

        if self.config.enable_prompts {
            capabilities.prompts = Some(PromptsCapability { list_changed: false });
        }

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities,
            server_info: ServerInfo {
                name: self.config.name.clone(),
                version: self.config.version.clone(),
            },
        };

        *self.initialized.write().await = true;

        JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_tools {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Tools not enabled"),
            );
        }

        let tools = self.tools.read().await;

        // Get MCP tool info, optionally filtered by policy
        let mcp_tools = match &self.policy {
            Some(policy) => tools.mcp_tools_filtered(policy),
            None => tools.mcp_tools(),
        };

        // Apply allowlist if configured
        let filtered_tools: Vec<McpTool> = match &self.config.tool_allowlist {
            Some(allowlist) => mcp_tools
                .into_iter()
                .filter(|t| allowlist.contains(&t.name))
                .map(|t| McpTool {
                    name: t.name,
                    description: t.description,
                    input_schema: t.input_schema,
                })
                .collect(),
            None => mcp_tools
                .into_iter()
                .map(|t| McpTool {
                    name: t.name,
                    description: t.description,
                    input_schema: t.input_schema,
                })
                .collect(),
        };

        let result = ToolsListResult {
            tools: filtered_tools,
        };

        JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_tools {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Tools not enabled"),
            );
        }

        let params: ToolCallParams = match request.params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        JsonRpcError::invalid_params(format!("Invalid tool call params: {}", e)),
                    );
                }
            },
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool call params"),
                );
            }
        };

        // Check allowlist
        if let Some(allowlist) = &self.config.tool_allowlist {
            if !allowlist.contains(&params.name) {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::new(-32002, format!("Tool '{}' not allowed", params.name)),
                );
            }
        }

        let tools = self.tools.read().await;
        let tool = match tools.get(&params.name) {
            Some(t) => Arc::clone(t),
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::new(-32002, format!("Tool '{}' not found", params.name)),
                );
            }
        };

        // Check policy
        if let Some(policy) = &self.policy {
            let required = tool.required_capabilities();
            if policy.check_all(&required).is_err() {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::new(-32003, format!("Tool '{}' not permitted by policy", params.name)),
                );
            }
        }

        // Execute tool
        let ctx = ToolExecutionContext::new();
        let result = match tool.execute(params.arguments, &ctx).await {
            Ok(envelope) => {
                match &envelope.result {
                    crate::tools::ToolResult::Success { value } => {
                        ToolCallResult {
                            content: vec![ContentBlock::text(
                                serde_json::to_string_pretty(value).unwrap_or_default()
                            )],
                            is_error: None,
                        }
                    }
                    crate::tools::ToolResult::Error { error } => {
                        ToolCallResult {
                            content: vec![ContentBlock::text(format!("Error: {}", error.message))],
                            is_error: Some(true),
                        }
                    }
                    crate::tools::ToolResult::Cancelled { reason } => {
                        ToolCallResult {
                            content: vec![ContentBlock::text(format!("Cancelled: {}", reason))],
                            is_error: Some(true),
                        }
                    }
                }
            }
            Err(e) => ToolCallResult {
                content: vec![ContentBlock::text(format!("Error: {}", e))],
                is_error: Some(true),
            },
        };

        JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    /// Handle resources/list request
    async fn handle_resources_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_resources {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Resources not enabled"),
            );
        }

        let providers = self.resources.read().await;
        let mut resources = Vec::new();

        for provider in providers.iter() {
            resources.extend(provider.list_resources().await);
        }

        let result = ResourcesListResult { resources };

        JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    /// Handle resources/read request
    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_resources {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Resources not enabled"),
            );
        }

        let params: ResourceReadParams = match request.params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        JsonRpcError::invalid_params(format!("Invalid resource read params: {}", e)),
                    );
                }
            },
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing resource read params"),
                );
            }
        };

        let providers = self.resources.read().await;

        for provider in providers.iter() {
            if let Some(content) = provider.read_resource(&params.uri).await {
                let result = ResourceReadResult {
                    contents: vec![content],
                };
                return JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result).unwrap_or(Value::Null),
                );
            }
        }

        JsonRpcResponse::error(
            request.id,
            JsonRpcError::new(-32002, format!("Resource '{}' not found", params.uri)),
        )
    }

    /// Handle prompts/list request
    async fn handle_prompts_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_prompts {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Prompts not enabled"),
            );
        }

        let prompts = self.prompts.read().await;

        let mcp_prompts: Vec<McpPrompt> = prompts
            .values()
            .map(|p| McpPrompt {
                name: p.name.clone(),
                description: p.description.clone(),
                arguments: Vec::new(),
            })
            .collect();

        let result = PromptsListResult { prompts: mcp_prompts };

        JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    /// Handle prompts/get request
    async fn handle_prompts_get(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.config.enable_prompts {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32001, "Prompts not enabled"),
            );
        }

        let params: PromptGetParams = match request.params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        JsonRpcError::invalid_params(format!("Invalid prompt get params: {}", e)),
                    );
                }
            },
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing prompt get params"),
                );
            }
        };

        let prompts = self.prompts.read().await;

        match prompts.get(&params.name) {
            Some(prompt) => {
                // Render the prompt with provided arguments
                let args_map: HashMap<String, String> = match params.arguments {
                    Value::Object(map) => map
                        .into_iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                        .collect(),
                    _ => HashMap::new(),
                };

                let rendered = prompt.render(&args_map);

                let result = PromptGetResult {
                    description: prompt.description.clone(),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: ContentBlock::text(rendered),
                    }],
                };

                JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result).unwrap_or(Value::Null),
                )
            }
            None => JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(-32002, format!("Prompt '{}' not found", params.name)),
            ),
        }
    }

    /// Run the server with a transport
    pub async fn run<T: Transport>(&self, mut transport: T) -> crate::error::Result<()> {
        loop {
            match transport.receive().await {
                Ok(Some(request)) => {
                    let response = self.handle_request(request).await;
                    transport.send(response).await?;
                }
                Ok(None) => {
                    // Connection closed
                    break;
                }
                Err(e) => {
                    tracing::error!("Transport error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Get a reference to the tool registry
    pub fn tools(&self) -> &Arc<RwLock<ToolRegistry>> {
        &self.tools
    }

    /// Register a tool
    pub async fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), crate::tools::RegistryError> {
        let mut tools = self.tools.write().await;
        tools.register(tool)
    }

    /// Register a skill's tools
    pub async fn register_skill(&self, skill: &Skill) -> Result<(), crate::tools::RegistryError> {
        let mut tools = self.tools.write().await;
        for tool in skill.tools() {
            tools.register(Arc::clone(tool))?;
        }

        // Also register prompts
        let mut prompts = self.prompts.write().await;
        for (name, prompt) in skill.prompts() {
            prompts.insert(name.clone(), prompt.clone());
        }

        Ok(())
    }

    /// Add a resource provider
    pub async fn add_resource_provider(&self, provider: Box<dyn ResourceProvider>) {
        let mut resources = self.resources.write().await;
        resources.push(provider);
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for MCP Server
pub struct McpServerBuilder {
    config: McpServerConfig,
    tools: ToolRegistry,
    resources: Vec<Box<dyn ResourceProvider>>,
    prompts: HashMap<String, PromptTemplate>,
    policy: Option<CapabilityPolicy>,
}

impl McpServerBuilder {
    pub fn new() -> Self {
        Self {
            config: McpServerConfig::default(),
            tools: ToolRegistry::new(),
            resources: Vec::new(),
            prompts: HashMap::new(),
            policy: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.config.version = version.into();
        self
    }

    pub fn with_tools(mut self, registry: ToolRegistry) -> Self {
        self.tools = registry;
        self
    }

    pub fn with_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        let _ = self.tools.register(tool);
        self
    }

    pub fn with_resource_provider(mut self, provider: Box<dyn ResourceProvider>) -> Self {
        self.resources.push(provider);
        self
    }

    pub fn with_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompts.insert(prompt.name.clone(), prompt);
        self
    }

    pub fn with_skill(mut self, skill: &Skill) -> Self {
        for tool in skill.tools() {
            let _ = self.tools.register(Arc::clone(tool));
        }
        for (name, prompt) in skill.prompts() {
            self.prompts.insert(name.clone(), prompt.clone());
        }
        self
    }

    pub fn with_policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn with_tool_allowlist(mut self, tools: Vec<String>) -> Self {
        self.config.tool_allowlist = Some(tools);
        self
    }

    pub fn enable_tools(mut self, enable: bool) -> Self {
        self.config.enable_tools = enable;
        self
    }

    pub fn enable_resources(mut self, enable: bool) -> Self {
        self.config.enable_resources = enable;
        self
    }

    pub fn enable_prompts(mut self, enable: bool) -> Self {
        self.config.enable_prompts = enable;
        self
    }

    pub fn build(self) -> McpServer {
        McpServer {
            config: self.config,
            tools: Arc::new(RwLock::new(self.tools)),
            resources: Arc::new(RwLock::new(self.resources)),
            prompts: Arc::new(RwLock::new(self.prompts)),
            policy: self.policy,
            initialized: Arc::new(RwLock::new(false)),
        }
    }
}

impl Default for McpServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{CapabilitySet, ToolMetadata, ToolProvenance, ToolResultEnvelope, ToolSchema, ToolError};
    use async_trait::async_trait;

    struct EchoTool {
        metadata: ToolMetadata,
    }

    impl EchoTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("echo", "Echoes back the input"),
            }
        }
    }

    #[async_trait]
    impl Tool for EchoTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::new(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }))
        }

        fn required_capabilities(&self) -> CapabilitySet {
            CapabilitySet::new()
        }

        async fn execute(
            &self,
            args: Value,
            _ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, ToolError> {
            let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("no message");
            let provenance = ToolProvenance::new("echo", "mcp-test");
            Ok(ToolResultEnvelope::success(
                serde_json::json!({ "echo": message }),
                provenance,
            ))
        }
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = McpServer::builder()
            .name("test-server")
            .version("1.0.0")
            .build();

        assert_eq!(server.config.name, "test-server");
        assert_eq!(server.config.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_initialize() {
        let server = McpServer::new();

        let request = JsonRpcRequest::new(1i64, "initialize")
            .with_params(serde_json::json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0"
                }
            }));

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(result["serverInfo"]["name"].is_string());
    }

    #[tokio::test]
    async fn test_tools_list() {
        let server = McpServer::builder()
            .with_tool(Arc::new(EchoTool::new()))
            .build();

        let request = JsonRpcRequest::new(1i64, "tools/list");
        let response = server.handle_request(request).await;

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[tokio::test]
    async fn test_tools_call() {
        let server = McpServer::builder()
            .with_tool(Arc::new(EchoTool::new()))
            .build();

        let request = JsonRpcRequest::new(1i64, "tools/call")
            .with_params(serde_json::json!({
                "name": "echo",
                "arguments": { "message": "hello" }
            }));

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_tool_allowlist() {
        let server = McpServer::builder()
            .with_tool(Arc::new(EchoTool::new()))
            .with_tool_allowlist(vec!["other_tool".to_string()])
            .build();

        // echo is not in allowlist
        let request = JsonRpcRequest::new(1i64, "tools/list");
        let response = server.handle_request(request).await;
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let server = McpServer::new();

        let request = JsonRpcRequest::new(1i64, "nonexistent/method");
        let response = server.handle_request(request).await;

        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_prompts() {
        let prompt = PromptTemplate::new("greeting", "Hello, {{name}}!");
        let server = McpServer::builder()
            .with_prompt(prompt)
            .build();

        // List prompts
        let request = JsonRpcRequest::new(1i64, "prompts/list");
        let response = server.handle_request(request).await;
        let list_result = response.result.unwrap();
        let prompts = list_result["prompts"].as_array().unwrap();
        assert_eq!(prompts.len(), 1);

        // Get prompt
        let request = JsonRpcRequest::new(2i64, "prompts/get")
            .with_params(serde_json::json!({
                "name": "greeting",
                "arguments": { "name": "World" }
            }));

        let response = server.handle_request(request).await;
        let get_result = response.result.unwrap();
        let text = &get_result["messages"][0]["content"]["text"];
        assert_eq!(text, "Hello, World!");
    }
}
