use super::model::{pick_best_coding_model, resolve_custom_api_key, resolve_preset_api_key};
use super::*;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use runtime::CustomProviderConfig;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn empty_providers() -> BTreeMap<String, CustomProviderConfig> {
    BTreeMap::new()
}

fn make_provider(base_url: &str, default_model: Option<&str>) -> CustomProviderConfig {
    CustomProviderConfig {
        base_url: base_url.to_string(),
        api_version: None,
        api_key: None,
        api_key_env: None,
        models: vec![],
        default_model: default_model.map(|s| s.to_string()),
        headers: BTreeMap::new(),
    }
}

fn config_with_providers(
    providers: BTreeMap<String, CustomProviderConfig>,
) -> runtime::RuntimeConfig {
    let mut feature = runtime::RuntimeFeatureConfig::default();
    feature.set_providers(providers);
    runtime::RuntimeConfig::new(BTreeMap::new(), Vec::new(), feature)
}

// -----------------------------------------------------------------------
// pick_best_coding_model
// -----------------------------------------------------------------------

#[test]
fn pick_best_coding_model_prefers_qwen3_coder() {
    let names = vec!["llama3:8b", "qwen3-coder:30b", "deepseek-coder:6.7b"];
    assert_eq!(pick_best_coding_model(&names), "qwen3-coder:30b");
}

#[test]
fn pick_best_coding_model_falls_back_to_deepseek() {
    let names = vec!["llama3:8b", "deepseek-coder-v2:16b", "mistral:7b"];
    assert_eq!(pick_best_coding_model(&names), "deepseek-coder-v2:16b");
}

#[test]
fn pick_best_coding_model_falls_back_to_first_when_no_match() {
    let names = vec!["llama3:8b", "mistral:7b", "phi3:14b"];
    assert_eq!(pick_best_coding_model(&names), "llama3:8b");
}

#[test]
fn pick_best_coding_model_respects_priority_order() {
    let names = vec!["codellama:13b", "qwen2.5-coder:7b", "starcoder2:3b"];
    assert_eq!(pick_best_coding_model(&names), "qwen2.5-coder:7b");
}

#[test]
fn pick_best_coding_model_prefers_qwen3_over_qwen25() {
    let names = vec!["qwen2.5-coder:7b", "qwen3-coder:30b", "codellama:13b"];
    assert_eq!(pick_best_coding_model(&names), "qwen3-coder:30b");
}

#[test]
fn pick_best_coding_model_selects_qwen3_over_codellama() {
    let names = vec!["codellama:13b", "qwen3:8b"];
    assert_eq!(pick_best_coding_model(&names), "qwen3:8b");
}

// -----------------------------------------------------------------------
// resolve_custom_api_key
// -----------------------------------------------------------------------

#[test]
fn resolve_custom_api_key_returns_inline_key() {
    let config = CustomProviderConfig {
        base_url: "http://localhost".to_string(),
        api_version: None,
        api_key: Some("sk-test-123".to_string()),
        api_key_env: Some("SHOULD_NOT_USE".to_string()),
        models: vec![],
        default_model: None,
        headers: BTreeMap::new(),
    };
    assert_eq!(resolve_custom_api_key(&config).unwrap(), "sk-test-123");
}

#[test]
fn resolve_custom_api_key_returns_empty_when_no_key_fields() {
    let config = make_provider("http://localhost", None);
    assert_eq!(resolve_custom_api_key(&config).unwrap(), "");
}

#[test]
fn resolve_custom_api_key_errors_on_missing_env_var() {
    let config = CustomProviderConfig {
        base_url: "http://localhost".to_string(),
        api_version: None,
        api_key: None,
        api_key_env: Some("__CODINEER_TEST_NONEXISTENT_KEY__".to_string()),
        models: vec![],
        default_model: None,
        headers: BTreeMap::new(),
    };
    let err = resolve_custom_api_key(&config).unwrap_err();
    assert!(err
        .to_string()
        .contains("__CODINEER_TEST_NONEXISTENT_KEY__"));
}

// -----------------------------------------------------------------------
// resolve_preset_api_key
// -----------------------------------------------------------------------

#[test]
fn resolve_preset_api_key_returns_empty_for_local_provider() {
    let preset = api::builtin_preset("ollama").unwrap();
    assert_eq!(resolve_preset_api_key(preset).unwrap(), "");
}

#[test]
fn resolve_preset_api_key_errors_when_env_missing() {
    let preset = api::builtin_preset("groq").unwrap();
    let err = resolve_preset_api_key(preset).unwrap_err();
    assert!(err.to_string().contains("GROQ_API_KEY"));
}

// -----------------------------------------------------------------------
// ModelResolver::expand_shorthand (via resolve)
// -----------------------------------------------------------------------

#[test]
fn resolver_resolves_user_alias_before_building_client() {
    let mut feature = runtime::RuntimeFeatureConfig::default();
    feature.set_providers(empty_providers());
    let mut aliases = BTreeMap::new();
    aliases.insert("sonnet".into(), "claude-sonnet-4-6".into());
    feature.set_model_aliases(aliases);
    let config = runtime::RuntimeConfig::new(BTreeMap::new(), Vec::new(), feature);
    let resolver = ModelResolver::new(&config);
    let err = resolver.resolve("sonnet").unwrap_err();
    assert!(
        err.to_string().contains("claude-sonnet-4-6"),
        "error should reference canonical model: {err}"
    );
}

#[test]
fn resolver_passes_through_unknown_name() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let err = resolver.resolve("sonnet").unwrap_err();
    assert!(
        err.to_string().contains("sonnet"),
        "error should reference raw model name when no alias: {err}"
    );
}

#[test]
fn resolver_passes_through_custom_prefixed_model() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        make_provider("http://localhost:11434/v1", None),
    );
    let config = config_with_providers(providers);
    let resolver = ModelResolver::new(&config);
    let result = resolver.resolve("ollama/qwen3-coder:30b").unwrap();
    assert_eq!(result.model, "ollama/qwen3-coder:30b");
}

#[test]
fn resolver_expands_bare_provider_with_default_model() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "groq".to_string(),
        make_provider(
            "https://api.groq.com/openai/v1",
            Some("llama-3.3-70b-versatile"),
        ),
    );
    let config = config_with_providers(providers);
    let resolver = ModelResolver::new(&config);
    let result = resolver.resolve("groq").unwrap();
    assert_eq!(result.model, "groq/llama-3.3-70b-versatile");
}

#[test]
fn resolver_errors_on_bare_provider_without_default() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let err = resolver.resolve("groq").unwrap_err();
    assert!(err.to_string().contains("requires a model name"));
}

#[test]
fn resolver_errors_on_unknown_provider_prefix() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let err = resolver.resolve("unknown-provider/some-model").unwrap_err();
    assert!(err.to_string().contains("unknown provider"));
}

#[test]
fn resolver_ollama_shorthand_errors_when_not_running() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let err = resolver.resolve("ollama").unwrap_err();
    assert!(err.to_string().contains("Ollama is not running"));
}

#[test]
fn resolver_uses_config_over_builtin_preset() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        CustomProviderConfig {
            base_url: "http://custom-ollama:11434/v1".to_string(),
            api_version: None,
            api_key: Some("custom-key".to_string()),
            api_key_env: None,
            models: vec![],
            default_model: None,
            headers: BTreeMap::new(),
        },
    );
    let config = config_with_providers(providers);
    let resolver = ModelResolver::new(&config);
    let result = resolver.resolve("ollama/llama3:8b").unwrap();
    assert_eq!(result.model, "ollama/llama3:8b");
}

// -----------------------------------------------------------------------
// is_tool_use_error
// -----------------------------------------------------------------------

#[test]
fn is_tool_use_error_detects_tool_keywords() {
    assert!(DefaultRuntimeClient::is_tool_use_error(
        "tool_use is not supported"
    ));
    assert!(DefaultRuntimeClient::is_tool_use_error(
        "Function calling unavailable"
    ));
    assert!(DefaultRuntimeClient::is_tool_use_error(
        "unsupported parameter: tools"
    ));
    assert!(DefaultRuntimeClient::is_tool_use_error(
        "model does not support this feature"
    ));
}

#[test]
fn is_tool_use_error_rejects_unrelated_errors() {
    assert!(!DefaultRuntimeClient::is_tool_use_error(
        "rate limit exceeded"
    ));
    assert!(!DefaultRuntimeClient::is_tool_use_error("invalid API key"));
    assert!(!DefaultRuntimeClient::is_tool_use_error(
        "connection refused"
    ));
}

// -----------------------------------------------------------------------
// ModelResolver::build_custom_client with preset fallback
// -----------------------------------------------------------------------

#[test]
fn resolver_uses_builtin_preset_for_lmstudio() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let result = resolver.resolve("lmstudio/my-model").unwrap();
    assert_eq!(result.model, "lmstudio/my-model");
}

// -----------------------------------------------------------------------
// resolve_ollama_base_url
// -----------------------------------------------------------------------

#[test]
fn ollama_base_url_defaults_to_localhost() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::remove_var("OLLAMA_HOST");
    let providers = empty_providers();
    assert_eq!(
        resolve_ollama_base_url(&providers),
        "http://localhost:11434"
    );
}

#[test]
fn ollama_base_url_from_config_takes_priority() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var("OLLAMA_HOST", "http://env-host:9999");
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        make_provider("http://config-host:11434/v1", None),
    );
    let url = resolve_ollama_base_url(&providers);
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(url, "http://config-host:11434");
}

#[test]
fn ollama_base_url_from_env_var() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var("OLLAMA_HOST", "http://remote-host:11434");
    let providers = empty_providers();
    let url = resolve_ollama_base_url(&providers);
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(url, "http://remote-host:11434");
}

#[test]
fn ollama_base_url_from_env_var_bare_host_port() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var("OLLAMA_HOST", "192.168.1.100:11434");
    let providers = empty_providers();
    let url = resolve_ollama_base_url(&providers);
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(url, "http://192.168.1.100:11434");
}

#[test]
fn ollama_base_url_from_env_var_strips_trailing_slash() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var("OLLAMA_HOST", "http://my-server:11434/");
    let providers = empty_providers();
    let url = resolve_ollama_base_url(&providers);
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(url, "http://my-server:11434");
}

// -----------------------------------------------------------------------
// try_fallback
// -----------------------------------------------------------------------

fn config_with_fallback(
    providers: BTreeMap<String, CustomProviderConfig>,
    fallback_models: Vec<String>,
) -> runtime::RuntimeConfig {
    let mut feature = runtime::RuntimeFeatureConfig::default();
    feature.set_providers(providers);
    feature.set_fallback_models(fallback_models);
    runtime::RuntimeConfig::new(BTreeMap::new(), Vec::new(), feature)
}

#[test]
fn try_fallback_returns_primary_error_when_no_fallbacks() {
    let config = config_with_fallback(empty_providers(), vec![]);
    let resolver = ModelResolver::new(&config);
    let err: Box<dyn std::error::Error> = "primary failure".into();
    let result = resolver.try_fallback("sonnet", err);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("primary failure"));
}

#[test]
fn try_fallback_skips_unavailable_and_returns_primary_error() {
    let config = config_with_fallback(
        empty_providers(),
        vec!["unknown-provider/model".to_string()],
    );
    let resolver = ModelResolver::new(&config);
    let err: Box<dyn std::error::Error> = "primary failure".into();
    let result = resolver.try_fallback("sonnet", err);
    assert!(result.is_err());
}

#[test]
fn try_fallback_succeeds_with_available_provider() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        make_provider("http://localhost:11434/v1", None),
    );
    let config = config_with_fallback(providers, vec!["ollama/qwen3-coder:30b".to_string()]);
    let resolver = ModelResolver::new(&config);
    let err: Box<dyn std::error::Error> = "primary failure".into();
    let result = resolver.try_fallback("sonnet", err);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().model, "ollama/qwen3-coder:30b");
}

#[test]
fn try_fallback_tries_multiple_entries() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        make_provider("http://localhost:11434/v1", None),
    );
    let config = config_with_fallback(
        providers,
        vec!["unknown/model".to_string(), "ollama/llama3:8b".to_string()],
    );
    let resolver = ModelResolver::new(&config);
    let err: Box<dyn std::error::Error> = "primary failure".into();
    let result = resolver.try_fallback("sonnet", err);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().model, "ollama/llama3:8b");
}

// -----------------------------------------------------------------------
// expand_shorthand
// -----------------------------------------------------------------------

#[test]
fn expand_shorthand_passes_through_model_name() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let expanded = resolver.expand_shorthand("claude-sonnet-4-6").unwrap();
    assert_eq!(expanded, "claude-sonnet-4-6");
}

#[test]
fn expand_shorthand_ollama_fails_gracefully() {
    let config = config_with_providers(empty_providers());
    let resolver = ModelResolver::new(&config);
    let err = resolver.expand_shorthand("ollama").unwrap_err();
    assert!(err.to_string().contains("Ollama is not running"));
}

// -----------------------------------------------------------------------
// query_ollama_tags with unreachable server
// -----------------------------------------------------------------------

#[test]
fn query_ollama_tags_returns_empty_on_unreachable() {
    let mut providers = BTreeMap::new();
    providers.insert(
        "ollama".to_string(),
        make_provider("http://127.0.0.1:1/v1", None),
    );
    let result = query_ollama_tags(&providers);
    assert!(result.is_empty());
}
