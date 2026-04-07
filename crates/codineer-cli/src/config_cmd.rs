use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use runtime::JsonValue;

use crate::error::CliResult;

fn global_settings_path() -> PathBuf {
    // Respect CODINEER_CONFIG_HOME just like the runtime config loader does.
    runtime::default_config_home().join("settings.json")
}

fn load_global_settings() -> CliResult<BTreeMap<String, JsonValue>> {
    let path = global_settings_path();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let contents = fs::read_to_string(&path)?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)?;
    Ok(serde_value_to_json_value_map(
        parsed.as_object().cloned().unwrap_or_default(),
    ))
}

fn save_global_settings(settings: &BTreeMap<String, JsonValue>) -> CliResult<()> {
    let path = global_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serde_map = json_value_map_to_serde(settings);
    let json = serde_json::to_string_pretty(&serde_json::Value::Object(serde_map))?;
    fs::write(&path, json + "\n")?;
    Ok(())
}

/// Set a value in the global settings file (`$CODINEER_CONFIG_HOME/settings.json`,
/// defaulting to `~/.codineer/settings.json`).
///
/// Supports dotted keys: `credentials.defaultSource` → `{ "credentials": { "defaultSource": ... } }`
pub fn run_config_set(key: &str, value: &str) -> CliResult<()> {
    let mut settings = load_global_settings()?;
    let parsed_value = parse_value(value);

    let parts: Vec<&str> = key.split('.').collect();
    set_nested(&mut settings, &parts, parsed_value);

    save_global_settings(&settings)?;
    println!("Set {key} = {value}");
    println!("  Saved to {}", global_settings_path().display());
    Ok(())
}

fn load_merged_config() -> CliResult<runtime::RuntimeConfig> {
    let cwd = std::env::current_dir()?;
    Ok(runtime::ConfigLoader::default_for(&cwd).load()?)
}

fn print_scalar(value: &JsonValue) {
    match value {
        JsonValue::String(s) => println!("{s}"),
        other => println!("{}", other.render()),
    }
}

/// Get a config value. If no key, show the merged config.
pub fn run_config_get(key: Option<&str>) -> CliResult<()> {
    let config = load_merged_config()?;

    match key {
        Some(k) => match get_nested(config.merged(), k) {
            Some(value) => print_scalar(&value),
            None => println!("(not set)"),
        },
        None => print_flat("", config.merged()),
    }
    Ok(())
}

/// List all settings.
pub fn run_config_list() -> CliResult<()> {
    let config = load_merged_config()?;

    println!("Configuration (merged from all sources):");
    println!();

    let entries = config.loaded_entries();
    if entries.is_empty() {
        println!("  (no configuration files found)");
    } else {
        println!("  Loaded files:");
        for entry in entries {
            println!("    [{}] {}", entry.source, entry.path.display());
        }
    }
    println!();

    let merged = config.merged();
    if merged.is_empty() {
        println!("  (empty configuration)");
    } else {
        print_flat("", merged);
    }

    Ok(())
}

fn parse_value(s: &str) -> JsonValue {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        return serde_value_to_json_value(v);
    }
    JsonValue::String(s.to_string())
}

fn set_nested(map: &mut BTreeMap<String, JsonValue>, parts: &[&str], value: JsonValue) {
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }
    let child = map
        .entry(parts[0].to_string())
        .or_insert_with(|| JsonValue::Object(BTreeMap::new()));
    if let JsonValue::Object(obj) = child {
        set_nested(obj, &parts[1..], value);
    } else {
        let mut new_obj = BTreeMap::new();
        set_nested(&mut new_obj, &parts[1..], value);
        *child = JsonValue::Object(new_obj);
    }
}

fn get_nested(map: &BTreeMap<String, JsonValue>, dotted_key: &str) -> Option<JsonValue> {
    let mut parts = dotted_key.split('.');
    let first = parts.next()?;
    let mut current = map.get(first)?.clone();
    for part in parts {
        if let JsonValue::Object(obj) = current {
            current = obj.get(part)?.clone();
        } else {
            return None;
        }
    }
    Some(current)
}

fn print_flat(prefix: &str, map: &BTreeMap<String, JsonValue>) {
    for (key, value) in map {
        let full_key = if prefix.is_empty() {
            key.as_str().to_string()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            JsonValue::Object(obj) => print_flat(&full_key, obj),
            JsonValue::String(s) => println!("  {full_key} = {s}"),
            other => println!("  {full_key} = {}", other.render()),
        }
    }
}

fn serde_value_to_json_value(v: serde_json::Value) -> JsonValue {
    match v {
        serde_json::Value::Null => JsonValue::Null,
        serde_json::Value::Bool(b) => JsonValue::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                JsonValue::Number(i)
            } else {
                JsonValue::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => JsonValue::String(s),
        serde_json::Value::Array(arr) => {
            JsonValue::Array(arr.into_iter().map(serde_value_to_json_value).collect())
        }
        serde_json::Value::Object(map) => JsonValue::Object(serde_value_to_json_value_map(map)),
    }
}

fn serde_value_to_json_value_map(
    map: serde_json::Map<String, serde_json::Value>,
) -> BTreeMap<String, JsonValue> {
    map.into_iter()
        .map(|(k, v)| (k, serde_value_to_json_value(v)))
        .collect()
}

fn json_value_to_serde(v: &JsonValue) -> serde_json::Value {
    match v {
        JsonValue::Null => serde_json::Value::Null,
        JsonValue::Bool(b) => serde_json::Value::Bool(*b),
        JsonValue::Number(n) => serde_json::Value::Number((*n).into()),
        JsonValue::String(s) => serde_json::Value::String(s.clone()),
        JsonValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(json_value_to_serde).collect())
        }
        JsonValue::Object(map) => serde_json::Value::Object(json_value_map_to_serde(map)),
    }
}

fn json_value_map_to_serde(
    map: &BTreeMap<String, JsonValue>,
) -> serde_json::Map<String, serde_json::Value> {
    map.iter()
        .map(|(k, v)| (k.clone(), json_value_to_serde(v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_value_detects_types() {
        assert_eq!(parse_value("true"), JsonValue::Bool(true));
        assert_eq!(parse_value("false"), JsonValue::Bool(false));
        assert_eq!(parse_value("null"), JsonValue::Null);
        assert_eq!(parse_value("42"), JsonValue::Number(42));
        assert_eq!(parse_value("hello"), JsonValue::String("hello".into()));
        assert_eq!(
            parse_value(r#"["a","b"]"#),
            JsonValue::Array(vec![
                JsonValue::String("a".into()),
                JsonValue::String("b".into())
            ])
        );
        let obj = parse_value(r#"{"key":"val"}"#);
        if let JsonValue::Object(map) = obj {
            assert_eq!(map.get("key"), Some(&JsonValue::String("val".into())));
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn set_nested_creates_deep_path() {
        let mut map = BTreeMap::new();
        set_nested(&mut map, &["a", "b", "c"], JsonValue::String("deep".into()));
        let a = match map.get("a").unwrap() {
            JsonValue::Object(o) => o,
            _ => panic!("expected object"),
        };
        let b = match a.get("b").unwrap() {
            JsonValue::Object(o) => o,
            _ => panic!("expected object"),
        };
        assert_eq!(b.get("c").unwrap(), &JsonValue::String("deep".into()));
    }

    #[test]
    fn set_nested_overwrites_existing() {
        let mut map = BTreeMap::new();
        map.insert("model".to_string(), JsonValue::String("old".into()));
        set_nested(&mut map, &["model"], JsonValue::String("new".into()));
        assert_eq!(map.get("model").unwrap(), &JsonValue::String("new".into()));
    }

    #[test]
    fn get_nested_traverses_dotted_key() {
        let mut inner = BTreeMap::new();
        inner.insert("key".to_string(), JsonValue::String("val".into()));
        let mut map = BTreeMap::new();
        map.insert("outer".to_string(), JsonValue::Object(inner));

        assert_eq!(
            get_nested(&map, "outer.key"),
            Some(JsonValue::String("val".into()))
        );
        assert_eq!(get_nested(&map, "missing"), None);
    }

    #[test]
    fn get_nested_returns_none_for_non_object_intermediate() {
        let mut map = BTreeMap::new();
        map.insert("flat".to_string(), JsonValue::String("value".into()));
        assert_eq!(get_nested(&map, "flat.sub"), None);
    }

    #[test]
    fn get_nested_returns_top_level_value() {
        let mut map = BTreeMap::new();
        map.insert("model".to_string(), JsonValue::String("sonnet".into()));
        assert_eq!(
            get_nested(&map, "model"),
            Some(JsonValue::String("sonnet".into()))
        );
    }

    #[test]
    fn set_nested_overwrites_scalar_with_object() {
        let mut map = BTreeMap::new();
        map.insert("a".to_string(), JsonValue::String("scalar".into()));
        set_nested(&mut map, &["a", "b"], JsonValue::Number(42));
        let a = match map.get("a").unwrap() {
            JsonValue::Object(o) => o,
            _ => panic!("expected object after overwrite"),
        };
        assert_eq!(a.get("b").unwrap(), &JsonValue::Number(42));
    }

    #[test]
    fn parse_value_handles_negative_numbers() {
        assert_eq!(parse_value("-7"), JsonValue::Number(-7));
    }

    #[test]
    fn parse_value_handles_float_as_string() {
        let val = parse_value("3.14");
        assert!(matches!(val, JsonValue::String(_) | JsonValue::Number(_)));
    }

    #[test]
    fn parse_value_bare_string_not_valid_json() {
        assert_eq!(
            parse_value("hello world"),
            JsonValue::String("hello world".into())
        );
    }

    #[test]
    fn parse_value_nested_json_object() {
        let val = parse_value(r#"{"a":{"b":1}}"#);
        if let JsonValue::Object(outer) = val {
            if let Some(JsonValue::Object(inner)) = outer.get("a") {
                assert_eq!(inner.get("b"), Some(&JsonValue::Number(1)));
            } else {
                panic!("expected nested object");
            }
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn serde_roundtrip_preserves_structure() {
        let mut map = BTreeMap::new();
        map.insert("flag".to_string(), JsonValue::Bool(true));
        map.insert("count".to_string(), JsonValue::Number(99));
        map.insert(
            "items".to_string(),
            JsonValue::Array(vec![JsonValue::String("x".into())]),
        );
        let serde_map = json_value_map_to_serde(&map);
        let back = serde_value_to_json_value_map(serde_map);
        assert_eq!(map, back);
    }
}
