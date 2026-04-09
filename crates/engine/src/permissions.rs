use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,
    Allow,
}

impl PermissionMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
            Self::Prompt => "prompt",
            Self::Allow => "allow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub input: String,
    pub current_mode: PermissionMode,
    pub required_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionPromptDecision {
    Allow,
    Deny { reason: String },
}

pub trait PermissionPrompter {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision;
}

/// A single permission rule that matches tool invocations by name and optional input pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name glob pattern (e.g., "bash", "mcp__*", "write_file")
    pub tool_pattern: String,
    /// Optional input content pattern (e.g., path glob for file tools, command pattern for bash)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_pattern: Option<String>,
    /// The decision for matching invocations
    pub decision: RuleDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDecision {
    AlwaysAllow,
    AlwaysDeny,
    AlwaysAsk,
}

/// Returns true if `pattern` matches `text` using only `*` as a wildcard for any substring.
#[must_use]
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    glob_matches_bytes(pattern.as_bytes(), text.as_bytes())
}

fn glob_matches_bytes(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.first().copied() {
        None => text.is_empty(),
        Some(b'*') => {
            glob_matches_bytes(&pattern[1..], text)
                || (!text.is_empty() && glob_matches_bytes(pattern, &text[1..]))
        }
        Some(ch) => {
            if text.first().is_some_and(|t| *t == ch) {
                glob_matches_bytes(&pattern[1..], &text[1..])
            } else {
                false
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allow,
    Deny { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_requirements: BTreeMap<String, PermissionMode>,
    rules: Vec<PermissionRule>,
}

impl PermissionPolicy {
    #[must_use]
    pub fn new(active_mode: PermissionMode) -> Self {
        Self {
            active_mode,
            tool_requirements: BTreeMap::new(),
            rules: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_rules(mut self, rules: Vec<PermissionRule>) -> Self {
        self.rules = rules;
        self
    }

    fn match_rule(&self, tool_name: &str, input: &str) -> Option<&PermissionRule> {
        self.rules.iter().find(|rule| {
            glob_matches(&rule.tool_pattern, tool_name)
                && rule
                    .input_pattern
                    .as_ref()
                    .is_none_or(|p| glob_matches(p, input))
        })
    }

    #[must_use]
    pub fn with_tool_requirement(
        mut self,
        tool_name: impl Into<String>,
        required_mode: PermissionMode,
    ) -> Self {
        self.tool_requirements
            .insert(tool_name.into(), required_mode);
        self
    }

    #[must_use]
    pub fn active_mode(&self) -> PermissionMode {
        self.active_mode
    }

    #[must_use]
    pub fn required_mode_for(&self, tool_name: &str) -> PermissionMode {
        self.tool_requirements
            .get(tool_name)
            .copied()
            .unwrap_or(PermissionMode::Prompt)
    }

    #[must_use]
    pub fn authorize(
        &self,
        tool_name: &str,
        input: &str,
        mut prompter: Option<&mut dyn PermissionPrompter>,
    ) -> PermissionOutcome {
        if let Some(rule) = self.match_rule(tool_name, input) {
            return match rule.decision {
                RuleDecision::AlwaysAllow => PermissionOutcome::Allow,
                RuleDecision::AlwaysDeny => PermissionOutcome::Deny {
                    reason: format!("denied by rule: {}", rule.tool_pattern),
                },
                RuleDecision::AlwaysAsk => {
                    let current_mode = self.active_mode();
                    let required_mode = self.required_mode_for(tool_name);
                    let request = PermissionRequest {
                        tool_name: tool_name.to_string(),
                        input: input.to_string(),
                        current_mode,
                        required_mode,
                    };
                    match prompter.as_mut() {
                        Some(prompter) => match prompter.decide(&request) {
                            PermissionPromptDecision::Allow => PermissionOutcome::Allow,
                            PermissionPromptDecision::Deny { reason } => {
                                PermissionOutcome::Deny { reason }
                            }
                        },
                        None => PermissionOutcome::Deny {
                            reason: format!(
                                "tool '{tool_name}' requires approval (permission rule: {})",
                                rule.tool_pattern
                            ),
                        },
                    }
                }
            };
        }

        let current_mode = self.active_mode();
        let required_mode = self.required_mode_for(tool_name);

        if current_mode == PermissionMode::Allow {
            return PermissionOutcome::Allow;
        }

        if current_mode == PermissionMode::Prompt {
            let request = PermissionRequest {
                tool_name: tool_name.to_string(),
                input: input.to_string(),
                current_mode,
                required_mode,
            };
            return match prompter.as_mut() {
                Some(prompter) => match prompter.decide(&request) {
                    PermissionPromptDecision::Allow => PermissionOutcome::Allow,
                    PermissionPromptDecision::Deny { reason } => PermissionOutcome::Deny { reason },
                },
                None => PermissionOutcome::Deny {
                    reason: format!(
                        "tool '{tool_name}' requires approval to escalate from {} to {}",
                        current_mode.as_str(),
                        required_mode.as_str()
                    ),
                },
            };
        }

        if current_mode >= required_mode {
            return PermissionOutcome::Allow;
        }

        let needs_prompt = required_mode == PermissionMode::Prompt
            || (current_mode == PermissionMode::WorkspaceWrite
                && required_mode == PermissionMode::DangerFullAccess);

        let request = PermissionRequest {
            tool_name: tool_name.to_string(),
            input: input.to_string(),
            current_mode,
            required_mode,
        };

        if needs_prompt {
            return match prompter.as_mut() {
                Some(prompter) => match prompter.decide(&request) {
                    PermissionPromptDecision::Allow => PermissionOutcome::Allow,
                    PermissionPromptDecision::Deny { reason } => PermissionOutcome::Deny { reason },
                },
                None => PermissionOutcome::Deny {
                    reason: format!(
                        "tool '{tool_name}' requires approval to escalate from {} to {}",
                        current_mode.as_str(),
                        required_mode.as_str()
                    ),
                },
            };
        }

        PermissionOutcome::Deny {
            reason: format!(
                "tool '{tool_name}' requires {} permission; current mode is {}",
                required_mode.as_str(),
                current_mode.as_str()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        glob_matches, PermissionMode, PermissionOutcome, PermissionPolicy,
        PermissionPromptDecision, PermissionPrompter, PermissionRequest, PermissionRule,
        RuleDecision,
    };

    struct RecordingPrompter {
        seen: Vec<PermissionRequest>,
        allow: bool,
    }

    impl PermissionPrompter for RecordingPrompter {
        fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
            self.seen.push(request.clone());
            if self.allow {
                PermissionPromptDecision::Allow
            } else {
                PermissionPromptDecision::Deny {
                    reason: "not now".to_string(),
                }
            }
        }
    }

    #[test]
    fn allows_tools_when_active_mode_meets_requirement() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
            .with_tool_requirement("read_file", PermissionMode::ReadOnly)
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite);

        assert_eq!(
            policy.authorize("read_file", "{}", None),
            PermissionOutcome::Allow
        );
        assert_eq!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Allow
        );
    }

    #[test]
    fn denies_read_only_escalations_without_prompt() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite)
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);

        assert!(matches!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Deny { reason } if reason.contains("requires workspace-write permission")
        ));
        assert!(matches!(
            policy.authorize("bash", "{}", None),
            PermissionOutcome::Deny { reason } if reason.contains("requires danger-full-access permission")
        ));
    }

    #[test]
    fn prompts_for_workspace_write_to_danger_full_access_escalation() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);
        let mut prompter = RecordingPrompter {
            seen: Vec::new(),
            allow: true,
        };

        let outcome = policy.authorize("bash", "echo hi", Some(&mut prompter));

        assert_eq!(outcome, PermissionOutcome::Allow);
        assert_eq!(prompter.seen.len(), 1);
        assert_eq!(prompter.seen[0].tool_name, "bash");
        assert_eq!(
            prompter.seen[0].current_mode,
            PermissionMode::WorkspaceWrite
        );
        assert_eq!(
            prompter.seen[0].required_mode,
            PermissionMode::DangerFullAccess
        );
    }

    #[test]
    fn honors_prompt_rejection_reason() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);
        let mut prompter = RecordingPrompter {
            seen: Vec::new(),
            allow: false,
        };

        assert!(matches!(
            policy.authorize("bash", "echo hi", Some(&mut prompter)),
            PermissionOutcome::Deny { reason } if reason == "not now"
        ));
    }

    #[test]
    fn prompt_mode_always_prompts_for_dangerous_tools() {
        let policy = PermissionPolicy::new(PermissionMode::Prompt)
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);
        let mut prompter = RecordingPrompter {
            seen: Vec::new(),
            allow: true,
        };

        let outcome = policy.authorize("bash", "rm -rf /", Some(&mut prompter));
        assert_eq!(outcome, PermissionOutcome::Allow);
        assert_eq!(
            prompter.seen.len(),
            1,
            "Prompt mode must invoke the prompter"
        );
    }

    #[test]
    fn prompt_mode_prompts_for_read_only_tools_too() {
        let policy = PermissionPolicy::new(PermissionMode::Prompt)
            .with_tool_requirement("read_file", PermissionMode::ReadOnly);
        let mut prompter = RecordingPrompter {
            seen: Vec::new(),
            allow: true,
        };

        let outcome = policy.authorize("read_file", "{}", Some(&mut prompter));
        assert_eq!(outcome, PermissionOutcome::Allow);
        assert_eq!(
            prompter.seen.len(),
            1,
            "Prompt mode should prompt even for read-only tools"
        );
    }

    #[test]
    fn prompt_mode_denies_without_prompter() {
        let policy = PermissionPolicy::new(PermissionMode::Prompt)
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);

        assert!(matches!(
            policy.authorize("bash", "echo hi", None),
            PermissionOutcome::Deny { reason } if reason.contains("requires approval")
        ));
    }

    #[test]
    fn read_only_denies_write_tools() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite);

        assert!(matches!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Deny { .. }
        ));
    }

    #[test]
    fn glob_matches_star_and_exact() {
        assert!(glob_matches("", ""));
        assert!(!glob_matches("", "a"));
        assert!(glob_matches("*", ""));
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("foo", "foo"));
        assert!(!glob_matches("foo", "bar"));
        assert!(glob_matches("mcp__*", "mcp__server__tool"));
        assert!(glob_matches("*suffix", "mysuffix"));
        assert!(glob_matches("pre*", "prelude"));
        assert!(glob_matches("*mid*", "abc middle def"));
    }

    #[test]
    fn rule_exact_tool_name_always_allow() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![PermissionRule {
                tool_pattern: "write_file".to_string(),
                input_pattern: None,
                decision: RuleDecision::AlwaysAllow,
            }])
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite);

        assert_eq!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Allow
        );
    }

    #[test]
    fn rule_wildcard_tool_pattern() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![PermissionRule {
                tool_pattern: "mcp__*".to_string(),
                input_pattern: None,
                decision: RuleDecision::AlwaysAllow,
            }])
            .with_tool_requirement("mcp__x__y", PermissionMode::DangerFullAccess);

        assert_eq!(
            policy.authorize("mcp__x__y", "{}", None),
            PermissionOutcome::Allow
        );
    }

    #[test]
    fn rule_with_input_pattern() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![PermissionRule {
                tool_pattern: "write_file".to_string(),
                input_pattern: Some("/safe/*".to_string()),
                decision: RuleDecision::AlwaysAllow,
            }])
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite);

        assert_eq!(
            policy.authorize("write_file", "/safe/foo.txt", None),
            PermissionOutcome::Allow
        );
        assert!(matches!(
            policy.authorize("write_file", "/other/foo.txt", None),
            PermissionOutcome::Deny { .. }
        ));
    }

    #[test]
    fn rule_priority_first_match_wins() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![
                PermissionRule {
                    tool_pattern: "bash".to_string(),
                    input_pattern: None,
                    decision: RuleDecision::AlwaysDeny,
                },
                PermissionRule {
                    tool_pattern: "bash".to_string(),
                    input_pattern: None,
                    decision: RuleDecision::AlwaysAllow,
                },
            ])
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);

        assert!(matches!(
            policy.authorize("bash", "echo hi", None),
            PermissionOutcome::Deny { reason } if reason.contains("denied by rule")
        ));
    }

    #[test]
    fn fallback_to_mode_when_no_rule_matches() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![PermissionRule {
                tool_pattern: "other_tool".to_string(),
                input_pattern: None,
                decision: RuleDecision::AlwaysAllow,
            }])
            .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite);

        assert!(matches!(
            policy.authorize("write_file", "{}", None),
            PermissionOutcome::Deny { reason } if reason.contains("workspace-write")
        ));
    }

    #[test]
    fn always_ask_rule_delegates_to_prompter() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
            .with_rules(vec![PermissionRule {
                tool_pattern: "bash".to_string(),
                input_pattern: None,
                decision: RuleDecision::AlwaysAsk,
            }])
            .with_tool_requirement("bash", PermissionMode::DangerFullAccess);
        let mut prompter = RecordingPrompter {
            seen: Vec::new(),
            allow: true,
        };

        assert_eq!(
            policy.authorize("bash", "ls", Some(&mut prompter)),
            PermissionOutcome::Allow
        );
        assert_eq!(prompter.seen.len(), 1);
    }
}
