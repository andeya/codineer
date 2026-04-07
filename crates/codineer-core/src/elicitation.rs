//! Elicitation types for structured user input collection.
//!
//! Enables the model to request structured information from the user
//! (e.g., selections, confirmations, form fields) without relying on
//! free-form text parsing.

use std::collections::BTreeMap;

/// Type-safe wrapper for elicitation request identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElicitationId(pub String);

/// Type-safe wrapper for option identifiers in selections.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptionId(pub String);

/// A request from the model to collect structured input from the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElicitationRequest {
    pub id: ElicitationId,
    pub kind: ElicitationKind,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

/// The kind of structured input to collect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElicitationKind {
    /// Yes/No confirmation.
    Confirm { default: bool },
    /// Single selection from a list of options.
    Select { options: Vec<ElicitationOption> },
    /// Multiple selection from a list of options.
    MultiSelect { options: Vec<ElicitationOption> },
    /// Free-form text input with optional validation.
    TextInput { placeholder: Option<String> },
    /// Secret input (e.g., API keys).
    SecretInput { placeholder: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElicitationOption {
    pub id: OptionId,
    pub label: String,
    pub description: Option<String>,
}

/// The user's response to an elicitation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElicitationResponse {
    Confirmed(bool),
    Selected(OptionId),
    MultiSelected(Vec<OptionId>),
    Text(String),
    Dismissed,
}

/// Trait for handling elicitation requests.
///
/// Implementations may present UI prompts, auto-respond in test mode, etc.
pub trait ElicitationHandler: Send {
    fn handle(&mut self, request: &ElicitationRequest) -> ElicitationResponse;
}

/// Auto-responder that always dismisses (for non-interactive contexts).
#[derive(Default)]
pub struct DismissHandler;

impl ElicitationHandler for DismissHandler {
    fn handle(&mut self, _request: &ElicitationRequest) -> ElicitationResponse {
        ElicitationResponse::Dismissed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismiss_handler_always_dismisses() {
        let mut handler = DismissHandler;
        let request = ElicitationRequest {
            id: ElicitationId("test".to_string()),
            kind: ElicitationKind::Confirm { default: true },
            message: "Continue?".to_string(),
            metadata: BTreeMap::new(),
        };
        assert_eq!(handler.handle(&request), ElicitationResponse::Dismissed);
    }

    #[test]
    fn elicitation_kind_debug() {
        let kind = ElicitationKind::Select {
            options: vec![ElicitationOption {
                id: OptionId("a".to_string()),
                label: "Option A".to_string(),
                description: None,
            }],
        };
        assert!(format!("{kind:?}").contains("Select"));
    }
}
