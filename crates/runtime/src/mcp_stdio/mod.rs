#![allow(unused_imports)] // re-exports for `crate::mcp_stdio::`

mod manager;
mod process;
mod types;

#[cfg(all(test, unix))]
mod tests;

pub use manager::McpServerManager;
pub use process::{spawn_mcp_stdio_process, McpStdioProcess};
pub use types::{
    JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, ManagedMcpTool, McpInitializeClientInfo,
    McpInitializeParams, McpInitializeResult, McpInitializeServerInfo, McpListResourcesParams,
    McpListResourcesResult, McpListToolsParams, McpListToolsResult, McpReadResourceParams,
    McpReadResourceResult, McpResource, McpResourceContents, McpServerManagerError, McpTool,
    McpToolCallContent, McpToolCallParams, McpToolCallResult, UnsupportedMcpServer,
};
