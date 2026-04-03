use super::{
    build_chat_completion_request, chat_completions_endpoint, normalize_finish_reason,
    openai_tool_choice, parse_tool_arguments, OpenAiCompatClient, OpenAiCompatConfig,
};
use crate::error::ApiError;
use crate::types::{
    InputContentBlock, InputMessage, MessageRequest, ToolChoice, ToolDefinition,
    ToolResultContentBlock,
};
use serde_json::json;
use std::sync::{Mutex, OnceLock};

#[test]
fn request_translation_uses_openai_compatible_shape() {
    let payload = build_chat_completion_request(&MessageRequest {
        model: "grok-3".to_string(),
        max_tokens: 64,
        messages: vec![InputMessage {
            role: "user".to_string(),
            content: vec![
                InputContentBlock::Text {
                    text: "hello".to_string(),
                },
                InputContentBlock::ToolResult {
                    tool_use_id: "tool_1".to_string(),
                    content: vec![ToolResultContentBlock::Json {
                        value: json!({"ok": true}),
                    }],
                    is_error: false,
                },
            ],
        }],
        system: Some("be helpful".to_string()),
        tools: Some(vec![ToolDefinition {
            name: "weather".to_string(),
            description: Some("Get weather".to_string()),
            input_schema: json!({"type": "object"}),
        }]),
        tool_choice: Some(ToolChoice::Auto),
        stream: false,
    });

    assert_eq!(payload["messages"][0]["role"], json!("system"));
    assert_eq!(payload["messages"][1]["role"], json!("user"));
    assert_eq!(payload["messages"][2]["role"], json!("tool"));
    assert_eq!(payload["tools"][0]["type"], json!("function"));
    assert_eq!(payload["tool_choice"], json!("auto"));
}

#[test]
fn tool_choice_translation_supports_required_function() {
    assert_eq!(openai_tool_choice(&ToolChoice::Any), json!("required"));
    assert_eq!(
        openai_tool_choice(&ToolChoice::Tool {
            name: "weather".to_string(),
        }),
        json!({"type": "function", "function": {"name": "weather"}})
    );
}

#[test]
fn parses_tool_arguments_fallback() {
    assert_eq!(
        parse_tool_arguments("{\"city\":\"Paris\"}"),
        json!({"city": "Paris"})
    );
    assert_eq!(parse_tool_arguments("not-json"), json!({"raw": "not-json"}));
}

#[test]
fn missing_xai_api_key_is_provider_specific() {
    let _lock = env_lock();
    std::env::remove_var("XAI_API_KEY");
    let error = OpenAiCompatClient::from_env(OpenAiCompatConfig::xai()).expect_err("missing key should error");
    assert!(matches!(
        error,
        ApiError::MissingCredentials {
            provider: "xAI",
            ..
        }
    ));
}

#[test]
fn endpoint_builder_accepts_base_urls_and_full_endpoints() {
    assert_eq!(
        chat_completions_endpoint("https://api.x.ai/v1"),
        "https://api.x.ai/v1/chat/completions"
    );
    assert_eq!(
        chat_completions_endpoint("https://api.x.ai/v1/"),
        "https://api.x.ai/v1/chat/completions"
    );
    assert_eq!(
        chat_completions_endpoint("https://api.x.ai/v1/chat/completions"),
        "https://api.x.ai/v1/chat/completions"
    );
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

#[test]
fn normalizes_stop_reasons() {
    assert_eq!(normalize_finish_reason("stop"), "end_turn");
    assert_eq!(normalize_finish_reason("tool_calls"), "tool_use");
}
