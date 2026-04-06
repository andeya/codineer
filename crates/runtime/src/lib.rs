mod bash;
mod compact;
mod config;
mod conversation;
pub mod credentials;
mod file_ops;
mod hooks;
mod json;
mod mcp;
mod mcp_client;
mod mcp_remote;
mod mcp_stdio;
mod oauth;
mod permissions;
mod prompt;
mod remote;
pub mod sandbox;
mod session;
mod usage;

pub use bash::{execute_bash, BashCommandInput, BashCommandOutput};
pub use compact::{
    compact_session, estimate_session_tokens, format_compact_summary,
    get_compact_continuation_message, should_compact, CompactionConfig, CompactionResult,
};
pub use config::{
    default_config_home, ConfigEntry, ConfigError, ConfigLoader, ConfigSource, CredentialConfig,
    CustomProviderConfig, McpConfigCollection, McpManagedProxyServerConfig, McpOAuthConfig,
    McpRemoteServerConfig, McpSdkServerConfig, McpServerConfig, McpStdioServerConfig, McpTransport,
    McpWebSocketServerConfig, OAuthConfig, ResolvedPermissionMode, RuntimeConfig,
    RuntimeFeatureConfig, RuntimeHookConfig, RuntimePluginConfig, ScopedMcpServerConfig,
    CODINEER_SETTINGS_SCHEMA_NAME,
};
pub use conversation::{
    ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError, StaticToolExecutor,
    ToolError, ToolExecutor, TurnSummary,
};
pub use file_ops::{
    edit_file, glob_search, grep_search, read_file, write_file, EditFileOutput, GlobSearchOutput,
    GrepOutputMode, GrepSearchInput, GrepSearchOutput, ReadFileOutput, StructuredPatchHunk,
    TextFilePayload, WriteFileOutput,
};
pub use hooks::{HookCommandSource, HookEvent, HookRunResult, HookRunner};
pub use json::JsonValue;
pub use lsp::{
    FileDiagnostics, LspContextEnrichment, LspError, LspManager, LspServerConfig, SymbolLocation,
    WorkspaceDiagnostics,
};
pub use mcp::{
    mcp_server_signature, mcp_tool_name, mcp_tool_prefix, normalize_name_for_mcp,
    scoped_mcp_config_hash, unwrap_mcp_proxy_url,
};
pub use mcp_client::McpClientBootstrap;
pub use mcp_stdio::{
    ManagedMcpTool, McpServerManager, McpServerManagerError, McpStdioProcess, McpTool,
    McpToolCallContent, McpToolCallResult, UnsupportedMcpServer,
};
pub use oauth::{
    clear_oauth_credentials, generate_pkce_pair, generate_state, load_oauth_credentials,
    loopback_redirect_uri, parse_oauth_callback_query, parse_oauth_callback_request_target,
    save_oauth_credentials, OAuthAuthorizationRequest, OAuthCallbackParams, OAuthRefreshRequest,
    OAuthTokenExchangeRequest, OAuthTokenSet, PkceCodePair,
};
pub use permissions::{
    PermissionMode, PermissionOutcome, PermissionPolicy, PermissionPromptDecision,
    PermissionPrompter, PermissionRequest,
};
pub use prompt::{
    load_system_prompt, load_system_prompt_with_lsp, ContextFile, ProjectContext,
    SystemPromptBuilder,
};
pub use remote::{
    inherited_upstream_proxy_env, RemoteSessionContext, UpstreamProxyBootstrap, UpstreamProxyState,
};

pub use credentials::{
    ClaudeCodeResolver, CodineerOAuthResolver, CredentialChain, CredentialError,
    CredentialResolver, CredentialStatus, EnvVarResolver, ResolvedCredential,
};
pub use session::{ContentBlock, ConversationMessage, MessageRole, Session, SessionError};
pub use usage::{format_usd, pricing_for_model, TokenUsage, UsageCostEstimate, UsageTracker};

/// Cross-platform home directory: tries `HOME` first (Unix + WSL), falls back to `USERPROFILE` (Windows).
#[must_use]
pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Returns the `.codineer/` directory to use for runtime artifacts (sessions, agents, todos,
/// sandbox dirs, etc.).
///
/// Walks up from `cwd` looking for an initialized project — one whose `.codineer/settings.json`
/// exists.  Falls back to `~/.codineer/` (always available after first launch) when no initialized
/// project is found in the ancestor chain.
#[must_use]
pub fn codineer_runtime_dir(cwd: &std::path::Path) -> std::path::PathBuf {
    for ancestor in cwd.ancestors() {
        let dir = ancestor.join(".codineer");
        if dir.join("settings.json").is_file() {
            return dir;
        }
    }
    home_dir()
        .map(|h| h.join(".codineer"))
        .unwrap_or_else(|| cwd.join(".codineer"))
}

#[cfg(test)]
fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
