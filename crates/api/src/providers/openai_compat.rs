#[path = "openai_compat_sse.rs"]
mod openai_compat_sse;
#[path = "openai_compat_stream.rs"]
mod openai_compat_stream;

use std::collections::VecDeque;

use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::providers::{parse_custom_provider_prefix, RetryPolicy};
use crate::types::{
    InputContentBlock, InputMessage, MessageRequest, MessageResponse, OutputContentBlock,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};

use openai_compat_sse::{first_non_empty_field, OpenAiSseParser};
use openai_compat_stream::StreamState;

pub use openai_compat_stream::MessageStream;

pub const DEFAULT_XAI_BASE_URL: &str = "https://api.x.ai/v1";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const REQUEST_ID_HEADER: &str = "request-id";
const ALT_REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiCompatConfig {
    pub provider_name: &'static str,
    pub api_key_env: &'static str,
    pub base_url_env: &'static str,
    pub default_base_url: &'static str,
}

const XAI_ENV_VARS: &[&str] = &["XAI_API_KEY"];
const OPENAI_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];

impl OpenAiCompatConfig {
    #[must_use]
    pub const fn xai() -> Self {
        Self {
            provider_name: "xAI",
            api_key_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: DEFAULT_XAI_BASE_URL,
        }
    }

    #[must_use]
    pub const fn openai() -> Self {
        Self {
            provider_name: "OpenAI",
            api_key_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: DEFAULT_OPENAI_BASE_URL,
        }
    }
    #[must_use]
    pub fn credential_env_vars(self) -> &'static [&'static str] {
        match self.api_key_env {
            "XAI_API_KEY" => XAI_ENV_VARS,
            "OPENAI_API_KEY" => OPENAI_ENV_VARS,
            _ => &[],
        }
    }
}

#[derive(Clone)]
pub struct OpenAiCompatClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    endpoint_query: Option<String>,
    retry: RetryPolicy,
    custom_headers: std::collections::BTreeMap<String, String>,
}

impl std::fmt::Debug for OpenAiCompatClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatClient")
            .field("base_url", &self.base_url)
            .field("endpoint_query", &self.endpoint_query)
            .field("api_key", &"***")
            .finish()
    }
}

impl OpenAiCompatClient {
    #[must_use]
    pub fn new(api_key: impl Into<String>, config: OpenAiCompatConfig) -> Self {
        Self {
            http: crate::default_http_client(),
            api_key: api_key.into(),
            base_url: read_base_url(config),
            endpoint_query: None,
            retry: RetryPolicy::default(),
            custom_headers: std::collections::BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn new_custom(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: crate::default_http_client(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            endpoint_query: None,
            retry: RetryPolicy::default(),
            custom_headers: std::collections::BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_endpoint_query(mut self, endpoint_query: Option<String>) -> Self {
        self.endpoint_query = endpoint_query
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self
    }

    #[must_use]
    pub fn with_custom_headers(
        mut self,
        headers: std::collections::BTreeMap<String, String>,
    ) -> Self {
        self.custom_headers = headers;
        self
    }

    pub fn from_env(config: OpenAiCompatConfig) -> Result<Self, ApiError> {
        let Some(api_key) = read_env_non_empty(config.api_key_env)? else {
            return Err(ApiError::missing_credentials(
                config.provider_name,
                config.credential_env_vars(),
            ));
        };
        Ok(Self::new(api_key, config))
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    #[must_use]
    pub fn with_retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        let response = self.send_with_retry(&request).await?;
        let request_id = request_id_from_headers(response.headers());
        let payload = response.json::<ChatCompletionResponse>().await?;
        let mut normalized = normalize_response(&request.model, payload)?;
        if normalized.request_id.is_none() {
            normalized.request_id = request_id;
        }
        Ok(normalized)
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        let response = self
            .send_with_retry(&request.clone().with_streaming())
            .await?;
        Ok(MessageStream {
            request_id: request_id_from_headers(response.headers()),
            response,
            parser: OpenAiSseParser::new(),
            pending: VecDeque::new(),
            done: false,
            state: StreamState::new(request.model.clone()),
        })
    }

    async fn send_with_retry(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let mut attempts = 0;

        let last_error = loop {
            attempts += 1;
            let retryable_error = match self.send_raw_request(request).await {
                Ok(response) => match expect_success(response).await {
                    Ok(response) => return Ok(response),
                    Err(error)
                        if error.is_retryable() && attempts <= self.retry.max_retries + 1 =>
                    {
                        error
                    }
                    Err(error) => return Err(error),
                },
                Err(error) if error.is_retryable() && attempts <= self.retry.max_retries + 1 => {
                    error
                }
                Err(error) => return Err(error),
            };

            if attempts > self.retry.max_retries {
                break retryable_error;
            }

            tokio::time::sleep(self.backoff_for_attempt(attempts)?).await;
        };

        Err(ApiError::RetriesExhausted {
            attempts,
            last_error: Box::new(last_error),
        })
    }

    async fn send_raw_request(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let request_url = chat_completions_endpoint(&self.base_url, self.endpoint_query.as_deref());
        let mut req = self
            .http
            .post(&request_url)
            .header("content-type", "application/json");
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        for (name, value) in &self.custom_headers {
            req = req.header(name.as_str(), value.as_str());
        }
        req.json(&build_chat_completion_request(request))
            .send()
            .await
            .map_err(ApiError::from)
    }

    fn backoff_for_attempt(&self, attempt: u32) -> Result<std::time::Duration, ApiError> {
        let Some(multiplier) = 1_u32.checked_shl(attempt.saturating_sub(1)) else {
            return Err(ApiError::BackoffOverflow {
                attempt,
                base_delay: self.retry.initial_backoff,
            });
        };
        Ok(self
            .retry
            .initial_backoff
            .checked_mul(multiplier)
            .map_or(self.retry.max_backoff, |delay| {
                delay.min(self.retry.max_backoff)
            }))
    }
}

// ---------------------------------------------------------------------------
// Non-streaming DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    id: String,
    model: String,
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(default, deserialize_with = "deserialize_openai_text_content")]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    thought: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ResponseToolCall>,
}

impl ChatMessage {
    fn assistant_visible_text(&self) -> Option<String> {
        first_non_empty_field(&[
            &self.content,
            &self.reasoning_content,
            &self.reasoning,
            &self.thought,
            &self.thinking,
        ])
    }
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    function: ResponseToolFunction,
}

#[derive(Debug, Deserialize)]
struct ResponseToolFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenAiUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(rename = "type")]
    error_type: Option<String>,
    message: Option<String>,
}

// ---------------------------------------------------------------------------
// Request / response mapping
// ---------------------------------------------------------------------------

fn upstream_openai_model(model: &str) -> String {
    parse_custom_provider_prefix(model)
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| model.to_string())
}

fn build_chat_completion_request(request: &MessageRequest) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = request.system.as_ref().filter(|value| !value.is_empty()) {
        messages.push(json!({
            "role": "system",
            "content": system,
        }));
    }
    for message in &request.messages {
        messages.extend(translate_message(message));
    }

    let upstream_model = upstream_openai_model(&request.model);
    const MAX_TOKENS_OPENAI_COMPAT_CAP: u32 = 32_768;
    let max_tokens = request.max_tokens.clamp(1, MAX_TOKENS_OPENAI_COMPAT_CAP);
    let mut payload = json!({
        "model": upstream_model,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": request.stream,
    });

    if let Some(tools) = &request.tools {
        payload["tools"] =
            Value::Array(tools.iter().map(openai_tool_definition).collect::<Vec<_>>());
    }
    if let Some(tool_choice) = &request.tool_choice {
        payload["tool_choice"] = openai_tool_choice(tool_choice);
    }

    payload
}

fn translate_message(message: &InputMessage) -> Vec<Value> {
    match message.role.as_str() {
        "assistant" => {
            let mut text = String::new();
            let mut tool_calls = Vec::new();
            for block in &message.content {
                match block {
                    InputContentBlock::Text { text: value } => text.push_str(value),
                    InputContentBlock::ToolUse { id, name, input } => tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(input).unwrap_or_default(),
                        }
                    })),
                    InputContentBlock::Image { .. } | InputContentBlock::ToolResult { .. } => {}
                }
            }
            if text.is_empty() && tool_calls.is_empty() {
                Vec::new()
            } else {
                let mut msg = json!({
                    "role": "assistant",
                    "content": (!text.is_empty()).then_some(text),
                });
                // Only include tool_calls when non-empty; some providers
                // (e.g. DashScope) reject an empty array.
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = json!(tool_calls);
                }
                vec![msg]
            }
        }
        _ => {
            let has_image = message
                .content
                .iter()
                .any(|b| matches!(b, InputContentBlock::Image { .. }));
            let mut result = Vec::new();
            let mut user_parts: Vec<Value> = Vec::new();

            for block in &message.content {
                match block {
                    InputContentBlock::Text { text } => {
                        if has_image {
                            user_parts.push(json!({ "type": "text", "text": text }));
                        } else {
                            result.push(json!({ "role": "user", "content": text }));
                        }
                    }
                    InputContentBlock::Image { source } => {
                        let data_url = format!("data:{};base64,{}", source.media_type, source.data);
                        user_parts.push(json!({
                            "type": "image_url",
                            "image_url": { "url": data_url }
                        }));
                    }
                    InputContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        flush_user_parts(&mut user_parts, &mut result);
                        result.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": flatten_tool_result_content(content),
                            "is_error": is_error,
                        }));
                    }
                    InputContentBlock::ToolUse { .. } => {}
                }
            }
            flush_user_parts(&mut user_parts, &mut result);
            result
        }
    }
}

fn flush_user_parts(parts: &mut Vec<Value>, result: &mut Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    let content = Value::Array(std::mem::take(parts));
    result.push(json!({ "role": "user", "content": content }));
}

fn flatten_tool_result_content(content: &[ToolResultContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ToolResultContentBlock::Text { text } => Some(text.clone()),
            ToolResultContentBlock::Json { value } => Some(value.to_string()),
            ToolResultContentBlock::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn openai_tool_definition(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        }
    })
}

fn openai_tool_choice(tool_choice: &ToolChoice) -> Value {
    match tool_choice {
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::Any => Value::String("required".to_string()),
        ToolChoice::Tool { name } => json!({
            "type": "function",
            "function": { "name": name },
        }),
    }
}

fn normalize_response(
    model: &str,
    response: ChatCompletionResponse,
) -> Result<MessageResponse, ApiError> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or(ApiError::InvalidSseFrame(
            "chat completion response missing choices",
        ))?;
    let mut content = Vec::new();
    if let Some(text) = choice.message.assistant_visible_text() {
        content.push(OutputContentBlock::Text { text });
    }
    for tool_call in choice.message.tool_calls {
        content.push(OutputContentBlock::ToolUse {
            id: tool_call.id,
            name: tool_call.function.name,
            input: parse_tool_arguments(&tool_call.function.arguments),
        });
    }

    Ok(MessageResponse {
        id: response.id,
        kind: "message".to_string(),
        role: choice.message.role,
        content,
        model: response.model.if_empty_then(model.to_string()),
        stop_reason: choice
            .finish_reason
            .map(|value| normalize_finish_reason(&value)),
        stop_sequence: None,
        usage: Usage {
            input_tokens: response
                .usage
                .as_ref()
                .map_or(0, |usage| usage.prompt_tokens),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            output_tokens: response
                .usage
                .as_ref()
                .map_or(0, |usage| usage.completion_tokens),
        },
        request_id: None,
    })
}

fn parse_tool_arguments(arguments: &str) -> Value {
    serde_json::from_str(arguments).unwrap_or_else(|_| json!({ "raw": arguments }))
}

// ---------------------------------------------------------------------------
// Deserialization helpers
// ---------------------------------------------------------------------------

/// OpenAI-compatible APIs usually send a string; some use `[{type,text}]`-style array parts.
fn deserialize_openai_text_content<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Str(String),
        Arr(Vec<Value>),
    }
    match Option::<Raw>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Raw::Str(s)) if s.is_empty() => Ok(None),
        Some(Raw::Str(s)) => Ok(Some(s)),
        Some(Raw::Arr(parts)) => {
            let mut joined = String::new();
            for part in parts {
                match part {
                    Value::Object(map) => {
                        if let Some(text) = map.get("text").and_then(Value::as_str) {
                            joined.push_str(text);
                        } else if let Some(text) = map.get("content").and_then(Value::as_str) {
                            joined.push_str(text);
                        }
                    }
                    Value::String(s) => joined.push_str(&s),
                    _ => {}
                }
            }
            Ok((!joined.is_empty()).then_some(joined))
        }
    }
}

// ---------------------------------------------------------------------------
// Env / URL / HTTP helpers
// ---------------------------------------------------------------------------

fn read_env_non_empty(key: &str) -> Result<Option<String>, ApiError> {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(ApiError::from(error)),
    }
}

#[must_use]
pub fn has_api_key(key: &str) -> bool {
    read_env_non_empty(key)
        .ok()
        .and_then(std::convert::identity)
        .is_some()
}

#[must_use]
pub fn read_base_url(config: OpenAiCompatConfig) -> String {
    std::env::var(config.base_url_env).unwrap_or_else(|_| config.default_base_url.to_string())
}

fn chat_completions_endpoint(base_url: &str, extra_query: Option<&str>) -> String {
    let trimmed = base_url.trim();
    let (path_part, base_query) = match trimmed.split_once('?') {
        Some((p, q)) => (p.trim_end_matches('/'), Some(q)),
        None => (trimmed.trim_end_matches('/'), None),
    };
    let path = if path_part.ends_with("/chat/completions") {
        path_part.to_string()
    } else {
        format!("{path_part}/chat/completions")
    };
    merge_url_query(&path, base_query, extra_query)
}

fn merge_url_query(path: &str, base_query: Option<&str>, extra_query: Option<&str>) -> String {
    let mut segments: Vec<&str> = Vec::new();
    if let Some(q) = base_query.map(str::trim).filter(|q| !q.is_empty()) {
        segments.push(q);
    }
    if let Some(q) = extra_query.map(str::trim).filter(|q| !q.is_empty()) {
        segments.push(q);
    }
    if segments.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", segments.join("&"))
    }
}

fn request_id_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .or_else(|| headers.get(ALT_REQUEST_ID_HEADER))
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

async fn expect_success(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_default();
    let parsed_error = serde_json::from_str::<ErrorEnvelope>(&body).ok();
    let retryable = is_retryable_status(status);

    Err(ApiError::Api {
        status,
        error_type: parsed_error
            .as_ref()
            .and_then(|error| error.error.error_type.clone()),
        message: parsed_error
            .as_ref()
            .and_then(|error| error.error.message.clone()),
        body,
        retryable,
    })
}

const fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 409 | 429 | 500 | 502 | 503 | 504)
}

fn normalize_finish_reason(value: &str) -> String {
    match value {
        "stop" => "end_turn",
        "tool_calls" => "tool_use",
        other => other,
    }
    .to_string()
}

trait StringExt {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringExt for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

#[cfg(test)]
mod openai_compat_inner_tests {
    use super::*;
    use crate::types::OutputContentBlock;

    #[test]
    fn chat_completions_url_appends_api_version() {
        assert_eq!(
            chat_completions_endpoint(
                "https://my.openai.azure.com/openai/deployments/gpt4",
                Some("api-version=2024-02-15-preview"),
            ),
            "https://my.openai.azure.com/openai/deployments/gpt4/chat/completions?api-version=2024-02-15-preview"
        );
    }

    #[test]
    fn chat_completions_url_merges_base_query_and_api_version() {
        assert_eq!(
            chat_completions_endpoint(
                "https://x/v1/chat/completions?existing=1",
                Some("api-version=2024-02-15-preview"),
            ),
            "https://x/v1/chat/completions?existing=1&api-version=2024-02-15-preview"
        );
    }

    #[test]
    fn non_streaming_message_parses_content_array() {
        let json = r#"{
            "id":"1",
            "model":"qwen",
            "choices":[{
                "message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},
                "finish_reason":"stop"
            }],
            "usage":{"prompt_tokens":1,"completion_tokens":1}
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let msg = normalize_response("qwen", resp).expect("normalize");
        assert_eq!(
            msg.content,
            vec![OutputContentBlock::Text {
                text: "hello".to_string()
            }]
        );
    }

    #[test]
    fn non_streaming_reasoning_only_message() {
        let json = r#"{
            "id":"1",
            "model":"qwen",
            "choices":[{
                "message":{"role":"assistant","content":null,"reasoning_content":"think"},
                "finish_reason":"stop"
            }],
            "usage":{"prompt_tokens":1,"completion_tokens":1}
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let msg = normalize_response("qwen", resp).expect("normalize");
        assert_eq!(
            msg.content,
            vec![OutputContentBlock::Text {
                text: "think".to_string()
            }]
        );
    }

    #[test]
    fn translate_user_message_with_image_produces_content_array() {
        use crate::types::ImageSource;
        let msg = InputMessage {
            role: "user".to_string(),
            content: vec![
                InputContentBlock::Text {
                    text: "describe this".to_string(),
                },
                InputContentBlock::Image {
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: "image/png".to_string(),
                        data: "abc123".to_string(),
                    },
                },
            ],
        };
        let result = translate_message(&msg);
        assert_eq!(result.len(), 1);
        let content = &result[0]["content"];
        assert!(content.is_array(), "content should be an array");
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "describe this");
        assert_eq!(arr[1]["type"], "image_url");
        assert_eq!(arr[1]["image_url"]["url"], "data:image/png;base64,abc123");
    }

    #[test]
    fn translate_text_only_user_message_stays_string() {
        let msg = InputMessage::user_text("hello");
        let result = translate_message(&msg);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["content"], "hello");
    }
}

#[cfg(test)]
#[path = "openai_compat_tests.rs"]
mod tests;
