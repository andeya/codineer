use std::collections::BTreeMap;

use api::{OpenAiCompatClient, ProviderClient};
use runtime::CustomProviderConfig;

use crate::auth::{build_credential_chain, no_credentials_error, provider_hint};

#[derive(Debug)]
pub(crate) struct ResolvedModel {
    pub model: String,
    pub client: ProviderClient,
}

/// Single-responsibility resolver: model string → (canonical model, provider client).
///
/// Pipeline:  input → expand_shorthand → resolve_alias → build_client
pub(crate) struct ModelResolver<'a> {
    providers: &'a BTreeMap<String, CustomProviderConfig>,
    config: &'a runtime::RuntimeConfig,
}

impl<'a> ModelResolver<'a> {
    pub fn new(config: &'a runtime::RuntimeConfig) -> Self {
        Self {
            providers: config.providers(),
            config,
        }
    }

    pub fn resolve(&self, input: &str) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let expanded = self.expand_shorthand(input)?;
        let canonical = api::resolve_model_alias(&expanded);
        match self.build_client(&canonical) {
            Ok(resolved) => Ok(resolved),
            Err(primary_err) => self.try_fallback(&canonical, primary_err),
        }
    }

    pub(super) fn try_fallback(
        &self,
        primary_model: &str,
        primary_err: Box<dyn std::error::Error>,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let fallbacks = self.config.fallback_models();
        if fallbacks.is_empty() {
            return Err(primary_err);
        }
        for fallback in fallbacks {
            let expanded = match self.expand_shorthand(fallback) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let canonical = api::resolve_model_alias(&expanded);
            if let Ok(resolved) = self.build_client(&canonical) {
                eprintln!("[info] {primary_model} unavailable, falling back to {canonical}");
                return Ok(resolved);
            }
        }
        Err(primary_err)
    }

    pub(super) fn expand_shorthand(
        &self,
        input: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match input {
            "auto" => self.auto_detect_model(),
            "ollama" => detect_ollama_model(self.providers)
                .ok_or_else(|| "Ollama is not running. Start it with: ollama serve".into()),
            bare if api::builtin_preset(bare).is_some()
                && api::parse_custom_provider_prefix(bare).is_none() =>
            {
                self.expand_bare_provider(bare)
            }
            other => Ok(other.to_string()),
        }
    }

    fn auto_detect_model(&self) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(builtin) = api::auto_detect_default_model() {
            return Ok(builtin.to_string());
        }
        if let Some(ollama) = detect_ollama_model(self.providers) {
            return Ok(ollama);
        }
        Err(no_credentials_error().into())
    }

    fn expand_bare_provider(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lower = name.to_ascii_lowercase();
        if let Some(config) = self.providers.get(&lower) {
            if let Some(default) = &config.default_model {
                return Ok(format!("{name}/{default}"));
            }
        }
        Err(format!(
            "provider '{name}' requires a model name.\n\
             Use: codineer --model {name}/<model-name>"
        )
        .into())
    }

    fn build_client(&self, model: &str) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        if let Some((provider_name, _)) = api::parse_custom_provider_prefix(model) {
            return self.build_custom_client(model, provider_name);
        }
        self.build_builtin_client(model)
    }

    fn build_custom_client(
        &self,
        model: &str,
        provider_name: &str,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let lower = provider_name.to_ascii_lowercase();

        let client = if let Some(config) = self.providers.get(&lower) {
            let api_key = resolve_custom_api_key(config)?;
            let mut c = OpenAiCompatClient::new_custom(&config.base_url, api_key);
            if let Some(ref v) = config.api_version {
                let q = format!("api-version={v}");
                c = c.with_endpoint_query(Some(q));
            }
            if !config.headers.is_empty() {
                c = c.with_custom_headers(config.headers.clone());
            }
            c
        } else if let Some(preset) = api::builtin_preset(&lower) {
            let api_key = resolve_preset_api_key(preset)?;
            OpenAiCompatClient::new_custom(preset.base_url, api_key)
        } else {
            return Err(format!(
                "unknown provider '{provider_name}'\n\n\
                 Built-in providers: ollama, lmstudio, openrouter, groq\n\
                 Or configure in settings.json: \
                 {{\"providers\": {{\"{provider_name}\": {{\"baseUrl\": \"...\"}}}}}}"
            )
            .into());
        };

        Ok(ResolvedModel {
            model: model.to_string(),
            client: ProviderClient::from_custom(client),
        })
    }

    fn build_builtin_client(
        &self,
        model: &str,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let kind = api::detect_provider_kind(model);
        let chain = build_credential_chain(kind, self.config);
        let credential = chain.resolve().map_err(|err| provider_hint(model, &err))?;
        let client = ProviderClient::from_model_with_credential(model, credential)
            .map_err(|err| provider_hint(model, &err))?;
        Ok(ResolvedModel {
            model: model.to_string(),
            client,
        })
    }
}

pub(crate) fn resolve_custom_api_key(
    config: &CustomProviderConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(key) = &config.api_key {
        return Ok(key.clone());
    }
    if let Some(env_name) = &config.api_key_env {
        let key = std::env::var(env_name).unwrap_or_default();
        if key.is_empty() {
            return Err(
                format!("provider config references env var {env_name} but it is not set").into(),
            );
        }
        return Ok(key);
    }
    Ok(String::new())
}

pub(crate) fn resolve_preset_api_key(
    preset: &api::BuiltinProviderPreset,
) -> Result<String, Box<dyn std::error::Error>> {
    if preset.api_key_env.is_empty() {
        return Ok(String::new());
    }
    let key = std::env::var(preset.api_key_env).unwrap_or_default();
    if key.is_empty() {
        return Err(format!(
            "provider '{}' requires {} to be set",
            preset.name, preset.api_key_env
        )
        .into());
    }
    Ok(key)
}

/// Probe local Ollama and pick the best coding model.
fn detect_ollama_model(providers: &BTreeMap<String, CustomProviderConfig>) -> Option<String> {
    let names = query_ollama_tags(providers);
    if names.is_empty() {
        return None;
    }
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let best = pick_best_coding_model(&refs);
    Some(format!("ollama/{best}"))
}

pub(crate) fn query_ollama_tags(providers: &BTreeMap<String, CustomProviderConfig>) -> Vec<String> {
    let base = resolve_ollama_base_url(providers);
    let tags_url = format!("{}/api/tags", base.trim_end_matches('/'));
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let response = match client.get(&tags_url).send() {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };
    let body: serde_json::Value = match response.json() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name")?.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn resolve_ollama_base_url(
    providers: &BTreeMap<String, CustomProviderConfig>,
) -> String {
    if let Some(config) = providers.get("ollama") {
        return config.base_url.trim_end_matches("/v1").to_string();
    }
    if let Ok(host) = std::env::var("OLLAMA_HOST") {
        let host = host.trim().trim_end_matches('/');
        if !host.is_empty() {
            if host.starts_with("http://") || host.starts_with("https://") {
                return host.to_string();
            }
            return format!("http://{host}");
        }
    }
    "http://localhost:11434".to_string()
}

pub(super) fn pick_best_coding_model<'a>(names: &[&'a str]) -> &'a str {
    const PREFERRED: &[&str] = &[
        "qwen3-coder",
        "qwen2.5-coder",
        "qwen3",
        "deepseek-coder-v2",
        "deepseek-coder",
        "codellama",
        "starcoder2",
        "codegemma",
    ];
    for preferred in PREFERRED {
        if let Some(found) = names.iter().find(|n| n.contains(preferred)) {
            return found;
        }
    }
    names.first().copied().unwrap_or("unknown")
}
