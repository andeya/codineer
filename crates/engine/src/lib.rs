//! Agent engine for Aineer: session, config, prompt, sandbox, commands.

mod bash;
pub mod commands;
pub mod compact;
mod config;
mod conversation;
pub mod credentials;
mod file_ops;
mod hooks;
mod json;
pub mod model_context;
mod oauth;
mod permissions;
mod prompt;
pub mod recovery;
mod remote;
pub mod sandbox;
mod session;
pub mod streaming_tool_executor;
pub mod swarm;
pub mod token_budget;
mod tool;
pub mod tool_orchestration;
pub mod tool_result;
mod usage;

pub use aineer_lsp::{
    FileDiagnostics, LspContextEnrichment, LspError, LspManager, LspServerConfig, SymbolLocation,
    WorkspaceDiagnostics,
};
pub use aineer_mcp::{
    mcp_server_signature, mcp_tool_name, mcp_tool_prefix, normalize_name_for_mcp,
    scoped_mcp_config_hash, unwrap_mcp_proxy_url, ManagedMcpTool, McpClientBootstrap,
    McpRemoteClient, McpServerManager, McpServerManagerError, McpStdioProcess, McpTool,
    McpToolCallContent, McpToolCallResult, UnsupportedMcpServer,
};
pub use aineer_protocol::prompt_types::{
    BlockKind, CacheControl, CacheScope, CacheType, SystemBlock, ThinkingConfig, ThinkingMode,
};
pub use bash::{execute_bash, BashCommandInput, BashCommandOutput};
pub use compact::{
    apply_model_compact_summary, build_model_compact_request, compact_session,
    estimate_session_tokens, estimate_tokens, format_compact_summary,
    get_compact_continuation_message, should_compact, should_compact_for_model, CompactionConfig,
    CompactionResult, ModelCompactionConfig, ModelCompactionResult, COMPACT_SUMMARY_SYSTEM_PROMPT,
};
pub use config::{
    default_config_home, ConfigEntry, ConfigError, ConfigLoader, ConfigSource, CredentialConfig,
    CustomProviderConfig, GeminiCacheConfig, McpConfigCollection, McpManagedProxyServerConfig,
    McpOAuthConfig, McpRemoteServerConfig, McpSdkServerConfig, McpServerConfig,
    McpStdioServerConfig, McpTransport, McpWebSocketServerConfig, OAuthConfig,
    ResolvedPermissionMode, RuntimeConfig, RuntimeFeatureConfig, RuntimeHookConfig,
    RuntimePluginConfig, ScopedMcpServerConfig, AINEER_SETTINGS_SCHEMA_NAME,
};
pub use conversation::{
    assistant_text_from_stream_events, ApiClient, ApiRequest, AssistantEvent, ConversationRuntime,
    RuntimeError, StaticToolExecutor, ToolError, ToolErrorCode, ToolExecutor, TurnSummary,
};
pub use file_ops::{
    edit_file, glob_search, grep_search, read_file, workspace_safe_path, write_file,
    EditFileOutput, GlobSearchOutput, GrepOutputMode, GrepSearchInput, GrepSearchOutput,
    ReadFileOutput, StructuredPatchHunk, TextFilePayload, WriteFileOutput,
};
pub use hooks::HookDispatcher;
pub use json::JsonValue;
pub use model_context::{context_window_for_model, ModelContextWindow};
pub use oauth::{
    clear_oauth_credentials, generate_state, load_oauth_credentials, loopback_redirect_uri,
    save_oauth_credentials, OAuthAuthorizationRequest, OAuthCallbackParams, OAuthRefreshRequest,
    OAuthTokenExchangeRequest, OAuthTokenSet, PkceCodePair,
};
pub use permissions::{
    glob_matches, PermissionMode, PermissionOutcome, PermissionPolicy, PermissionPromptDecision,
    PermissionPrompter, PermissionRequest, PermissionRule, RuleDecision,
};
pub use prompt::{
    load_system_prompt, load_system_prompt_with_lsp, ContextFile, InstructionLoader,
    ProjectContext, PromptBuildError, PromptCache, SystemPromptBuilder,
};
pub use remote::{
    inherited_upstream_proxy_env, RemoteSessionContext, UpstreamProxyBootstrap, UpstreamProxyState,
};
pub use tool::{Tool, ToolResult};

pub use credentials::{
    AineerOAuthResolver, ClaudeCodeResolver, CredentialChain, CredentialError, CredentialResolver,
    CredentialStatus, EnvVarResolver, ResolvedCredential,
};
pub use session::{
    CacheLock, ContentBlock, ConversationMessage, MessageRole, Session, SessionError,
};
pub use usage::{format_usd, pricing_for_model, TokenUsage, UsageCostEstimate, UsageTracker};

/// Cross-platform home directory: tries `HOME` first (Unix + WSL), falls back to `USERPROFILE` (Windows).
#[must_use]
pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Finds the nearest `.aineer/` directory in `cwd` or its ancestors that contains
/// `settings.json`, indicating an initialized project.  Returns `None` when none is found.
#[must_use]
pub fn find_project_aineer_dir(cwd: &std::path::Path) -> Option<std::path::PathBuf> {
    cwd.ancestors().find_map(|ancestor| {
        let dir = ancestor.join(".aineer");
        dir.join("settings.json").is_file().then_some(dir)
    })
}

/// Returns the `.aineer/` directory to use for runtime artifacts (sessions, agents, todos,
/// sandbox dirs, etc.).
///
/// Uses the nearest initialized project's `.aineer/` (found by walking up the ancestor chain).
/// Falls back to `~/.aineer/` when no project is initialized; `cwd/.aineer/` as last resort.
#[must_use]
pub fn aineer_runtime_dir(cwd: &std::path::Path) -> std::path::PathBuf {
    find_project_aineer_dir(cwd)
        .or_else(|| home_dir().map(|h| h.join(".aineer")))
        .unwrap_or_else(|| cwd.join(".aineer"))
}

#[cfg(test)]
fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
