use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<InputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
}

impl MessageRequest {
    #[must_use]
    pub fn with_streaming(mut self) -> Self {
        self.stream = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMessage {
    pub role: String,
    pub content: Vec<InputContentBlock>,
}

impl InputMessage {
    #[must_use]
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text { text: text.into() }],
        }
    }

    #[must_use]
    pub fn user_tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![InputContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: vec![ToolResultContentBlock::Text {
                    text: content.into(),
                }],
                is_error,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<ToolResultContentBlock>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    Text { text: String },
    Json { value: Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub content: Vec<OutputContentBlock>,
    pub model: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
    #[serde(default)]
    pub request_id: Option<String>,
}

impl MessageResponse {
    #[must_use]
    pub fn total_tokens(&self) -> u32 {
        self.usage.total_tokens()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    RedactedThinking {
        data: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    #[must_use]
    pub const fn total_tokens(&self) -> u32 {
        self.input_tokens
            + self.output_tokens
            + self.cache_creation_input_tokens
            + self.cache_read_input_tokens
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageStartEvent {
    pub message: MessageResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDeltaEvent {
    pub delta: MessageDelta,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDelta {
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentBlockStartEvent {
    pub index: u32,
    pub content_block: OutputContentBlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentBlockDeltaEvent {
    pub index: u32,
    pub delta: ContentBlockDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBlockStopEvent {
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageStopEvent {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart(MessageStartEvent),
    MessageDelta(MessageDeltaEvent),
    ContentBlockStart(ContentBlockStartEvent),
    ContentBlockDelta(ContentBlockDeltaEvent),
    ContentBlockStop(ContentBlockStopEvent),
    MessageStop(MessageStopEvent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_request_serializes_with_streaming_and_tools() {
        let request = MessageRequest {
            model: "claude-opus-4-6".to_string(),
            max_tokens: 4096,
            messages: vec![InputMessage::user_text("hello")],
            system: Some("you are helpful".to_string()),
            tools: Some(vec![ToolDefinition {
                name: "read_file".to_string(),
                description: Some("Read a file".to_string()),
                input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }]),
            tool_choice: Some(ToolChoice::Auto),
            stream: false,
        };
        let json = serde_json::to_string(&request).expect("serialize");
        let deserialized: MessageRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, request);
        assert!(!json.contains("\"stream\""));

        let streaming = request.with_streaming();
        let json = serde_json::to_string(&streaming).expect("serialize streaming");
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn input_content_block_round_trips_all_variants() {
        let text = InputContentBlock::Text {
            text: "hello".to_string(),
        };
        let tool_use = InputContentBlock::ToolUse {
            id: "t1".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "ls"}),
        };
        let tool_result = InputContentBlock::ToolResult {
            tool_use_id: "t1".to_string(),
            content: vec![ToolResultContentBlock::Text {
                text: "output".to_string(),
            }],
            is_error: false,
        };
        for block in [text, tool_use, tool_result] {
            let json = serde_json::to_value(&block).expect("serialize");
            let deserialized: InputContentBlock =
                serde_json::from_value(json).expect("deserialize");
            assert_eq!(deserialized, block);
        }
    }

    #[test]
    fn message_response_deserializes_with_defaults() {
        let json = json!({
            "id": "msg-1",
            "type": "message",
            "model": "claude-opus-4-6",
            "role": "assistant",
            "content": [{"type": "text", "text": "hi"}],
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        let response: MessageResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.id, "msg-1");
        assert_eq!(response.role, "assistant");
        assert_eq!(response.usage.total_tokens(), 15);
    }

    #[test]
    fn output_content_block_round_trips_including_thinking() {
        let blocks = vec![
            OutputContentBlock::Text {
                text: "hello".to_string(),
            },
            OutputContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "bash".to_string(),
                input: json!({"command": "ls"}),
            },
            OutputContentBlock::Thinking {
                thinking: "hmm".to_string(),
                signature: Some("sig".to_string()),
            },
        ];
        for block in blocks {
            let json = serde_json::to_value(&block).expect("serialize");
            let deserialized: OutputContentBlock =
                serde_json::from_value(json).expect("deserialize");
            assert_eq!(deserialized, block);
        }
    }

    #[test]
    fn tool_choice_variants_serialize_with_type_tag() {
        let auto_json = serde_json::to_value(ToolChoice::Auto).expect("auto");
        assert_eq!(auto_json, json!({"type": "auto"}));
        let any_json = serde_json::to_value(ToolChoice::Any).expect("any");
        assert_eq!(any_json, json!({"type": "any"}));
        let tool_json = serde_json::to_value(ToolChoice::Tool {
            name: "bash".to_string(),
        })
        .expect("tool");
        assert_eq!(tool_json, json!({"type": "tool", "name": "bash"}));
    }

    #[test]
    fn stream_event_deserializes_all_variants() {
        let msg_start = json!({
            "type": "message_start",
            "message": {
                "id": "msg-1", "type": "message", "model": "m",
                "role": "assistant", "content": [],
                "usage": {"input_tokens": 0, "output_tokens": 0}
            }
        });
        let parsed: StreamEvent = serde_json::from_value(msg_start).expect("message_start");
        assert!(matches!(parsed, StreamEvent::MessageStart(_)));

        let delta = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "hi"}
        });
        let parsed: StreamEvent = serde_json::from_value(delta).expect("content_block_delta");
        assert!(matches!(parsed, StreamEvent::ContentBlockDelta(_)));
    }

    #[test]
    fn usage_computes_total_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 20,
            cache_creation_input_tokens: 10,
        };
        assert_eq!(usage.total_tokens(), 180);
    }

    #[test]
    fn content_block_delta_all_variants_round_trip() {
        let deltas = vec![
            ContentBlockDelta::TextDelta {
                text: "hi".to_string(),
            },
            ContentBlockDelta::InputJsonDelta {
                partial_json: "{\"a\"".to_string(),
            },
            ContentBlockDelta::ThinkingDelta {
                thinking: "hmm".to_string(),
            },
            ContentBlockDelta::SignatureDelta {
                signature: "sig".to_string(),
            },
        ];
        for delta in deltas {
            let json = serde_json::to_value(&delta).expect("serialize");
            let deserialized: ContentBlockDelta =
                serde_json::from_value(json).expect("deserialize");
            assert_eq!(deserialized, delta);
        }
    }
}
