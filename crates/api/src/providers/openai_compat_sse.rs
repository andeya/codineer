use serde::Deserialize;

use crate::error::ApiError;

#[derive(Debug, Default)]
pub(super) struct OpenAiSseParser {
    buffer: Vec<u8>,
}

impl OpenAiSseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<ChatCompletionChunk>, ApiError> {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > 16 * 1024 * 1024 {
            return Err(ApiError::ResponsePayloadTooLarge {
                limit: 16 * 1024 * 1024,
            });
        }
        let mut events = Vec::new();

        while let Some(frame) = next_sse_frame(&mut self.buffer) {
            if let Some(event) = parse_sse_frame(&frame)? {
                events.push(event);
            }
        }

        Ok(events)
    }
}

pub(super) fn next_sse_frame(buffer: &mut Vec<u8>) -> Option<String> {
    let separator = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2))
        .or_else(|| {
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| (position, 4))
        })?;

    let (position, separator_len) = separator;
    let frame = buffer.drain(..position + separator_len).collect::<Vec<_>>();
    let frame_len = frame.len().saturating_sub(separator_len);
    Some(String::from_utf8_lossy(&frame[..frame_len]).into_owned())
}

pub(crate) fn parse_sse_frame(frame: &str) -> Result<Option<ChatCompletionChunk>, ApiError> {
    let trimmed = frame.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut data_lines = Vec::new();
    for line in trimmed.lines() {
        if line.starts_with(':') {
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }
    if data_lines.is_empty() {
        return Ok(None);
    }
    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(None);
    }

    // Detect upstream error events embedded in the SSE stream before
    // attempting to deserialize as ChatCompletionChunk.
    if let Ok(err) = serde_json::from_str::<SseErrorEnvelope>(&payload) {
        if let Some(inner) = err.error {
            let msg = inner
                .message
                .unwrap_or_else(|| "upstream error".to_string());
            return Err(ApiError::StreamApplicationError {
                error_type: inner.error_type,
                message: msg,
            });
        }
    }

    serde_json::from_str(&payload)
        .map(Some)
        .map_err(ApiError::from)
}

#[derive(Deserialize)]
struct SseErrorEnvelope {
    error: Option<SseErrorBody>,
}

#[derive(Deserialize)]
struct SseErrorBody {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ChatCompletionChunk {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<super::OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ChunkChoice {
    #[serde(default)]
    pub delta: ChunkDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ChunkDelta {
    #[serde(default, deserialize_with = "super::deserialize_openai_text_content")]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub thought: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<DeltaToolCall>,
}

impl ChunkDelta {
    pub fn stream_text_fragment(&self) -> Option<String> {
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
pub(super) struct DeltaToolCall {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: DeltaFunction,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct DeltaFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

/// Shared logic: pick the first non-empty optional string from a list of fields.
pub(super) fn first_non_empty_field(fields: &[&Option<String>]) -> Option<String> {
    fields
        .iter()
        .find_map(|f| f.as_ref().filter(|s| !s.is_empty()).cloned())
}
