use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use aineer_api::{
    ContentBlockDelta, MessageResponse, OutputContentBlock, ProviderClient,
    StreamEvent as ApiStreamEvent,
};
use aineer_engine::{AssistantEvent, RuntimeError, TokenUsage};

use crate::progress::InternalPromptProgressReporter;
use crate::render::{MarkdownStreamState, TerminalRenderer};
use crate::tool_display::format_tool_call_start;

/// Typed delta emitted during streaming — allows callers to distinguish
/// model reasoning (thinking) from formal assistant output (text).
#[derive(Debug, Clone, Copy)]
pub enum StreamDelta<'a> {
    Text(&'a str),
    Thinking(&'a str),
}

/// Owned copy of [`StreamDelta`] for desktop hooks (e.g. Tauri emit).
#[derive(Debug, Clone)]
pub enum DesktopStreamDelta {
    Text(String),
    Thinking(String),
}

pub(crate) struct StreamState {
    renderer: TerminalRenderer,
    markdown_stream: MarkdownStreamState,
    events: Vec<AssistantEvent>,
    pending_tool: Option<(String, String, String)>,
    saw_stop: bool,
}

impl StreamState {
    pub(super) fn new() -> Self {
        Self {
            renderer: TerminalRenderer::new(),
            markdown_stream: MarkdownStreamState::default(),
            events: Vec::new(),
            pending_tool: None,
            saw_stop: false,
        }
    }

    pub(super) fn handle_event(
        &mut self,
        event: ApiStreamEvent,
        progress: Option<&InternalPromptProgressReporter>,
        out: &mut dyn Write,
        on_delta: &mut dyn FnMut(StreamDelta<'_>),
    ) -> Result<(), RuntimeError> {
        match event {
            ApiStreamEvent::MessageStart(start) => {
                for block in start.message.content {
                    push_output_block(
                        block,
                        out,
                        &mut self.events,
                        &mut self.pending_tool,
                        true,
                        on_delta,
                    )?;
                }
            }
            ApiStreamEvent::ContentBlockStart(start) => {
                push_output_block(
                    start.content_block,
                    out,
                    &mut self.events,
                    &mut self.pending_tool,
                    true,
                    on_delta,
                )?;
            }
            ApiStreamEvent::ContentBlockDelta(delta) => match delta.delta {
                ContentBlockDelta::TextDelta { text } if !text.is_empty() => {
                    if let Some(reporter) = progress {
                        reporter.mark_text_phase(&text);
                    }
                    if let Some(rendered) = self.markdown_stream.push(&self.renderer, &text) {
                        write_flush(out, &rendered)?;
                    }
                    on_delta(StreamDelta::Text(&text));
                    self.events.push(AssistantEvent::TextDelta(text));
                }
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    if let Some((_, _, input)) = &mut self.pending_tool {
                        input.push_str(&partial_json);
                    }
                }
                ContentBlockDelta::ThinkingDelta { thinking } if !thinking.is_empty() => {
                    if let Some(reporter) = progress {
                        reporter.mark_text_phase(&thinking);
                    }
                    if let Some(rendered) = self.markdown_stream.push(&self.renderer, &thinking) {
                        write_flush(out, &rendered)?;
                    }
                    on_delta(StreamDelta::Thinking(&thinking));
                    self.events.push(AssistantEvent::ThinkingDelta(thinking));
                }
                ContentBlockDelta::SignatureDelta { .. } => {}
                _ => {}
            },
            ApiStreamEvent::ContentBlockStop(_) => {
                if let Some(rendered) = self.markdown_stream.flush(&self.renderer) {
                    write_flush(out, &rendered)?;
                }
                if let Some((id, name, input)) = self.pending_tool.take() {
                    if let Some(reporter) = progress {
                        reporter.mark_tool_phase(&name, &input);
                    }
                    let display = format!("\n{}", format_tool_call_start(&name, &input));
                    writeln!(out, "{display}")
                        .and_then(|()| out.flush())
                        .map_err(|error| RuntimeError::new(error.to_string()))?;
                    self.events
                        .push(AssistantEvent::ToolUse { id, name, input });
                }
            }
            ApiStreamEvent::MessageDelta(delta) => {
                self.events.push(AssistantEvent::Usage(TokenUsage {
                    input_tokens: delta.usage.input_tokens,
                    output_tokens: delta.usage.output_tokens,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }));
            }
            ApiStreamEvent::MessageStop(_) => {
                self.saw_stop = true;
                if let Some(rendered) = self.markdown_stream.flush(&self.renderer) {
                    write_flush(out, &rendered)?;
                }
                self.events.push(AssistantEvent::MessageStop);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn ensure_stop_event(mut self) -> Vec<AssistantEvent> {
        if !self.saw_stop
            && self.events.iter().any(|event| {
                matches!(event, AssistantEvent::TextDelta(text) if !text.is_empty())
                    || matches!(event, AssistantEvent::ThinkingDelta(text) if !text.is_empty())
                    || matches!(event, AssistantEvent::ToolUse { .. })
            })
        {
            self.events.push(AssistantEvent::MessageStop);
        }
        self.events
    }
}

pub(crate) fn write_flush(out: &mut dyn Write, buf: &str) -> Result<(), RuntimeError> {
    write!(out, "{buf}")
        .and_then(|()| out.flush())
        .map_err(|error| RuntimeError::new(error.to_string()))
}

pub(super) async fn stream_with_client(
    client: &ProviderClient,
    message_request: &aineer_api::MessageRequest,
    emit_output: bool,
    progress: Option<&InternalPromptProgressReporter>,
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    let mut stream = client
        .stream_message(message_request)
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?;
    let (gf, gc) = crate::render::gutter_prefixes();
    let mut gutter = crate::render::GutterWriter::new(std::io::stdout(), gf, gc);
    let mut sink = std::io::sink();
    let out: &mut dyn Write = if emit_output { &mut gutter } else { &mut sink };
    let mut state = StreamState::new();
    let mut noop_cb = |_: StreamDelta<'_>| {};
    while let Some(event) = stream
        .next_event()
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?
    {
        state.handle_event(event, progress, out, &mut noop_cb)?;
    }

    let events = state.ensure_stop_event();
    let has_body = events.iter().any(|event| {
        matches!(event, AssistantEvent::TextDelta(t) if !t.is_empty())
            || matches!(event, AssistantEvent::ThinkingDelta(t) if !t.is_empty())
            || matches!(event, AssistantEvent::ToolUse { .. })
    });

    if has_body {
        return Ok(events);
    }

    let response = client
        .send_message(&aineer_api::MessageRequest {
            stream: false,
            thinking: None,
            ..message_request.clone()
        })
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?;
    response_to_events(response, out, &mut noop_cb)
}

/// Stream from `client`, invoking `on_delta` for each assistant text / thinking chunk.
/// Stops reading when `cancel` is set; partial events are still returned.
pub(crate) async fn stream_with_client_deltas<F>(
    client: &ProviderClient,
    message_request: &aineer_api::MessageRequest,
    cancel: Arc<AtomicBool>,
    mut on_delta: F,
) -> Result<Vec<AssistantEvent>, RuntimeError>
where
    F: FnMut(StreamDelta<'_>),
{
    let mut stream = client
        .stream_message(message_request)
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?;
    let mut sink = std::io::sink();
    let out: &mut dyn Write = &mut sink;
    let mut state = StreamState::new();
    while let Some(event) = stream
        .next_event()
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        state.handle_event(event, None, out, &mut on_delta)?;
    }

    let events = state.ensure_stop_event();
    let has_body = events.iter().any(|event| {
        matches!(event, AssistantEvent::TextDelta(t) if !t.is_empty())
            || matches!(event, AssistantEvent::ThinkingDelta(t) if !t.is_empty())
            || matches!(event, AssistantEvent::ToolUse { .. })
    });

    if has_body {
        return Ok(events);
    }

    // Fallback: non-streaming request. `on_delta` is intentionally passed here
    // because this path only runs when streaming produced no content.
    let response = client
        .send_message(&aineer_api::MessageRequest {
            stream: false,
            thinking: None,
            ..message_request.clone()
        })
        .await
        .map_err(aineer_api::ApiError::into_runtime_error)?;
    response_to_events(response, out, &mut on_delta)
}

pub(crate) fn push_output_block(
    block: OutputContentBlock,
    out: &mut (impl Write + ?Sized),
    events: &mut Vec<AssistantEvent>,
    pending_tool: &mut Option<(String, String, String)>,
    streaming_tool_input: bool,
    on_delta: &mut dyn FnMut(StreamDelta<'_>),
) -> Result<(), RuntimeError> {
    match block {
        OutputContentBlock::Text { text } if !text.is_empty() => {
            let rendered = TerminalRenderer::new().render_markdown(&text);
            write!(out, "{rendered}")
                .and_then(|()| out.flush())
                .map_err(|error| RuntimeError::new(error.to_string()))?;
            on_delta(StreamDelta::Text(&text));
            events.push(AssistantEvent::TextDelta(text));
        }
        OutputContentBlock::ToolUse { id, name, input } => {
            let initial_input = if streaming_tool_input
                && input.as_object().is_some_and(serde_json::Map::is_empty)
            {
                String::new()
            } else {
                input.to_string()
            };
            *pending_tool = Some((id, name, initial_input));
        }
        OutputContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
            let rendered = TerminalRenderer::new().render_markdown(&thinking);
            write!(out, "{rendered}")
                .and_then(|()| out.flush())
                .map_err(|error| RuntimeError::new(error.to_string()))?;
            on_delta(StreamDelta::Thinking(&thinking));
            events.push(AssistantEvent::ThinkingDelta(thinking));
        }
        OutputContentBlock::RedactedThinking { .. } => {}
        _ => {}
    }
    Ok(())
}

pub(crate) fn response_to_events(
    response: MessageResponse,
    out: &mut (impl Write + ?Sized),
    on_delta: &mut dyn FnMut(StreamDelta<'_>),
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    let mut events = Vec::new();
    let mut pending_tool = None;

    for block in response.content {
        push_output_block(block, out, &mut events, &mut pending_tool, false, on_delta)?;
        if let Some((id, name, input)) = pending_tool.take() {
            events.push(AssistantEvent::ToolUse { id, name, input });
        }
    }

    events.push(AssistantEvent::Usage(TokenUsage {
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
        cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
        cache_read_input_tokens: response.usage.cache_read_input_tokens,
    }));
    events.push(AssistantEvent::MessageStop);
    Ok(events)
}
