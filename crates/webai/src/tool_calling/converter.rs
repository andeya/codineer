use serde::{Deserialize, Serialize};

use super::parser::{extract_tool_calls, has_tool_call};
use super::prompt::{build_tool_prompt, detect_language};

/// Result of converting OpenAI messages + tools into a plain-text prompt.
#[derive(Debug, Clone)]
pub struct ConvertedPrompt {
    pub prompt: String,
    pub has_tools: bool,
}

/// OpenAI-format tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// OpenAI-format `tool_choice` value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String),
    Object {
        r#type: String,
        function: ToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

/// Tool call output for the OpenAI response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallOutput {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// A chat message in the OpenAI format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// Build a plain-text prompt from OpenAI-format messages, optionally injecting tool definitions.
pub fn build_prompt_from_messages(
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
    tool_choice: Option<&ToolChoice>,
) -> ConvertedPrompt {
    let (effective_tools, force_use) = resolve_effective_tools(tools, tool_choice);
    let has_tools = !effective_tools.is_empty();

    let mut parts: Vec<String> = Vec::new();

    if has_tools {
        let defs: Vec<serde_json::Value> = effective_tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.function.name,
                    "description": t.function.description,
                    "parameters": t.function.parameters,
                })
            })
            .collect();
        let tools_json = serde_json::to_string_pretty(&defs).unwrap_or_default();

        let lang = detect_last_user_language(messages);
        parts.push(build_tool_prompt(&tools_json, lang, force_use));
    }

    for msg in messages {
        parts.push(format_message(msg));
    }

    if let Some(last) = messages.last() {
        if last.role == "tool" || last.role == "function" {
            let lang = detect_last_user_language(messages);
            let hint = if lang == "cn" {
                "请根据上述工具结果回答用户的问题。"
            } else {
                "Please answer the user's question based on the tool results above."
            };
            parts.push(hint.to_string());
        }
    }

    ConvertedPrompt {
        prompt: parts.join("\n\n"),
        has_tools,
    }
}

/// Parse the model's text response, extracting tool calls if tools were injected.
pub fn parse_tool_response(
    text: &str,
    requested_tools: Option<&[ToolDefinition]>,
) -> ParsedResponse {
    let tool_names: Vec<&str> = requested_tools
        .map(|tools| tools.iter().map(|t| t.function.name.as_str()).collect())
        .unwrap_or_default();

    if tool_names.is_empty() || !has_tool_call(text) {
        return ParsedResponse {
            content: Some(text.to_string()),
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
        };
    }

    let calls = extract_tool_calls(text);
    let valid_calls: Vec<_> = calls
        .into_iter()
        .filter(|c| tool_names.contains(&c.name.as_str()))
        .collect();

    if valid_calls.is_empty() {
        return ParsedResponse {
            content: Some(text.to_string()),
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
        };
    }

    let tool_call_outputs: Vec<ToolCallOutput> = valid_calls
        .into_iter()
        .map(|c| {
            let id = format!("call_{}", uuid::Uuid::new_v4().simple());
            ToolCallOutput {
                id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: c.name,
                    arguments: serde_json::to_string(&c.arguments).unwrap_or_default(),
                },
            }
        })
        .collect();

    ParsedResponse {
        content: None,
        tool_calls: tool_call_outputs,
        finish_reason: "tool_calls".to_string(),
    }
}

/// Result of parsing a model response.
#[derive(Debug, Clone)]
pub struct ParsedResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallOutput>,
    pub finish_reason: String,
}

fn resolve_effective_tools<'a>(
    tools: Option<&'a [ToolDefinition]>,
    tool_choice: Option<&ToolChoice>,
) -> (Vec<&'a ToolDefinition>, bool) {
    let tools = match tools {
        Some(t) if !t.is_empty() => t,
        _ => return (Vec::new(), false),
    };

    match tool_choice {
        Some(ToolChoice::String(s)) if s == "none" => (Vec::new(), false),
        Some(ToolChoice::String(s)) if s == "required" => (tools.iter().collect(), true),
        Some(ToolChoice::Object { function, .. }) => {
            let matched: Vec<_> = tools
                .iter()
                .filter(|t| t.function.name == function.name)
                .collect();
            let force = !matched.is_empty();
            (matched, force)
        }
        _ => (tools.iter().collect(), false),
    }
}

fn detect_last_user_language(messages: &[ChatMessage]) -> &'static str {
    for msg in messages.iter().rev() {
        if msg.role == "user" {
            if let Some(content) = &msg.content {
                let text = content_to_text(content);
                if !text.is_empty() {
                    return detect_language(&text);
                }
            }
        }
    }
    "en"
}

fn content_to_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|part| {
                if part.get("type")?.as_str()? == "text" {
                    part.get("text")?.as_str().map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn format_message(msg: &ChatMessage) -> String {
    match msg.role.as_str() {
        "system" | "developer" => {
            let text = msg
                .content
                .as_ref()
                .map(content_to_text)
                .unwrap_or_default();
            format!("System: {text}")
        }
        "user" => {
            let text = msg
                .content
                .as_ref()
                .map(content_to_text)
                .unwrap_or_default();
            format!("Human: {text}")
        }
        "assistant" => {
            if let Some(tool_calls) = &msg.tool_calls {
                let calls_text: Vec<String> = tool_calls
                    .iter()
                    .filter_map(|tc| {
                        let func = tc.get("function")?;
                        let name = func.get("name")?.as_str()?;
                        let args = func.get("arguments")?.as_str().unwrap_or("{}");
                        Some(format!(
                            "```tool_json\n{{\"tool\":\"{name}\",\"parameters\":{args}}}\n```"
                        ))
                    })
                    .collect();
                format!("Assistant: [Called tools]\n{}", calls_text.join("\n"))
            } else {
                let text = msg
                    .content
                    .as_ref()
                    .map(content_to_text)
                    .unwrap_or_default();
                format!("Assistant: {text}")
            }
        }
        "tool" | "function" => {
            let id = msg
                .tool_call_id
                .as_deref()
                .or(msg.name.as_deref())
                .unwrap_or("unknown");
            let text = msg
                .content
                .as_ref()
                .map(content_to_text)
                .unwrap_or_default();
            format!("<tool_result tool_call_id=\"{id}\">\n{text}\n</tool_result>")
        }
        _ => {
            let text = msg
                .content
                .as_ref()
                .map(content_to_text)
                .unwrap_or_default();
            format!("{}: {text}", msg.role)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            r#type: "function".into(),
            function: FunctionDef {
                name: name.into(),
                description: Some("test".into()),
                parameters: None,
            },
        }
    }

    #[test]
    fn test_build_prompt_no_tools() {
        let msgs = vec![ChatMessage {
            role: "user".into(),
            content: Some(serde_json::Value::String("Hello".into())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        let result = build_prompt_from_messages(&msgs, None, None);
        assert!(!result.has_tools);
        assert!(result.prompt.contains("Human: Hello"));
    }

    #[test]
    fn test_build_prompt_with_tools() {
        let tools = vec![make_tool("read_file")];
        let msgs = vec![ChatMessage {
            role: "user".into(),
            content: Some(serde_json::Value::String("Read the file".into())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        let result = build_prompt_from_messages(&msgs, Some(&tools), None);
        assert!(result.has_tools);
        assert!(result.prompt.contains("read_file"));
        assert!(result.prompt.contains("Human: Read the file"));
    }

    #[test]
    fn test_parse_tool_response_no_tools() {
        let resp = parse_tool_response("Hello world", None);
        assert_eq!(resp.finish_reason, "stop");
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_tool_response_with_call() {
        let tools = vec![make_tool("read_file")];
        let text = "```tool_json\n{\"tool\":\"read_file\",\"parameters\":{\"path\":\"/tmp\"}}\n```";
        let resp = parse_tool_response(text, Some(&tools));
        assert_eq!(resp.finish_reason, "tool_calls");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].function.name, "read_file");
    }
}
