use std::time::Duration;

pub mod codineer_provider;
pub mod openai_compat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    CodineerApi,
    Xai,
    OpenAi,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderMetadata {
    pub provider: ProviderKind,
    pub auth_env: &'static str,
    pub base_url_env: &'static str,
    pub default_base_url: &'static str,
}

const MODEL_REGISTRY: &[(&str, ProviderMetadata)] = &[
    (
        "opus",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "sonnet",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "haiku",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "claude-opus-4-6",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "claude-sonnet-4-6",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "claude-haiku-4-5-20251213",
        ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        },
    ),
    (
        "grok",
        ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        },
    ),
    (
        "grok-3",
        ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        },
    ),
    (
        "grok-mini",
        ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        },
    ),
    (
        "grok-3-mini",
        ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        },
    ),
    (
        "grok-2",
        ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        },
    ),
    (
        "gpt",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
    (
        "gpt-4o",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
    (
        "mini",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
    (
        "gpt-4o-mini",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
    (
        "o3",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
    (
        "o3-mini",
        ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        },
    ),
];

/// Built-in provider presets for OpenAI-compatible services.
/// Each entry: (name, base_url, api_key_env or empty for local providers).
pub const BUILTIN_PROVIDER_PRESETS: &[BuiltinProviderPreset] = &[
    BuiltinProviderPreset {
        name: "ollama",
        base_url: "http://localhost:11434/v1",
        api_key_env: "",
        description: "Local Ollama instance (no API key needed)",
    },
    BuiltinProviderPreset {
        name: "lmstudio",
        base_url: "http://localhost:1234/v1",
        api_key_env: "",
        description: "Local LM Studio instance (no API key needed)",
    },
    BuiltinProviderPreset {
        name: "openrouter",
        base_url: "https://openrouter.ai/api/v1",
        api_key_env: "OPENROUTER_API_KEY",
        description: "OpenRouter (free models available)",
    },
    BuiltinProviderPreset {
        name: "groq",
        base_url: "https://api.groq.com/openai/v1",
        api_key_env: "GROQ_API_KEY",
        description: "Groq Cloud (generous free tier)",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinProviderPreset {
    pub name: &'static str,
    pub base_url: &'static str,
    pub api_key_env: &'static str,
    pub description: &'static str,
}

/// If model starts with `provider/`, return `(provider_name, model_name)`.
/// Otherwise return `None`.
#[must_use]
pub fn parse_custom_provider_prefix(model: &str) -> Option<(&str, &str)> {
    let trimmed = model.trim();
    let slash_pos = trimmed.find('/')?;
    let provider = &trimmed[..slash_pos];
    let model_part = &trimmed[slash_pos + 1..];
    if provider.is_empty() || model_part.is_empty() {
        return None;
    }
    if MODEL_REGISTRY
        .iter()
        .any(|(alias, _)| *alias == provider.to_ascii_lowercase())
    {
        return None;
    }
    Some((provider, model_part))
}

/// Look up a built-in provider preset by name (case-insensitive).
#[must_use]
pub fn builtin_preset(name: &str) -> Option<&'static BuiltinProviderPreset> {
    let lower = name.to_ascii_lowercase();
    BUILTIN_PROVIDER_PRESETS
        .iter()
        .find(|preset| preset.name == lower)
}

#[must_use]
pub fn resolve_model_alias(model: &str) -> String {
    let trimmed = model.trim();
    if parse_custom_provider_prefix(trimmed).is_some() {
        return trimmed.to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    MODEL_REGISTRY
        .iter()
        .find_map(|(alias, metadata)| {
            (*alias == lower).then_some(match metadata.provider {
                ProviderKind::CodineerApi => match *alias {
                    "opus" => "claude-opus-4-6",
                    "sonnet" => "claude-sonnet-4-6",
                    "haiku" => "claude-haiku-4-5-20251213",
                    _ => trimmed,
                },
                ProviderKind::Xai => match *alias {
                    "grok" | "grok-3" => "grok-3",
                    "grok-mini" | "grok-3-mini" => "grok-3-mini",
                    "grok-2" => "grok-2",
                    _ => trimmed,
                },
                ProviderKind::OpenAi => match *alias {
                    "gpt" | "gpt-4o" => "gpt-4o",
                    "mini" | "gpt-4o-mini" => "gpt-4o-mini",
                    "o3" => "o3",
                    "o3-mini" => "o3-mini",
                    _ => trimmed,
                },
                ProviderKind::Custom => trimmed,
            })
        })
        .map_or_else(|| trimmed.to_string(), ToOwned::to_owned)
}

#[must_use]
pub fn metadata_for_model(model: &str) -> Option<ProviderMetadata> {
    let canonical = resolve_model_alias(model);
    let lower = canonical.to_ascii_lowercase();
    if let Some((_, metadata)) = MODEL_REGISTRY.iter().find(|(alias, _)| *alias == lower) {
        return Some(*metadata);
    }
    if lower.starts_with("grok") {
        return Some(ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_XAI_BASE_URL,
        });
    }
    if lower.starts_with("claude-") || lower == "claude" {
        return Some(ProviderMetadata {
            provider: ProviderKind::CodineerApi,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: codineer_provider::DEFAULT_BASE_URL,
        });
    }
    if lower.starts_with("gpt")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.starts_with("chatgpt-")
    {
        return Some(ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: openai_compat::DEFAULT_OPENAI_BASE_URL,
        });
    }
    None
}

#[must_use]
pub fn detect_provider_kind(model: &str) -> ProviderKind {
    if parse_custom_provider_prefix(model).is_some() {
        return ProviderKind::Custom;
    }
    if let Some(metadata) = metadata_for_model(model) {
        return metadata.provider;
    }
    let fallback = detect_available_provider().unwrap_or(ProviderKind::CodineerApi);
    eprintln!("[warn] unknown model \"{model}\", falling back to {fallback:?} provider");
    fallback
}

fn detect_available_provider() -> Option<ProviderKind> {
    if codineer_provider::has_auth_from_env_or_saved().unwrap_or(false) {
        return Some(ProviderKind::CodineerApi);
    }
    if openai_compat::has_api_key("OPENAI_API_KEY") {
        return Some(ProviderKind::OpenAi);
    }
    if openai_compat::has_api_key("XAI_API_KEY") {
        return Some(ProviderKind::Xai);
    }
    None
}

/// Detect which provider has available credentials and return its default model.
/// Returns `None` if no credentials are found for any provider.
#[must_use]
pub fn auto_detect_default_model() -> Option<&'static str> {
    match detect_available_provider()? {
        ProviderKind::CodineerApi => Some("claude-sonnet-4-6"),
        ProviderKind::Xai => Some("grok-3"),
        ProviderKind::OpenAi => Some("gpt-4o"),
        ProviderKind::Custom => None,
    }
}

#[must_use]
pub fn max_tokens_for_model(model: &str) -> u32 {
    let canonical = resolve_model_alias(model);
    if canonical.starts_with("claude-opus") || canonical == "opus" {
        32_000
    } else if parse_custom_provider_prefix(&canonical).is_some() {
        // Local / custom models often have smaller context windows;
        // 16k is a safe default that avoids hitting limits on 8B–32B models.
        16_000
    } else {
        64_000
    }
}

/// A model alias entry for listing.
#[derive(Debug, Clone)]
pub struct ModelAliasEntry {
    pub alias: &'static str,
    pub canonical: String,
    pub provider: ProviderKind,
}

/// Return all built-in model aliases, optionally filtered by provider kind.
#[must_use]
pub fn list_builtin_models(filter_provider: Option<ProviderKind>) -> Vec<ModelAliasEntry> {
    MODEL_REGISTRY
        .iter()
        .filter(|(_, meta)| filter_provider.is_none_or(|p| meta.provider == p))
        .map(|(alias, meta)| ModelAliasEntry {
            alias,
            canonical: resolve_model_alias(alias),
            provider: meta.provider,
        })
        .collect()
}

/// Resolve a provider name to `ProviderKind` from known aliases.
#[must_use]
pub fn provider_kind_by_name(name: &str) -> Option<ProviderKind> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "anthropic" | "claude" => Some(ProviderKind::CodineerApi),
        "xai" | "grok" => Some(ProviderKind::Xai),
        "openai" | "gpt" => Some(ProviderKind::OpenAi),
        _ => None,
    }
}

impl ProviderKind {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::CodineerApi => "Anthropic",
            Self::Xai => "xAI",
            Self::OpenAi => "OpenAI",
            Self::Custom => "Custom",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_preset, detect_provider_kind, list_builtin_models, max_tokens_for_model,
        parse_custom_provider_prefix, provider_kind_by_name, resolve_model_alias, ProviderKind,
    };

    #[test]
    fn resolves_grok_aliases() {
        assert_eq!(resolve_model_alias("grok"), "grok-3");
        assert_eq!(resolve_model_alias("grok-mini"), "grok-3-mini");
        assert_eq!(resolve_model_alias("grok-2"), "grok-2");
    }

    #[test]
    fn detects_provider_from_model_name_first() {
        assert_eq!(detect_provider_kind("grok"), ProviderKind::Xai);
        assert_eq!(
            detect_provider_kind("claude-sonnet-4-6"),
            ProviderKind::CodineerApi
        );
    }

    #[test]
    fn detects_provider_by_unlisted_model_id_prefix() {
        assert_eq!(
            detect_provider_kind("claude-3-5-sonnet-20241022"),
            ProviderKind::CodineerApi
        );
        assert_eq!(detect_provider_kind("gpt-4-turbo"), ProviderKind::OpenAi);
        assert_eq!(detect_provider_kind("o1-preview"), ProviderKind::OpenAi);
        assert_eq!(detect_provider_kind("o3-pro"), ProviderKind::OpenAi);
    }

    #[test]
    fn keeps_existing_max_token_heuristic() {
        assert_eq!(max_tokens_for_model("opus"), 32_000);
        assert_eq!(max_tokens_for_model("grok-3"), 64_000);
    }

    #[test]
    fn parses_custom_provider_prefix() {
        assert_eq!(
            parse_custom_provider_prefix("ollama/qwen2.5-coder"),
            Some(("ollama", "qwen2.5-coder"))
        );
        assert_eq!(
            parse_custom_provider_prefix("groq/llama-3.3-70b"),
            Some(("groq", "llama-3.3-70b"))
        );
        assert_eq!(
            parse_custom_provider_prefix("openrouter/meta-llama/llama-3.1-8b:free"),
            Some(("openrouter", "meta-llama/llama-3.1-8b:free"))
        );
        assert_eq!(parse_custom_provider_prefix("grok-3"), None);
        assert_eq!(parse_custom_provider_prefix("sonnet"), None);
        assert_eq!(parse_custom_provider_prefix("/model"), None);
        assert_eq!(parse_custom_provider_prefix("provider/"), None);
    }

    #[test]
    fn detects_custom_provider_kind() {
        assert_eq!(
            detect_provider_kind("ollama/qwen2.5-coder"),
            ProviderKind::Custom
        );
        assert_eq!(
            detect_provider_kind("lmstudio/my-model"),
            ProviderKind::Custom
        );
    }

    #[test]
    fn resolves_custom_model_passthrough() {
        assert_eq!(
            resolve_model_alias("ollama/qwen2.5-coder"),
            "ollama/qwen2.5-coder"
        );
    }

    #[test]
    fn custom_model_tokens_smaller_default() {
        assert_eq!(max_tokens_for_model("ollama/qwen2.5-coder"), 16_000);
    }

    #[test]
    fn builtin_presets_lookup() {
        let ollama = builtin_preset("ollama").expect("ollama preset should exist");
        assert_eq!(ollama.base_url, "http://localhost:11434/v1");
        assert!(ollama.api_key_env.is_empty());

        let groq = builtin_preset("groq").expect("groq preset should exist");
        assert_eq!(groq.api_key_env, "GROQ_API_KEY");

        assert!(builtin_preset("nonexistent").is_none());
    }

    #[test]
    fn list_builtin_models_returns_all_when_unfiltered() {
        let all = list_builtin_models(None);
        assert!(!all.is_empty());
        assert!(all.iter().any(|e| e.provider == ProviderKind::CodineerApi));
        assert!(all.iter().any(|e| e.provider == ProviderKind::Xai));
    }

    #[test]
    fn list_builtin_models_filters_by_provider() {
        let xai = list_builtin_models(Some(ProviderKind::Xai));
        assert!(!xai.is_empty());
        assert!(xai.iter().all(|e| e.provider == ProviderKind::Xai));

        let anthropic = list_builtin_models(Some(ProviderKind::CodineerApi));
        assert!(!anthropic.is_empty());
        assert!(anthropic
            .iter()
            .all(|e| e.provider == ProviderKind::CodineerApi));
    }

    #[test]
    fn list_builtin_models_custom_filter_returns_empty() {
        let custom = list_builtin_models(Some(ProviderKind::Custom));
        assert!(custom.is_empty());
    }

    #[test]
    fn provider_kind_by_name_resolves_known() {
        assert_eq!(
            provider_kind_by_name("anthropic"),
            Some(ProviderKind::CodineerApi)
        );
        assert_eq!(
            provider_kind_by_name("claude"),
            Some(ProviderKind::CodineerApi)
        );
        assert_eq!(provider_kind_by_name("xai"), Some(ProviderKind::Xai));
        assert_eq!(provider_kind_by_name("grok"), Some(ProviderKind::Xai));
        assert_eq!(provider_kind_by_name("openai"), Some(ProviderKind::OpenAi));
        assert_eq!(provider_kind_by_name("gpt"), Some(ProviderKind::OpenAi));
    }

    #[test]
    fn provider_kind_by_name_case_insensitive() {
        assert_eq!(
            provider_kind_by_name("Anthropic"),
            Some(ProviderKind::CodineerApi)
        );
        assert_eq!(provider_kind_by_name("XAI"), Some(ProviderKind::Xai));
    }

    #[test]
    fn provider_kind_by_name_returns_none_for_unknown() {
        assert_eq!(provider_kind_by_name("ollama"), None);
        assert_eq!(provider_kind_by_name("unknown"), None);
        assert_eq!(provider_kind_by_name(""), None);
    }

    #[test]
    fn provider_kind_display_name_covers_all_variants() {
        assert_eq!(ProviderKind::CodineerApi.display_name(), "Anthropic");
        assert_eq!(ProviderKind::Xai.display_name(), "xAI");
        assert_eq!(ProviderKind::OpenAi.display_name(), "OpenAI");
        assert_eq!(ProviderKind::Custom.display_name(), "Custom");
    }
}
