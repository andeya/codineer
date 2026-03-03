use crate::config::{McpServerConfig, ScopedMcpServerConfig};

const CLAUDEAI_SERVER_PREFIX: &str = "claude.ai ";
const CCR_PROXY_PATH_MARKERS: [&str; 2] = ["/v2/session_ingress/shttp/mcp/", "/v2/ccr-sessions/"];

#[must_use]
pub fn normalize_name_for_mcp(name: &str) -> String {
    let mut normalized = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();

    if name.starts_with(CLAUDEAI_SERVER_PREFIX) {
        normalized = collapse_underscores(&normalized)
            .trim_matches('_')
            .to_string();
    }

    normalized
}

#[must_use]
pub fn mcp_tool_prefix(server_name: &str) -> String {
    format!("mcp__{}__", normalize_name_for_mcp(server_name))
}

#[must_use]
pub fn mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "{}{}",
        mcp_tool_prefix(server_name),
        normalize_name_for_mcp(tool_name)
    )
}

#[must_use]
pub fn unwrap_ccr_proxy_url(url: &str) -> String {
    if !CCR_PROXY_PATH_MARKERS
        .iter()
        .any(|marker| url.contains(marker))
    {
        return url.to_string();
    }

    let Some(query_start) = url.find('?') else {
        return url.to_string();
    };
    let query = &url[query_start + 1..];
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if matches!(parts.next(), Some("mcp_url")) {
            if let Some(value) = parts.next() {
                return percent_decode(value);
            }
        }
    }

    url.to_string()
}

#[must_use]
pub fn mcp_server_signature(config: &McpServerConfig) -> Option<String> {
    match config {
        McpServerConfig::Stdio(config) => {
            let mut command = vec![config.command.clone()];
            command.extend(config.args.clone());
            Some(format!("stdio:{}", render_command_signature(&command)))
        }
        McpServerConfig::Sse(config) | McpServerConfig::Http(config) => {
            Some(format!("url:{}", unwrap_ccr_proxy_url(&config.url)))
        }
        McpServerConfig::Ws(config) => Some(format!("url:{}", unwrap_ccr_proxy_url(&config.url))),
        McpServerConfig::ManagedProxy(config) => {
            Some(format!("url:{}", unwrap_ccr_proxy_url(&config.url)))
        }
        McpServerConfig::Sdk(_) => None,
    }
}

#[must_use]
pub fn scoped_mcp_config_hash(config: &ScopedMcpServerConfig) -> String {
    let rendered = match &config.config {
        McpServerConfig::Stdio(stdio) => format!(
            "stdio|{}|{}|{}",
            stdio.command,
            render_command_signature(&stdio.args),
            render_env_signature(&stdio.env)
        ),
        McpServerConfig::Sse(remote) => format!(
            "sse|{}|{}|{}|{}",
            remote.url,
            render_env_signature(&remote.headers),
            remote.headers_helper.as_deref().unwrap_or(""),
            render_oauth_signature(remote.oauth.as_ref())
        ),
