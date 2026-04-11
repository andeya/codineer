use aineer_api::{
    CacheControl, ImageSource, InputContentBlock, InputMessage, ToolResultContentBlock,
};
use aineer_engine::ConversationMessage;

/// Convert runtime messages to API input messages, placing a single
/// `cache_control: ephemeral` breakpoint on the last content block of the
/// last message. This lets the Anthropic API cache the entire conversation
/// prefix on subsequent turns.
pub(crate) fn convert_messages(messages: &[ConversationMessage]) -> Vec<InputMessage> {
    use aineer_engine::ContentBlock;
    let mut result: Vec<InputMessage> = messages
        .iter()
        .filter_map(|message| {
            let role = match message.role {
                aineer_engine::MessageRole::System
                | aineer_engine::MessageRole::User
                | aineer_engine::MessageRole::Tool => "user",
                aineer_engine::MessageRole::Assistant => "assistant",
                _ => "user",
            };
            let content = message
                .blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => InputContentBlock::Text {
                        text: text.clone(),
                        cache_control: None,
                    },
                    ContentBlock::Image { media_type, data } => InputContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".to_string(),
                            media_type: media_type.clone(),
                            data: data.clone(),
                        },
                    },
                    ContentBlock::ToolUse { id, name, input } => InputContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::from_str(input)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": input })),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        output,
                        is_error,
                        ..
                    } => InputContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: vec![ToolResultContentBlock::Text {
                            text: output.clone(),
                        }],
                        is_error: *is_error,
                        cache_control: None,
                    },
                    _ => InputContentBlock::Text {
                        text: String::new(),
                        cache_control: None,
                    },
                })
                .collect::<Vec<_>>();
            (!content.is_empty()).then(|| InputMessage {
                role: role.to_string(),
                content,
            })
        })
        .collect();

    add_cache_breakpoint(&mut result);
    result
}

/// Place a single `cache_control: ephemeral` on the last cacheable content
/// block of the last message (Text or ToolResult). This mirrors the Claude
/// Code strategy of exactly one message-level cache breakpoint.
fn add_cache_breakpoint(messages: &mut [InputMessage]) {
    if let Some(last_msg) = messages.last_mut() {
        for block in last_msg.content.iter_mut().rev() {
            match block {
                InputContentBlock::Text { cache_control, .. }
                | InputContentBlock::ToolResult { cache_control, .. } => {
                    *cache_control = Some(CacheControl::ephemeral());
                    return;
                }
                _ => {}
            }
        }
    }
}
