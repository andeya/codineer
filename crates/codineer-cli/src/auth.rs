use std::env;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;

use api::{AuthSource, CodineerApiClient, ProviderKind};
use runtime::{
    generate_pkce_pair, generate_state, parse_oauth_callback_request_target,
    save_oauth_credentials, ClaudeCodeResolver, CodineerOAuthResolver, ConfigLoader,
    CredentialChain, EnvVarResolver, OAuthAuthorizationRequest, OAuthConfig, OAuthRefreshRequest,
    OAuthTokenExchangeRequest, RuntimeConfig,
};

const DEFAULT_OAUTH_CALLBACK_PORT: u16 = 4545;

pub fn default_oauth_config() -> OAuthConfig {
    OAuthConfig {
        client_id: String::from("df03b862-78fe-4a2b-bb24-426ac30897b7"),
        authorize_url: String::from("https://platform.codineer.dev/oauth/authorize"),
        token_url: String::from("https://platform.codineer.dev/v1/oauth/token"),
        callback_port: None,
        manual_redirect_url: None,
        scopes: vec![
            String::from("user:profile"),
            String::from("user:inference"),
            String::from("user:sessions:codineer"),
        ],
    }
}

// ---------------------------------------------------------------------------
// Provider name resolution
// ---------------------------------------------------------------------------

const KNOWN_PROVIDERS: &[(&str, ProviderKind)] = &[
    ("anthropic", ProviderKind::CodineerApi),
    ("claude", ProviderKind::CodineerApi),
    ("xai", ProviderKind::Xai),
    ("grok", ProviderKind::Xai),
    ("openai", ProviderKind::OpenAi),
];

fn resolve_provider_name(name: &str) -> Result<ProviderKind, String> {
    let lower = name.to_ascii_lowercase();
    KNOWN_PROVIDERS
        .iter()
        .find(|(alias, _)| *alias == lower)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| {
            let names: Vec<&str> = KNOWN_PROVIDERS.iter().map(|(n, _)| *n).collect();
            format!(
                "unknown provider: {name}\nAvailable providers: {}",
                names.join(", ")
            )
        })
}

fn provider_display_name(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::CodineerApi => "Anthropic",
        ProviderKind::Xai => "xAI",
        ProviderKind::OpenAi => "OpenAI",
        ProviderKind::Custom => "Custom",
    }
}

// ---------------------------------------------------------------------------
// Credential chain assembly
// ---------------------------------------------------------------------------

/// Build the credential chain for a given provider kind.
pub fn build_credential_chain(kind: ProviderKind, config: &RuntimeConfig) -> CredentialChain {
    match kind {
        ProviderKind::CodineerApi => {
            let default_oauth = default_oauth_config();
            let oauth_config = config.oauth().cloned().unwrap_or(default_oauth);
            let cred_config = config.credentials();

            let refresh_fn = make_refresh_fn();
            let mut resolvers: Vec<Box<dyn runtime::CredentialResolver>> = vec![
                Box::new(EnvVarResolver::anthropic()),
                Box::new(
                    CodineerOAuthResolver::new(Some(oauth_config)).with_refresh_fn(refresh_fn),
                ),
            ];
            if cred_config.auto_discover && cred_config.claude_code_enabled {
                resolvers.push(Box::new(ClaudeCodeResolver::new()));
            }
            CredentialChain::new("Anthropic", resolvers)
        }
        ProviderKind::Xai => CredentialChain::new("xAI", vec![Box::new(EnvVarResolver::xai())]),
        ProviderKind::OpenAi => {
            CredentialChain::new("OpenAI", vec![Box::new(EnvVarResolver::openai())])
        }
        ProviderKind::Custom => CredentialChain::empty("Custom"),
    }
}

fn make_refresh_fn() -> runtime::credentials::oauth_resolver::RefreshFn {
    Arc::new(|config: &OAuthConfig, token_set: runtime::OAuthTokenSet| {
        let client =
            CodineerApiClient::from_auth(AuthSource::None).with_base_url(api::read_base_url());
        let refresh_token = token_set.refresh_token.clone().ok_or_else(|| {
            Box::<dyn std::error::Error + Send + Sync>::from("no refresh token available")
        })?;
        let request =
            OAuthRefreshRequest::from_config(config, refresh_token, Some(token_set.scopes.clone()));
        let refreshed =
            client_runtime_block_on(async { client.refresh_oauth_token(config, &request).await })
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(runtime::OAuthTokenSet {
            access_token: refreshed.access_token,
            refresh_token: refreshed.refresh_token.or(token_set.refresh_token),
            expires_at: refreshed.expires_at,
            scopes: refreshed.scopes,
        })
    })
}

fn client_runtime_block_on<F, T>(future: F) -> Result<T, api::ApiError>
where
    F: std::future::Future<Output = Result<T, api::ApiError>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Runtime::new()
            .map_err(api::ApiError::from)?
            .block_on(future),
    }
}

// ---------------------------------------------------------------------------
// Login / Logout / Status
// ---------------------------------------------------------------------------

fn resolve_provider_and_chain(
    provider: Option<&str>,
) -> Result<(ProviderKind, RuntimeConfig, CredentialChain), Box<dyn std::error::Error>> {
    let kind = match provider {
        Some(name) => resolve_provider_name(name)?,
        None => ProviderKind::CodineerApi,
    };
    let cwd = env::current_dir()?;
    let config = ConfigLoader::default_for(&cwd).load()?;
    let chain = build_credential_chain(kind, &config);
    Ok((kind, config, chain))
}

fn find_source<'a>(
    chain: &'a CredentialChain,
    source_id: &str,
) -> Result<&'a dyn runtime::CredentialResolver, Box<dyn std::error::Error>> {
    chain
        .get_resolver(source_id)
        .ok_or_else(|| format!("unknown credential source: {source_id}").into())
}

pub fn run_login(
    provider: Option<&str>,
    source: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (kind, config, chain) = resolve_provider_and_chain(provider)?;

    if let Some(source_id) = source {
        let resolver = find_source(&chain, source_id)?;
        if !resolver.supports_login() {
            return Err(format!(
                "credential source '{}' does not support interactive login",
                resolver.display_name()
            )
            .into());
        }
        return resolver.login();
    }

    if kind == ProviderKind::CodineerApi {
        return run_codineer_oauth_login(&config);
    }

    let login_sources = chain.login_sources();
    if login_sources.is_empty() {
        return Err(format!(
            "{} does not support interactive login. Set credentials via environment variables.",
            provider_display_name(kind)
        )
        .into());
    }
    login_sources[0].login()
}

pub fn run_logout(
    provider: Option<&str>,
    source: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (kind, _config, chain) = resolve_provider_and_chain(provider)?;

    if let Some(source_id) = source {
        let resolver = find_source(&chain, source_id)?;
        if !resolver.supports_login() {
            return Err(format!(
                "credential source '{}' does not support logout",
                resolver.display_name()
            )
            .into());
        }
        resolver.logout()?;
        println!("{} credentials cleared.", resolver.display_name());
        return Ok(());
    }

    if kind == ProviderKind::CodineerApi {
        runtime::clear_oauth_credentials()?;
        println!("Codineer OAuth credentials cleared.");
        return Ok(());
    }

    Err(format!(
        "{} does not support logout. Remove the environment variable to revoke credentials.",
        provider_display_name(kind)
    )
    .into())
}

pub fn run_status(provider: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let config = ConfigLoader::default_for(&cwd).load()?;

    let providers: Vec<ProviderKind> = match provider {
        Some(name) => vec![resolve_provider_name(name)?],
        None => vec![
            ProviderKind::CodineerApi,
            ProviderKind::Xai,
            ProviderKind::OpenAi,
        ],
    };

    for (i, kind) in providers.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let chain = build_credential_chain(*kind, &config);
        println!("{}:", provider_display_name(*kind));
        for status in chain.status() {
            let indicator = if status.available { "●" } else { "○" };
            let login_hint = if status.supports_login {
                " (supports login)"
            } else {
                ""
            };
            println!("  {indicator} {}{login_hint}", status.display_name);
        }
        match chain.resolve() {
            Ok(cred) => {
                let label = match cred {
                    runtime::ResolvedCredential::ApiKey(_) => "API key",
                    runtime::ResolvedCredential::BearerToken(_) => "Bearer token",
                    runtime::ResolvedCredential::ApiKeyAndBearer { .. } => "API key + Bearer token",
                };
                println!("  Active: {label}");
            }
            Err(_) => {
                println!("  Active: none");
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Codineer OAuth browser login flow
// ---------------------------------------------------------------------------

fn run_codineer_oauth_login(config: &RuntimeConfig) -> Result<(), Box<dyn std::error::Error>> {
    let default_oauth = default_oauth_config();
    let oauth = config.oauth().unwrap_or(&default_oauth);
    let callback_port = oauth.callback_port.unwrap_or(DEFAULT_OAUTH_CALLBACK_PORT);
    let redirect_uri = runtime::loopback_redirect_uri(callback_port);
    let pkce = generate_pkce_pair()?;
    let state = generate_state()?;
    let authorize_url =
        OAuthAuthorizationRequest::from_config(oauth, redirect_uri.clone(), state.clone(), &pkce)
            .build_url();

    println!("Starting Codineer OAuth login...");
    println!("Listening for callback on {redirect_uri}");
    if let Err(error) = open_browser(&authorize_url) {
        eprintln!("warning: failed to open browser automatically: {error}");
        println!("Open this URL manually:\n{authorize_url}");
    }

    let callback = wait_for_oauth_callback(callback_port)?;
    if let Some(error) = callback.error {
        let description = callback
            .error_description
            .unwrap_or_else(|| "authorization failed".to_string());
        return Err(io::Error::other(format!("{error}: {description}")).into());
    }
    let code = callback.code.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "callback did not include code")
    })?;
    let returned_state = callback.state.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "callback did not include state")
    })?;
    if returned_state != state {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "oauth state mismatch").into());
    }

    let client = CodineerApiClient::from_auth(AuthSource::None).with_base_url(api::read_base_url());
    let exchange_request =
        OAuthTokenExchangeRequest::from_config(oauth, code, state, pkce.verifier, redirect_uri);
    let token_set = client_runtime_block_on(async {
        client.exchange_oauth_code(oauth, &exchange_request).await
    })?;
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: token_set.access_token,
        refresh_token: token_set.refresh_token,
        expires_at: token_set.expires_at,
        scopes: token_set.scopes,
    })?;
    println!("Codineer OAuth login complete.");
    Ok(())
}

fn open_browser(url: &str) -> io::Result<()> {
    let commands = if cfg!(target_os = "macos") {
        vec![("open", vec![url])]
    } else if cfg!(target_os = "windows") {
        vec![("cmd", vec!["/C", "start", "", url])]
    } else {
        vec![("xdg-open", vec![url])]
    };
    for (program, args) in commands {
        match Command::new(program).args(args).spawn() {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no supported browser opener command found",
    ))
}

fn wait_for_oauth_callback(
    port: u16,
) -> Result<runtime::OAuthCallbackParams, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let (mut stream, _) = listener.accept()?;
    let mut buffer = [0_u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing callback request line")
    })?;
    let target = request_line.split_whitespace().nth(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "missing callback request target",
        )
    })?;
    let callback = parse_oauth_callback_request_target(target)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let body = if callback.error.is_some() {
        "Codineer OAuth login failed. You can close this window."
    } else {
        "Codineer OAuth login succeeded. You can close this window."
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    Ok(callback)
}

// ---------------------------------------------------------------------------
// Error hints
// ---------------------------------------------------------------------------

const PROVIDER_SETUP_HINT: &str = "\
Cloud providers (requires API key):\n\
 Anthropic (Claude)  export ANTHROPIC_API_KEY=sk-…\n\
 OpenAI              export OPENAI_API_KEY=sk-…\n\
 xAI (Grok)          export XAI_API_KEY=xai-…\n\n\
Free cloud providers:\n\
 OpenRouter           export OPENROUTER_API_KEY=…  (free models available)\n\
 Groq                 export GROQ_API_KEY=…        (generous free tier)\n\n\
Local models (no API key needed):\n\
 Ollama               ollama serve + codineer --model ollama/qwen3-coder\n\
 LM Studio            codineer --model lmstudio/model-name\n\n\
Or authenticate via OAuth:\n\
 codineer login\n\n\
Auto-discovered sources:\n\
 Claude Code          install Claude Code and run `claude login`\n\n\
Check auth status:\n\
 codineer status\n\n\
Switch models:\n\
 codineer --model <name>";

pub fn no_credentials_error() -> String {
    format!("no API credentials found\n\n{PROVIDER_SETUP_HINT}")
}

pub fn provider_hint(model: &str, err: &dyn std::fmt::Display) -> String {
    format!("{err}\n\nCurrent model: {model}\n\n{PROVIDER_SETUP_HINT}")
}
