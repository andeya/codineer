use std::collections::BTreeMap;

use api::{list_builtin_models, provider_kind_by_name, ProviderKind, BUILTIN_PROVIDER_PRESETS};
use runtime::{ConfigLoader, CustomProviderConfig};

pub fn run_models(provider: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let config = ConfigLoader::default_for(&cwd).load()?;
    let providers_config = config.providers().clone();

    let filter = provider.map(|name| {
        provider_kind_by_name(name)
            .map(FilterKind::Builtin)
            .unwrap_or(FilterKind::Custom(name.to_string()))
    });

    match filter {
        None => {
            print_builtin_models(None);
            println!();
            print_custom_providers(&providers_config);
            print_ollama_models(&providers_config);
        }
        Some(FilterKind::Builtin(kind)) => {
            print_builtin_models(Some(kind));
        }
        Some(FilterKind::Custom(name)) => {
            if name.eq_ignore_ascii_case("ollama") {
                print_ollama_models(&providers_config);
            } else if let Some(preset) = api::builtin_preset(&name) {
                println!("{} ({})", preset.name, preset.description);
                println!("  Usage: codineer --model {}/MODEL_NAME", preset.name);
                if !preset.api_key_env.is_empty() {
                    println!("  API key: ${}", preset.api_key_env);
                }
            } else if providers_config.contains_key(&name) {
                println!("Custom provider: {name}");
                println!("  Usage: codineer --model {name}/MODEL_NAME");
            } else {
                return Err(format!("unknown provider: {name}").into());
            }
        }
    }

    Ok(())
}

enum FilterKind {
    Builtin(ProviderKind),
    Custom(String),
}

fn print_builtin_models(filter: Option<ProviderKind>) {
    let entries = list_builtin_models(filter);
    if entries.is_empty() {
        return;
    }

    let mut grouped: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for entry in &entries {
        grouped
            .entry(entry.provider.display_name())
            .or_default()
            .push((entry.alias, &entry.canonical));
    }

    for (provider_name, models) in &grouped {
        println!("{provider_name}:");
        let mut seen_canonical = std::collections::HashSet::new();
        for (alias, canonical) in models.iter() {
            if *alias == *canonical {
                continue;
            }
            println!("  {alias:<16} → {canonical}");
            seen_canonical.insert(*canonical);
        }
        for (alias, canonical) in models.iter() {
            if *alias == *canonical && !seen_canonical.contains(canonical) {
                println!("  {alias}");
            }
        }
        println!();
    }
}

fn print_custom_providers(providers: &BTreeMap<String, CustomProviderConfig>) {
    println!("Custom providers (OpenAI-compatible):");
    for preset in BUILTIN_PROVIDER_PRESETS {
        if preset.api_key_env.is_empty() {
            println!("  {:<16} {}", preset.name, preset.description);
        } else {
            println!(
                "  {:<16} {} (${} required)",
                preset.name, preset.description, preset.api_key_env
            );
        }
    }
    for name in providers.keys() {
        if api::builtin_preset(name).is_none() {
            println!("  {name:<16} (custom)");
        }
    }
    println!("  Usage: codineer --model PROVIDER/MODEL_NAME");
    println!();
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
    fn print_builtin_models_empty_for_custom_filter() {
        let entries = list_builtin_models(Some(ProviderKind::Custom));
        assert!(entries.is_empty());
    }

    #[test]
    fn print_builtin_models_has_entries_for_anthropic() {
        let entries = list_builtin_models(Some(ProviderKind::CodineerApi));
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|e| e.alias.contains("sonnet") || e.alias.contains("opus")));
    }

    #[test]
    fn builtin_preset_ollama_exists() {
        assert!(api::builtin_preset("ollama").is_some());
        assert!(api::builtin_preset("groq").is_some());
        assert!(api::builtin_preset("lmstudio").is_some());
        assert!(api::builtin_preset("nonexistent").is_none());
    }
}
