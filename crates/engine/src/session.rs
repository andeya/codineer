use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::json::{JsonError, JsonValue};
use crate::usage::TokenUsage;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        media_type: String,
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub blocks: Vec<ContentBlock>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub version: u32,
    pub messages: Vec<ConversationMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Session-scoped lock aligned with tool-definition cache TTL so mid-session
/// invalidation does not change serialized tool schemas and bust prompt cache.
#[derive(Debug, Clone)]
pub struct CacheLock {
    locked_at: Instant,
    ttl: Duration,
}

impl CacheLock {
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            locked_at: Instant::now(),
            ttl,
        }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.locked_at.elapsed() < self.ttl
    }

    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.ttl.saturating_sub(self.locked_at.elapsed())
    }
}

impl Default for CacheLock {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

#[non_exhaustive]
#[derive(Debug)]
pub enum SessionError {
    Io(std::io::Error),
    Json(JsonError),
    Format(String),
}

impl Display for SessionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<JsonError> for SessionError {
    fn from(value: JsonError) -> Self {
        Self::Json(value)
    }
}

impl Session {
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: 1,
            messages: Vec::new(),
            cwd: None,
            model_id: None,
            created_at: None,
        }
    }

    #[must_use]
    pub fn with_metadata(cwd: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            version: 1,
            messages: Vec::new(),
            cwd: Some(cwd.into()),
            model_id: Some(model_id.into()),
            created_at: None,
        }
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), SessionError> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| SessionError::Format(e.to_string()))?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(|e| SessionError::Format(e.to_string()))
    }

    #[must_use]
    pub fn to_json(&self) -> JsonValue {
        let json = serde_json::to_string(self).expect("Session should serialize");
        JsonValue::parse(&json).expect("serde JSON should parse as runtime JsonValue")
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, SessionError> {
        let s = value.render();
        serde_json::from_str(&s).map_err(|e| SessionError::Format(e.to_string()))
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationMessage {
    #[must_use]
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            blocks: vec![ContentBlock::Text { text: text.into() }],
            usage: None,
        }
    }

    #[must_use]
    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::User,
            blocks,
            usage: None,
        }
    }

    #[must_use]
    pub fn assistant(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::Assistant,
            blocks,
            usage: None,
        }
    }

    #[must_use]
    pub fn assistant_with_usage(blocks: Vec<ContentBlock>, usage: Option<TokenUsage>) -> Self {
        Self {
            role: MessageRole::Assistant,
            blocks,
            usage,
        }
    }

    #[must_use]
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        tool_name: impl Into<String>,
        output: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: MessageRole::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                tool_name: tool_name.into(),
                output: output.into(),
                is_error,
            }],
            usage: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentBlock, ConversationMessage, MessageRole, Session};
    use crate::usage::TokenUsage;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn persists_and_restores_session_json() {
        let mut session = Session::new();
        session
            .messages
            .push(ConversationMessage::user_text("hello"));
        session
            .messages
            .push(ConversationMessage::assistant_with_usage(
                vec![
                    ContentBlock::Text {
                        text: "thinking".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "tool-1".to_string(),
                        name: "bash".to_string(),
                        input: "echo hi".to_string(),
                    },
                ],
                Some(TokenUsage {
                    input_tokens: 10,
                    output_tokens: 4,
                    cache_creation_input_tokens: 1,
                    cache_read_input_tokens: 2,
                }),
            ));
        session.messages.push(ConversationMessage::tool_result(
            "tool-1", "bash", "hi", false,
        ));

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("runtime-session-{nanos}.json"));
        session.save_to_path(&path).expect("session should save");
        let restored = Session::load_from_path(&path).expect("session should load");
        fs::remove_file(&path).expect("temp file should be removable");

        assert_eq!(restored, session);
        assert_eq!(restored.messages[2].role, MessageRole::Tool);
        assert_eq!(
            restored.messages[1].usage.expect("usage").total_tokens(),
            17
        );
    }

    #[test]
    fn round_trips_system_role_message() {
        let json_str = r#"{"version":1,"messages":[{"role":"system","blocks":[{"type":"text","text":"sys prompt"}]}]}"#;
        let parsed = crate::json::JsonValue::parse(json_str).unwrap();
        let session = Session::from_json(&parsed).unwrap();
        assert_eq!(session.messages[0].role, MessageRole::System);

        let rendered = session.to_json();
        let restored = Session::from_json(&rendered).unwrap();
        assert_eq!(restored, session);
    }

    #[test]
    fn rejects_unsupported_message_role() {
        let json_str = r#"{"version":1,"messages":[{"role":"admin","blocks":[]}]}"#;
        let parsed = crate::json::JsonValue::parse(json_str).unwrap();
        let err = Session::from_json(&parsed).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("admin") || s.contains("unknown") || s.contains("invalid"),
            "unexpected error: {s}"
        );
    }

    #[test]
    fn rejects_unsupported_block_type() {
        let json_str =
            r#"{"version":1,"messages":[{"role":"user","blocks":[{"type":"video","url":"x"}]}]}"#;
        let parsed = crate::json::JsonValue::parse(json_str).unwrap();
        let err = Session::from_json(&parsed).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("video") || s.contains("unknown") || s.contains("invalid"),
            "unexpected error: {s}"
        );
    }

    #[test]
    fn rejects_missing_version() {
        let json_str = r#"{"messages":[]}"#;
        let parsed = crate::json::JsonValue::parse(json_str).unwrap();
        let err = Session::from_json(&parsed).unwrap_err();
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn rejects_non_object_root() {
        let parsed = crate::json::JsonValue::parse("[1,2]").unwrap();
        let err = Session::from_json(&parsed).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("invalid") || s.contains("expected") || s.contains("Session"),
            "unexpected error: {s}"
        );
    }

    #[test]
    fn load_from_nonexistent_path_returns_io_error() {
        let err = Session::load_from_path(Path::new("/nonexistent/path/session.json")).unwrap_err();
        assert!(matches!(err, super::SessionError::Io(_)));
    }

    #[test]
    fn session_error_display_covers_all_variants() {
        let io_err = super::SessionError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(io_err.to_string().contains("not found"));

        let json_err = super::SessionError::Json(crate::json::JsonError::new("bad"));
        assert!(json_err.to_string().contains("bad"));

        let fmt_err = super::SessionError::Format("bad format".into());
        assert!(fmt_err.to_string().contains("bad format"));
    }

    #[test]
    fn cache_lock_default_is_one_hour_and_starts_valid() {
        let lock = super::CacheLock::default();
        assert!(lock.remaining() <= Duration::from_secs(3600));
        assert!(lock.remaining() > Duration::from_secs(3590));
        assert!(lock.is_valid());
    }

    #[test]
    fn cache_lock_remaining_saturates_when_expired() {
        let lock = super::CacheLock::new(Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(200));
        assert!(!lock.is_valid());
        assert_eq!(lock.remaining(), Duration::ZERO);
    }

    #[test]
    fn cache_lock_zero_ttl_is_never_valid() {
        let lock = super::CacheLock::new(Duration::ZERO);
        assert!(!lock.is_valid());
        assert_eq!(lock.remaining(), Duration::ZERO);
    }

    #[test]
    fn with_metadata_sets_optional_fields() {
        let s = Session::with_metadata("/tmp/ws", "claude-3");
        assert_eq!(s.cwd.as_deref(), Some("/tmp/ws"));
        assert_eq!(s.model_id.as_deref(), Some("claude-3"));
        assert!(s.created_at.is_none());
    }

    #[test]
    fn metadata_round_trips_and_skips_none_in_json() {
        let mut session = Session::with_metadata("/project", "m1");
        session.created_at = Some("2026-04-08".to_string());
        session.messages.push(ConversationMessage::user_text("hi"));

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("runtime-session-meta-{nanos}.json"));
        session.save_to_path(&path).expect("save");
        let raw = fs::read_to_string(&path).expect("read");
        fs::remove_file(&path).expect("remove temp");

        assert!(raw.contains("cwd"));
        assert!(raw.contains("model_id"));
        assert!(raw.contains("created_at"));

        let restored = serde_json::from_str::<Session>(&raw).expect("serde round trip");
        assert_eq!(restored, session);
    }

    #[test]
    fn round_trips_image_content_block() {
        let mut session = Session::new();
        session.messages.push(ConversationMessage::user_blocks(vec![
            ContentBlock::Text {
                text: "describe this image".to_string(),
            },
            ContentBlock::Image {
                media_type: "image/png".to_string(),
                data: "iVBOR...".to_string(),
            },
        ]));

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("runtime-session-img-{nanos}.json"));
        session.save_to_path(&path).expect("session should save");
        let restored = Session::load_from_path(&path).expect("session should load");
        fs::remove_file(&path).expect("temp file should be removable");

        assert_eq!(restored, session);
        assert_eq!(restored.messages[0].blocks.len(), 2);
        match &restored.messages[0].blocks[1] {
            ContentBlock::Image { media_type, data } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "iVBOR...");
            }
            other => panic!("expected Image block, got: {other:?}"),
        }
    }
}
