use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use tokio::sync::watch;

use aineer_api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, ProviderClient,
    StreamEvent, SystemBlock,
};
use aineer_webai::{OpenAiStreamResult, WebAiEngine};

use crate::config::GatewayConfig;
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayStatus {
    Starting,
    Running,
    Stopped,
    Error,
}

pub struct GatewayServer {
    config: GatewayConfig,
    webai: Option<WebAiEngine>,
    status_tx: watch::Sender<GatewayStatus>,
    status_rx: watch::Receiver<GatewayStatus>,
}

struct AppState {
    config: GatewayConfig,
    webai: Option<WebAiEngine>,
}

impl GatewayServer {
    pub fn new(config: GatewayConfig) -> Self {
        let (status_tx, status_rx) = watch::channel(GatewayStatus::Stopped);
        Self {
            config,
            webai: None,
            status_tx,
            status_rx,
        }
    }

    pub fn with_webai(mut self, engine: WebAiEngine) -> Self {
        self.webai = Some(engine);
        self
    }

    pub fn status(&self) -> GatewayStatus {
        *self.status_rx.borrow()
    }

    pub fn status_rx(&self) -> watch::Receiver<GatewayStatus> {
        self.status_rx.clone()
    }

    pub fn mark_starting(&self) {
        self.status_tx.send_replace(GatewayStatus::Starting);
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        if !self.config.enabled {
            tracing::info!("Gateway is disabled in config");
            self.status_tx.send_replace(GatewayStatus::Stopped);
            return Ok(());
        }

        let addr: SocketAddr = match self.config.listen_addr.parse() {
            Ok(a) => a,
            Err(e) => {
                self.status_tx.send_replace(GatewayStatus::Error);
                return Err(e.into());
            }
        };

        let state = Arc::new(AppState {
            config: self.config.clone(),
            webai: self.webai.clone(),
        });

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/v1/chat/completions", post(completions_handler))
            .route("/v1/models", get(models_handler))
            .with_state(state);

        self.status_tx.send_replace(GatewayStatus::Starting);

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Gateway failed to bind {addr}: {e}");
                self.status_tx.send_replace(GatewayStatus::Error);
                return Err(e.into());
            }
        };

        tracing::info!("Aineer Gateway listening on {}", addr);
        self.status_tx.send_replace(GatewayStatus::Running);

        if let Err(e) = axum::serve(listener, app).await {
            self.status_tx.send_replace(GatewayStatus::Error);
            return Err(e.into());
        }

        self.status_tx.send_replace(GatewayStatus::Stopped);
        Ok(())
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

async fn models_handler(State(state): State<Arc<AppState>>) -> Json<ModelListResponse> {
    let now = now_secs();

    let mut data: Vec<ModelInfo> = aineer_api::list_known_models(None)
        .into_iter()
        .map(|(id, kind)| ModelInfo {
            id: id.to_string(),
            object: "model".to_string(),
            created: now,
            owned_by: format!("{kind:?}"),
        })
        .collect();

    if let Some(ref engine) = state.webai {
        for provider in engine.list_providers() {
            for model in engine.list_models(&provider.id) {
                let short_name = provider.id.strip_suffix("-web").unwrap_or(&provider.id);
                data.push(ModelInfo {
                    id: format!("webai/{}/{}", short_name, model.id),
                    object: "model".to_string(),
                    created: now,
                    owned_by: format!("webai:{}", provider.name),
                });
            }
        }
    }

    Json(ModelListResponse {
        object: "list".to_string(),
        data,
    })
}

async fn completions_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let model = if req.model.is_empty() {
        state
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| "auto".to_string())
    } else {
        req.model.clone()
    };

    if WebAiEngine::parse_webai_model(&model).is_some() {
        return handle_webai(state, &req, &model).await;
    }

    let (system, messages) = convert_messages(&req.messages);
    let max_tokens = req
        .max_tokens
        .unwrap_or_else(|| aineer_api::max_tokens_for_model(&model));

    let tools = req.tools.as_ref().and_then(|t| convert_tools(t));
    let tool_choice = req.tool_choice.as_ref().and_then(convert_tool_choice);

    let api_request = MessageRequest {
        model: model.clone(),
        max_tokens,
        messages,
        system,
        tools,
        tool_choice,
        stream: req.stream.unwrap_or(false),
        thinking: None,
        gemini_cached_content: None,
    };

    let client = match ProviderClient::from_model(&api_request.model) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    format!("Failed to resolve provider for model '{}': {}", model, e),
                    "invalid_request_error",
                )),
            )
                .into_response();
        }
    };

    if req.stream.unwrap_or(false) {
        match handle_streaming(client, api_request, &model).await {
            Ok(sse) => sse.into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string(), "api_error")),
            )
                .into_response(),
        }
    } else {
        match handle_non_streaming(client, api_request, &model).await {
            Ok(json) => json.into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string(), "api_error")),
            )
                .into_response(),
        }
    }
}

async fn handle_non_streaming(
    client: ProviderClient,
    request: MessageRequest,
    model: &str,
) -> anyhow::Result<Json<ChatCompletionResponse>> {
    let response = client.send_message(&request).await?;

    let content_text = response
        .content
        .iter()
        .filter_map(|block| {
            if let aineer_api::OutputContentBlock::Text { text } = block {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let now = now_secs();

    Ok(Json(ChatCompletionResponse {
        id: response.id,
        object: "chat.completion".to_string(),
        created: now,
        model: model.to_string(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: Some(serde_json::Value::String(content_text)),
            },
            finish_reason: response.stop_reason.map(|r| map_finish_reason(&r)),
        }],
        usage: Some(UsageInfo {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        }),
    }))
}

async fn handle_streaming(
    client: ProviderClient,
    request: MessageRequest,
    model: &str,
) -> anyhow::Result<Sse<impl futures_core::Stream<Item = Result<Event, anyhow::Error>>>> {
    let mut stream = client.stream_message(&request).await?;
    let model = model.to_string();
    let now = now_secs();

    let sse_stream = async_stream::stream! {
        let id = format!("chatcmpl-{now}");

        let initial_chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: now,
            model: model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&initial_chunk).unwrap_or_default()));

        loop {
            match stream.next_event().await {
                Ok(Some(event)) => {
                    match event {
                        StreamEvent::ContentBlockDelta(delta_event) => {
                            let text = match &delta_event.delta {
                                ContentBlockDelta::TextDelta { text } => Some(text.clone()),
                                _ => None,
                            };

                            if let Some(text) = text {
                                let chunk = ChatCompletionChunk {
                                    id: id.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created: now,
                                    model: model.clone(),
                                    choices: vec![ChatChunkChoice {
                                        index: 0,
                                        delta: ChatDelta {
                                            role: None,
                                            content: Some(text),
                                        },
                                        finish_reason: None,
                                    }],
                                };
                                yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()));
                            }
                        }
                        StreamEvent::MessageStop(_) => {
                            let final_chunk = ChatCompletionChunk {
                                id: id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created: now,
                                model: model.clone(),
                                choices: vec![ChatChunkChoice {
                                    index: 0,
                                    delta: ChatDelta {
                                        role: None,
                                        content: None,
                                    },
                                    finish_reason: Some("stop".to_string()),
                                }],
                            };
                            yield Ok(Event::default().data(serde_json::to_string(&final_chunk).unwrap_or_default()));
                            yield Ok(Event::default().data("[DONE]"));
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(None) => {
                    yield Ok(Event::default().data("[DONE]"));
                    break;
                }
                Err(e) => {
                    let err = ErrorResponse::new(e.to_string(), "stream_error");
                    yield Ok(Event::default().data(serde_json::to_string(&err).unwrap_or_default()));
                    break;
                }
            }
        }
    };

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

fn convert_messages(messages: &[ChatMessage]) -> (Option<Vec<SystemBlock>>, Vec<InputMessage>) {
    let mut system_blocks: Vec<SystemBlock> = Vec::new();
    let mut input_messages: Vec<InputMessage> = Vec::new();

    for msg in messages {
        let text = match &msg.content {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        };

        match msg.role.as_str() {
            "system" => {
                system_blocks.extend(SystemBlock::from_plain(&text));
            }
            "assistant" => {
                input_messages.push(InputMessage {
                    role: "assistant".to_string(),
                    content: vec![InputContentBlock::Text {
                        text,
                        cache_control: None,
                    }],
                });
            }
            "tool" => {
                if let Some(tool_call_id) = msg
                    .content
                    .as_ref()
                    .and_then(|v| v.get("tool_call_id"))
                    .and_then(|v| v.as_str())
                {
                    input_messages.push(InputMessage {
                        role: "user".to_string(),
                        content: vec![InputContentBlock::ToolResult {
                            tool_use_id: tool_call_id.to_string(),
                            content: vec![aineer_api::ToolResultContentBlock::Text { text }],
                            is_error: false,
                            cache_control: None,
                        }],
                    });
                } else {
                    input_messages.push(InputMessage {
                        role: "user".to_string(),
                        content: vec![InputContentBlock::Text {
                            text,
                            cache_control: None,
                        }],
                    });
                }
            }
            _ => {
                input_messages.push(InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text,
                        cache_control: None,
                    }],
                });
            }
        }
    }

    let system = if system_blocks.is_empty() {
        None
    } else {
        Some(system_blocks)
    };

    (system, input_messages)
}

fn convert_tools(tools: &[serde_json::Value]) -> Option<Vec<aineer_api::ToolDefinition>> {
    let mut result = Vec::new();
    for tool in tools {
        let func = tool.get("function")?;
        let name = func.get("name")?.as_str()?.to_string();
        let description = func
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from);
        let input_schema = func
            .get("parameters")
            .cloned()
            .unwrap_or(serde_json::json!({"type": "object"}));
        result.push(aineer_api::ToolDefinition {
            name,
            description,
            input_schema,
            cache_control: None,
        });
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn convert_tool_choice(tc: &serde_json::Value) -> Option<aineer_api::ToolChoice> {
    match tc {
        serde_json::Value::String(s) => match s.as_str() {
            "auto" => Some(aineer_api::ToolChoice::Auto),
            "any" | "required" => Some(aineer_api::ToolChoice::Any),
            "none" => None,
            _ => Some(aineer_api::ToolChoice::Auto),
        },
        serde_json::Value::Object(obj) => {
            if let Some(name) = obj
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                Some(aineer_api::ToolChoice::Tool {
                    name: name.to_string(),
                })
            } else {
                Some(aineer_api::ToolChoice::Auto)
            }
        }
        _ => None,
    }
}

fn map_finish_reason(reason: &str) -> String {
    match reason {
        "end_turn" | "stop" => "stop".to_string(),
        "max_tokens" => "length".to_string(),
        "tool_use" => "tool_calls".to_string(),
        other => other.to_string(),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── WebAI handler ───────────────────────────────────────────────────

async fn handle_webai(
    state: Arc<AppState>,
    req: &ChatCompletionRequest,
    model: &str,
) -> axum::response::Response {
    let engine = match &state.webai {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "WebAI engine is not available (no Tauri runtime)".to_string(),
                    "service_unavailable",
                )),
            )
                .into_response();
        }
    };

    let (provider_name, specific_model) = match WebAiEngine::parse_webai_model(model) {
        Some(parsed) => parsed,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    format!("Invalid webai model format: {model}"),
                    "invalid_request_error",
                )),
            )
                .into_response();
        }
    };

    let model_id = specific_model.unwrap_or("").to_string();

    let chat_messages: Vec<aineer_webai::tool_calling::converter::ChatMessage> = req
        .messages
        .iter()
        .map(|m| aineer_webai::tool_calling::converter::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        })
        .collect();

    let tools: Option<Vec<aineer_webai::tool_calling::converter::ToolDefinition>> =
        req.tools.as_ref().map(|tools_val| {
            tools_val
                .iter()
                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                .collect()
        });

    let tool_choice: Option<aineer_webai::tool_calling::converter::ToolChoice> = req
        .tool_choice
        .as_ref()
        .and_then(|tc| serde_json::from_value(tc.clone()).ok());

    let result = engine
        .send_openai(
            provider_name,
            &model_id,
            &chat_messages,
            tools.as_deref(),
            tool_choice.as_ref(),
        )
        .await;

    let stream = req.stream.unwrap_or(false);
    let now = now_secs();

    match result {
        Ok(OpenAiStreamResult::Streaming(rx)) => {
            if stream {
                webai_stream_sse(rx, model, now).into_response()
            } else {
                webai_collect_response(rx, model, now).await.into_response()
            }
        }
        Ok(OpenAiStreamResult::Completed(parsed)) => {
            webai_parsed_response(parsed, model, now, stream).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("WebAI error: {e}"), "api_error")),
        )
            .into_response(),
    }
}

fn webai_stream_sse(
    mut rx: tokio::sync::mpsc::Receiver<String>,
    model: &str,
    now: u64,
) -> Sse<impl futures_core::Stream<Item = Result<Event, anyhow::Error>>> {
    let model = model.to_string();
    let sse_stream = async_stream::stream! {
        let id = format!("chatcmpl-webai-{now}");

        let initial = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".into(),
            created: now,
            model: model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatDelta { role: Some("assistant".into()), content: None },
                finish_reason: None,
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&initial).unwrap_or_default()));

        while let Some(chunk) = rx.recv().await {
            let c = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".into(),
                created: now,
                model: model.clone(),
                choices: vec![ChatChunkChoice {
                    index: 0,
                    delta: ChatDelta { role: None, content: Some(chunk) },
                    finish_reason: None,
                }],
            };
            yield Ok(Event::default().data(serde_json::to_string(&c).unwrap_or_default()));
        }

        let final_c = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".into(),
            created: now,
            model: model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatDelta { role: None, content: None },
                finish_reason: Some("stop".into()),
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&final_c).unwrap_or_default()));
        yield Ok(Event::default().data("[DONE]"));
    };

    Sse::new(sse_stream).keep_alive(KeepAlive::default())
}

async fn webai_collect_response(
    mut rx: tokio::sync::mpsc::Receiver<String>,
    model: &str,
    now: u64,
) -> Json<ChatCompletionResponse> {
    let mut text = String::new();
    while let Some(chunk) = rx.recv().await {
        text.push_str(&chunk);
    }
    let tokens = (text.len() as u32).div_ceil(4);
    Json(ChatCompletionResponse {
        id: format!("chatcmpl-webai-{now}"),
        object: "chat.completion".into(),
        created: now,
        model: model.to_string(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".into(),
                content: Some(serde_json::Value::String(text)),
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(UsageInfo {
            prompt_tokens: 0,
            completion_tokens: tokens,
            total_tokens: tokens,
        }),
    })
}

fn webai_parsed_response(
    parsed: aineer_webai::tool_calling::converter::ParsedResponse,
    model: &str,
    now: u64,
    stream: bool,
) -> axum::response::Response {
    let model = model.to_string();
    if stream {
        let sse_stream = async_stream::stream! {
            let id = format!("chatcmpl-webai-{now}");

            let initial = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".into(),
                created: now,
                model: model.clone(),
                choices: vec![ChatChunkChoice {
                    index: 0,
                    delta: ChatDelta { role: Some("assistant".into()), content: None },
                    finish_reason: None,
                }],
            };
            yield Ok::<_, anyhow::Error>(Event::default().data(serde_json::to_string(&initial).unwrap_or_default()));

            if let Some(ref content) = parsed.content {
                let c = ChatCompletionChunk {
                    id: id.clone(),
                    object: "chat.completion.chunk".into(),
                    created: now,
                    model: model.clone(),
                    choices: vec![ChatChunkChoice {
                        index: 0,
                        delta: ChatDelta { role: None, content: Some(content.clone()) },
                        finish_reason: None,
                    }],
                };
                yield Ok(Event::default().data(serde_json::to_string(&c).unwrap_or_default()));
            }

            let final_c = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".into(),
                created: now,
                model: model.clone(),
                choices: vec![ChatChunkChoice {
                    index: 0,
                    delta: ChatDelta { role: None, content: None },
                    finish_reason: Some(parsed.finish_reason.clone()),
                }],
            };
            yield Ok(Event::default().data(serde_json::to_string(&final_c).unwrap_or_default()));
            yield Ok(Event::default().data("[DONE]"));
        };
        Sse::new(sse_stream)
            .keep_alive(KeepAlive::default())
            .into_response()
    } else {
        let content_val = parsed
            .content
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        Json(ChatCompletionResponse {
            id: format!("chatcmpl-webai-{now}"),
            object: "chat.completion".into(),
            created: now,
            model,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: Some(content_val),
                },
                finish_reason: Some(parsed.finish_reason),
            }],
            usage: Some(UsageInfo {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }),
        })
        .into_response()
    }
}
