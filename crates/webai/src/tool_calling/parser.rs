use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// A tool call extracted from model text output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Quick check whether `text` likely contains a tool call.
pub fn has_tool_call(text: &str) -> bool {
    static FENCED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```tool_json\s*\n").unwrap());
    static BARE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\{\s*"tool"\s*:"#).unwrap());
    static XML_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<tool_call>").unwrap());
    static WRAPPER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"\{\s*"tool_calls"\s*:\s*\["#).unwrap());

    FENCED_RE.is_match(text)
        || BARE_RE.is_match(text)
        || XML_RE.is_match(text)
        || WRAPPER_RE.is_match(text)
}

/// Extract all tool calls from model output text.
///
/// Tries multiple patterns in priority order (matching TFG's `extractToolCalls`):
/// 1. `<tool_call>...</tool_call>` XML blocks (can be multiple)
/// 2. Fenced `tool_json` code blocks
/// 3. `{"tool_calls":[...]}` wrapper
/// 4. Bare `{"tool":"...","parameters":{...}}`
pub fn extract_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    if let Some(calls) = extract_xml_tool_calls(text) {
        if !calls.is_empty() {
            return calls;
        }
    }

    if let Some(call) = extract_fenced_tool_call(text) {
        return vec![call];
    }

    if let Some(call) = extract_wrapper_tool_call(text) {
        return vec![call];
    }

    if let Some(call) = extract_bare_tool_call(text) {
        return vec![call];
    }

    Vec::new()
}

fn extract_xml_tool_calls(text: &str) -> Option<Vec<ParsedToolCall>> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<tool_call>(.*?)</tool_call>").unwrap());

    let captures: Vec<_> = RE.captures_iter(text).collect();
    if captures.is_empty() {
        return None;
    }

    let calls: Vec<ParsedToolCall> = captures
        .iter()
        .filter_map(|cap| parse_tool_json(cap.get(1)?.as_str()))
        .collect();

    Some(calls)
}

fn extract_fenced_tool_call(text: &str) -> Option<ParsedToolCall> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)```tool_json\s*\n(.*?)```").unwrap());

    RE.captures(text)
        .and_then(|cap| parse_tool_json(cap.get(1)?.as_str()))
}

fn extract_wrapper_tool_call(text: &str) -> Option<ParsedToolCall> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\{\s*"tool_calls"\s*:\s*\[(\{.*?\})\s*(?:,|\])"#).unwrap()
    });

    RE.captures(text)
        .and_then(|cap| parse_tool_json(cap.get(1)?.as_str()))
}

fn extract_bare_tool_call(text: &str) -> Option<ParsedToolCall> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\{\s*"tool"\s*:\s*"[^"]+"\s*,\s*"parameters"\s*:\s*\{.*?\}\s*\}"#)
            .unwrap()
    });

    RE.captures(text)
        .and_then(|cap| parse_tool_json(cap.get(0)?.as_str()))
}

/// Parse a JSON string that may use either `tool`/`parameters` or `name`/`arguments` keys.
fn parse_tool_json(raw: &str) -> Option<ParsedToolCall> {
    let trimmed = raw.trim();
    let balanced = balance_braces(trimmed);
    let value: serde_json::Value = serde_json::from_str(&balanced).ok()?;
    let obj = value.as_object()?;

    let name = obj
        .get("tool")
        .or_else(|| obj.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)?;

    let arguments = obj
        .get("parameters")
        .or_else(|| obj.get("arguments"))
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    Some(ParsedToolCall { name, arguments })
}

/// Append closing braces to balance unclosed `{`.
fn balance_braces(s: &str) -> std::borrow::Cow<'_, str> {
    let open = s.chars().filter(|c| *c == '{').count();
    let close = s.chars().filter(|c| *c == '}').count();
    if open > close {
        let mut result = s.to_string();
        for _ in 0..(open - close) {
            result.push('}');
        }
        result.into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fenced_tool_call() {
        let text = "Sure, let me help.\n```tool_json\n{\"tool\":\"read_file\",\"parameters\":{\"path\":\"/tmp/test\"}}\n```\n";
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
    }

    #[test]
    fn test_xml_tool_calls() {
        let text = "<tool_call>{\"tool\":\"a\",\"parameters\":{}}</tool_call> then <tool_call>{\"tool\":\"b\",\"parameters\":{\"x\":1}}</tool_call>";
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[1].name, "b");
    }

    #[test]
    fn test_has_tool_call() {
        assert!(has_tool_call("```tool_json\n{}```"));
        assert!(has_tool_call("{\"tool\":\"x\"}"));
        assert!(!has_tool_call("No tools here."));
    }

    #[test]
    fn test_name_arguments_variant() {
        let raw = r#"{"name":"test","arguments":{"key":"val"}}"#;
        let parsed = parse_tool_json(raw).unwrap();
        assert_eq!(parsed.name, "test");
    }
}
