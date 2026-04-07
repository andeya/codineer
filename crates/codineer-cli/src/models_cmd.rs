use std::collections::BTreeMap;

use api::{list_known_models, provider_kind_by_name, ProviderKind, BUILTIN_PROVIDER_PRESETS};
use runtime::{ConfigLoader, CustomProviderConfig, RuntimeConfig};

use crate::runtime_client::{resolve_custom_api_key, resolve_preset_api_key};

fn load_config() -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    Ok(ConfigLoader::default_for(&cwd).load()?)
}

pub fn run_models(provider: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;

    let filter = provider.map(|name| {
        provider_kind_by_name(name)
            .map(FilterKind::Builtin)
            .unwrap_or(FilterKind::Custom(name.to_string()))
    });

    match filter {
        None => {
            print_known_models(None);
            print_user_aliases(config.model_aliases());
            println!();
            print_dynamic_models_all(&config);
        }
        Some(FilterKind::Builtin(kind)) => {
            print_known_models(Some(kind));
        }
        Some(FilterKind::Custom(name)) => {
            if name.eq_ignore_ascii_case("ollama") {
                print_ollama_models(config.providers());
            } else {
                print_dynamic_models_for(&name, &config);
            }
        }
    }

    Ok(())
}

pub fn run_providers() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;

    println!("Built-in providers:");
    for preset in BUILTIN_PROVIDER_PRESETS {
        let status = if preset.api_key_env.is_empty() {
            "(no API key needed)".to_string()
        } else if config.resolve_env(preset.api_key_env).is_some() {
            format!("(${} ✓)", preset.api_key_env)
        } else {
            format!("(${} not set)", preset.api_key_env)
        };
        println!("  {:<16} {} {status}", preset.name, preset.description);
    }
    println!();

    let providers = config.providers();
    println!("Custom providers (from settings.json):");
    if providers.is_empty() {
        println!("  (none)");
        println!("  Configure in settings.json: {{\"providers\": {{\"name\": {{\"baseUrl\": \"...\", ...}}}}}}");
    } else {
        for (name, cfg) in providers {
            if api::builtin_preset(name).is_some() {
                continue;
            }
            let model_count = cfg.models.len();
            let models_hint = if model_count > 0 {
                format!("{model_count} model(s) configured")
            } else {
                "no models listed".to_string()
            };
            println!("  {:<16} {} ({models_hint})", name, cfg.base_url);
        }
    }
    println!();

    println!("Usage: codineer --model PROVIDER/MODEL_NAME");
    println!("       /models [provider]  to list available models");
    Ok(())
}

// ---------------------------------------------------------------------------

enum FilterKind {
    Builtin(ProviderKind),
    Custom(String),
}

fn print_known_models(filter: Option<ProviderKind>) {
    let entries = list_known_models(filter);
    if entries.is_empty() {
        return;
    }

    let mut grouped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, kind) in &entries {
        grouped.entry(kind.display_name()).or_default().push(name);
    }

    println!("Known models:");
    for (provider_name, models) in &grouped {
        println!("  {provider_name}:");
        for model in models {
            println!("    {model}");
        }
    }
    println!();
}

fn print_user_aliases(aliases: &BTreeMap<String, String>) {
    if aliases.is_empty() {
        println!("Model aliases: (none)");
        println!("  Configure in settings.json: {{\"modelAliases\": {{\"name\": \"model-id\"}}}}");
    } else {
        println!("Model aliases (from settings.json):");
        for (alias, canonical) in aliases {
            println!("  {alias:<16} → {canonical}");
        }
    }
}

// ---------------------------------------------------------------------------
// v1/models dynamic fetching
// ---------------------------------------------------------------------------

struct V1ModelsResult {
    models: Vec<String>,
    error: Option<String>,
}

fn query_v1_models(
    base_url: &str,
    api_key: &str,
    headers: &BTreeMap<String, String>,
) -> V1ModelsResult {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(5000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return V1ModelsResult {
                models: Vec::new(),
                error: Some(format!("HTTP client error: {e}")),
            }
        }
    };
    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let response = match req.send() {
        Ok(r) => r,
        Err(e) => {
            return V1ModelsResult {
                models: Vec::new(),
                error: Some(format!("request failed: {e}")),
            }
        }
    };
    if !response.status().is_success() {
        return V1ModelsResult {
            models: Vec::new(),
            error: Some(format!("HTTP {}", response.status())),
        };
    }
    let body: serde_json::Value = match response.json() {
        Ok(v) => v,
        Err(e) => {
            return V1ModelsResult {
                models: Vec::new(),
                error: Some(format!("invalid JSON: {e}")),
            }
        }
    };
    let models = body
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            let mut models: Vec<String> = arr
                .iter()
                .filter_map(|m| m.get("id")?.as_str().map(String::from))
                .collect();
            models.sort();
            models
        })
        .unwrap_or_default();
    V1ModelsResult {
        models,
        error: None,
    }
}

fn query_provider_models(cfg: &CustomProviderConfig, config: &RuntimeConfig) -> V1ModelsResult {
    let api_key = resolve_custom_api_key(cfg, config).unwrap_or_default();
    query_v1_models(&cfg.base_url, &api_key, &cfg.headers)
}

fn query_preset_models(
    preset: &api::BuiltinProviderPreset,
    config: &RuntimeConfig,
) -> V1ModelsResult {
    match resolve_preset_api_key(preset, config) {
        Ok(key) => query_v1_models(preset.base_url, &key, &BTreeMap::new()),
        Err(_) => V1ModelsResult {
            models: Vec::new(),
            error: Some(format!("${} not set", preset.api_key_env)),
        },
    }
}

fn print_v1_result(name: &str, result: &V1ModelsResult) {
    if result.models.is_empty() {
        if let Some(ref err) = result.error {
            println!("{name}: v1/models failed ({err})");
        } else {
            println!("{name}: no models found via v1/models");
        }
    } else {
        println!("{name} ({} model(s) from v1/models):", result.models.len());
        for model in &result.models {
            println!("  {name}/{model}");
        }
    }
}

fn print_dynamic_models_all(config: &RuntimeConfig) {
    for (name, cfg) in config.providers() {
        if name.eq_ignore_ascii_case("ollama") {
            print_ollama_models(config.providers());
            continue;
        }
        print_v1_result(name, &query_provider_models(cfg, config));
        println!();
    }

    for preset in BUILTIN_PROVIDER_PRESETS {
        if config.providers().contains_key(preset.name) {
            continue;
        }
        let result = query_preset_models(preset, config);
        if !result.models.is_empty() {
            print_v1_result(preset.name, &result);
            println!();
        }
    }
}

fn print_dynamic_models_for(name: &str, config: &RuntimeConfig) {
    if let Some(cfg) = config.providers().get(name) {
        let result = query_provider_models(cfg, config);
        print_v1_result(name, &result);
        if result.models.is_empty() {
            println!("  Base URL: {}", cfg.base_url);
            if !cfg.models.is_empty() {
                println!("  Configured models:");
                for m in &cfg.models {
                    println!("    {name}/{m}");
                }
            }
        }
        return;
    }

    if let Some(preset) = api::builtin_preset(name) {
        print_v1_result(name, &query_preset_models(preset, config));
        return;
    }

    eprintln!("unknown provider: {name}");
    eprintln!("  Run /providers to see available providers.");
}

fn print_ollama_models(providers: &BTreeMap<String, CustomProviderConfig>) {
    let models = crate::runtime_client::query_ollama_tags(providers);
    if models.is_empty() {
        println!("Ollama: not running or no models found");
        println!("  Start with: ollama serve");
        println!("  Pull a model: ollama pull qwen3-coder");
        return;
    }
    println!("Ollama (local models):");
    for model in &models {
        println!("  ollama/{model}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_kind_builtin_from_known_name() {
        let filter = Some("anthropic").map(|name| {
            provider_kind_by_name(name)
                .map(FilterKind::Builtin)
                .unwrap_or(FilterKind::Custom(name.to_string()))
        });
        assert!(matches!(
            filter,
            Some(FilterKind::Builtin(ProviderKind::CodineerApi))
        ));
    }

    #[test]
    fn filter_kind_custom_from_unknown_name() {
        let filter = Some("ollama").map(|name| {
            provider_kind_by_name(name)
                .map(FilterKind::Builtin)
                .unwrap_or(FilterKind::Custom(name.to_string()))
        });
        assert!(matches!(filter, Some(FilterKind::Custom(_))));
        if let Some(FilterKind::Custom(n)) = filter {
            assert_eq!(n, "ollama");
        }
    }

    #[test]
    fn filter_kind_none_when_no_provider() {
        let filter: Option<FilterKind> = None::<&str>.map(|name| {
            provider_kind_by_name(name)
                .map(FilterKind::Builtin)
                .unwrap_or(FilterKind::Custom(name.to_string()))
        });
        assert!(filter.is_none());
    }

    #[test]
    fn known_models_empty_for_custom_filter() {
        let entries = list_known_models(Some(ProviderKind::Custom));
        assert!(entries.is_empty());
    }

    #[test]
    fn known_models_has_entries_for_anthropic() {
        let entries = list_known_models(Some(ProviderKind::CodineerApi));
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|(name, _)| name.contains("sonnet") || name.contains("opus")));
    }

    #[test]
    fn builtin_preset_ollama_exists() {
        assert!(api::builtin_preset("ollama").is_some());
        assert!(api::builtin_preset("groq").is_some());
        assert!(api::builtin_preset("lmstudio").is_some());
        assert!(api::builtin_preset("nonexistent").is_none());
    }

    #[test]
    fn query_v1_models_unreachable() {
        let result = query_v1_models("http://127.0.0.1:1/v1", "", &BTreeMap::new());
        assert!(result.models.is_empty());
        assert!(result.error.is_some());
    }
}
