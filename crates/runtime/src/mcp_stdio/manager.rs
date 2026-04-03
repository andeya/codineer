use std::collections::BTreeMap;

use serde_json::Value as JsonValue;

use crate::config::{McpTransport, RuntimeConfig, ScopedMcpServerConfig};
use crate::mcp::mcp_tool_name;
use crate::mcp_client::McpClientBootstrap;

use super::process::{default_initialize_params, spawn_mcp_stdio_process, McpStdioProcess};
use super::types::{
    JsonRpcId, JsonRpcResponse, ManagedMcpTool, McpListToolsParams, McpServerManagerError,
    McpToolCallParams, McpToolCallResult, UnsupportedMcpServer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolRoute {
    pub(crate) server_name: String,
    pub(crate) raw_name: String,
}

#[derive(Debug)]
pub(crate) struct ManagedMcpServer {
    pub(crate) bootstrap: McpClientBootstrap,
    pub(crate) process: Option<McpStdioProcess>,
    pub(crate) initialized: bool,
}

impl ManagedMcpServer {
    pub(crate) fn new(bootstrap: McpClientBootstrap) -> Self {
        Self {
            bootstrap,
            process: None,
            initialized: false,
        }
    }
}

#[derive(Debug)]
pub struct McpServerManager {
    servers: BTreeMap<String, ManagedMcpServer>,
    unsupported_servers: Vec<UnsupportedMcpServer>,
    tool_index: BTreeMap<String, ToolRoute>,
    next_request_id: u64,
}

impl McpServerManager {
    #[must_use]
    pub fn from_runtime_config(config: &RuntimeConfig) -> Self {
        Self::from_servers(config.mcp().servers())
    }

    #[must_use]
    pub fn from_servers(servers: &BTreeMap<String, ScopedMcpServerConfig>) -> Self {
        let mut managed_servers = BTreeMap::new();
        let mut unsupported_servers = Vec::new();

        for (server_name, server_config) in servers {
            if server_config.transport() == McpTransport::Stdio {
                let bootstrap = McpClientBootstrap::from_scoped_config(server_name, server_config);
                managed_servers.insert(server_name.clone(), ManagedMcpServer::new(bootstrap));
            } else {
                unsupported_servers.push(UnsupportedMcpServer {
                    server_name: server_name.clone(),
                    transport: server_config.transport(),
                    reason: format!(
                        "transport {:?} is not supported by McpServerManager",
                        server_config.transport()
                    ),
                });
            }
        }

        Self {
            servers: managed_servers,
            unsupported_servers,
            tool_index: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    #[must_use]
    pub fn unsupported_servers(&self) -> &[UnsupportedMcpServer] {
        &self.unsupported_servers
    }

    pub async fn discover_tools(&mut self) -> Result<Vec<ManagedMcpTool>, McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        let mut discovered_tools = Vec::new();

        for server_name in server_names {
            self.ensure_server_ready(&server_name).await?;
            self.clear_routes_for_server(&server_name);

            let mut cursor = None;
            loop {
                let request_id = self.take_request_id();
                let response = {
                    let server = self.server_mut(&server_name)?;
                    let process = server.process.as_mut().ok_or_else(|| {
                        McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "tools/list",
                            details: "server process missing after initialization".to_string(),
                        }
                    })?;
                    process
                        .list_tools(
                            request_id,
                            Some(McpListToolsParams {
                                cursor: cursor.clone(),
                            }),
                        )
                        .await?
                };

                if let Some(error) = response.error {
                    return Err(McpServerManagerError::JsonRpc {
                        server_name: server_name.clone(),
                        method: "tools/list",
                        error,
                    });
                }

                let result =
                    response
                        .result
                        .ok_or_else(|| McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "tools/list",
                            details: "missing result payload".to_string(),
                        })?;

                for tool in result.tools {
                    let qualified_name = mcp_tool_name(&server_name, &tool.name);
                    self.tool_index.insert(
                        qualified_name.clone(),
                        ToolRoute {
                            server_name: server_name.clone(),
                            raw_name: tool.name.clone(),
                        },
                    );
                    discovered_tools.push(ManagedMcpTool {
                        server_name: server_name.clone(),
                        qualified_name,
                        raw_name: tool.name.clone(),
                        tool,
                    });
                }

                match result.next_cursor {
                    Some(next_cursor) => cursor = Some(next_cursor),
                    None => break,
                }
            }
        }

        Ok(discovered_tools)
    }

    pub async fn call_tool(
        &mut self,
        qualified_tool_name: &str,
        arguments: Option<JsonValue>,
    ) -> Result<JsonRpcResponse<McpToolCallResult>, McpServerManagerError> {
        let route = self
            .tool_index
            .get(qualified_tool_name)
            .cloned()
            .ok_or_else(|| McpServerManagerError::UnknownTool {
                qualified_name: qualified_tool_name.to_string(),
            })?;

        self.ensure_server_ready(&route.server_name).await?;
        let request_id = self.take_request_id();
        let response =
            {
                let server = self.server_mut(&route.server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: route.server_name.clone(),
                        method: "tools/call",
                        details: "server process missing after initialization".to_string(),
                    }
                })?;
                process
                    .call_tool(
                        request_id,
                        McpToolCallParams {
                            name: route.raw_name,
                            arguments,
                            meta: None,
                        },
                    )
                    .await?
            };
        Ok(response)
    }

    pub async fn shutdown(&mut self) -> Result<(), McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        for server_name in server_names {
            let server = self.server_mut(&server_name)?;
            if let Some(process) = server.process.as_mut() {
                process.shutdown().await?;
            }
            server.process = None;
            server.initialized = false;
        }
        Ok(())
    }

    fn clear_routes_for_server(&mut self, server_name: &str) {
        self.tool_index
            .retain(|_, route| route.server_name != server_name);
    }

    fn server_mut(
        &mut self,
        server_name: &str,
    ) -> Result<&mut ManagedMcpServer, McpServerManagerError> {
        self.servers
            .get_mut(server_name)
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })
    }

    fn take_request_id(&mut self) -> JsonRpcId {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        JsonRpcId::Number(id)
    }

    async fn ensure_server_ready(
        &mut self,
        server_name: &str,
    ) -> Result<(), McpServerManagerError> {
        let needs_spawn = self
            .servers
            .get(server_name)
            .map(|server| server.process.is_none())
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })?;

        if needs_spawn {
            let server = self.server_mut(server_name)?;
            server.process = Some(spawn_mcp_stdio_process(&server.bootstrap)?);
            server.initialized = false;
        }

        let needs_initialize = self
            .servers
            .get(server_name)
            .map(|server| !server.initialized)
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })?;

        if needs_initialize {
            let request_id = self.take_request_id();
            let response = {
                let server = self.server_mut(server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: server_name.to_string(),
                        method: "initialize",
                        details: "server process missing before initialize".to_string(),
                    }
                })?;
                process
                    .initialize(request_id, default_initialize_params())
                    .await?
            };

            if let Some(error) = response.error {
                return Err(McpServerManagerError::JsonRpc {
                    server_name: server_name.to_string(),
                    method: "initialize",
                    error,
                });
            }

            if response.result.is_none() {
                return Err(McpServerManagerError::InvalidResponse {
                    server_name: server_name.to_string(),
                    method: "initialize",
                    details: "missing result payload".to_string(),
                });
            }

            let server = self.server_mut(server_name)?;
            server.initialized = true;
        }

        Ok(())
    }
}
