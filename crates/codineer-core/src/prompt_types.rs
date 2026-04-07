use serde::{Deserialize, Serialize};

// ── Prompt caching ──────────────────────────────────────────────────

/// Anthropic `cache_control` block attached to system blocks or tool definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub kind: CacheType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<CacheScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheType {
    Ephemeral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheScope {
    Global,
}

impl CacheControl {
    #[must_use]
    pub const fn ephemeral() -> Self {
        Self {
            kind: CacheType::Ephemeral,
            ttl: None,
            scope: None,
        }
    }

    #[must_use]
    pub fn global_1h() -> Self {
        Self {
            kind: CacheType::Ephemeral,
            ttl: Some("1h".to_string()),
            scope: Some(CacheScope::Global),
        }
    }
}

// ── System prompt blocks ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Text,
}

/// A single system prompt block sent to the Anthropic Messages API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub kind: BlockKind,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemBlock {
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            kind: BlockKind::Text,
            text: content.into(),
            cache_control: None,
        }
    }

    #[must_use]
    pub fn cached(content: impl Into<String>, cc: CacheControl) -> Self {
        Self {
            kind: BlockKind::Text,
            text: content.into(),
            cache_control: Some(cc),
        }
    }

    /// Build a system block list from a plain string (convenience for tests / simple callers).
    #[must_use]
    pub fn from_plain(text: impl Into<String>) -> Vec<Self> {
        vec![Self::text(text)]
    }
}

// ── Extended thinking ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub kind: ThinkingMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingMode {
    Enabled,
    Disabled,
}

impl ThinkingConfig {
    #[must_use]
    pub fn enabled(budget: u32) -> Self {
        Self {
            kind: ThinkingMode::Enabled,
            budget_tokens: Some(budget),
        }
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            kind: ThinkingMode::Disabled,
            budget_tokens: None,
        }
    }
}
