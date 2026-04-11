use std::io::Write;

use aineer_engine::{PermissionMode, PermissionPolicy, PermissionRule};
use aineer_tools::GlobalToolRegistry;

pub(crate) struct CliPermissionPrompter {
    current_mode: PermissionMode,
}

impl CliPermissionPrompter {
    pub(crate) fn new(current_mode: PermissionMode) -> Self {
        Self { current_mode }
    }
}

impl aineer_engine::PermissionPrompter for CliPermissionPrompter {
    fn decide(
        &mut self,
        request: &aineer_engine::PermissionRequest,
    ) -> aineer_engine::PermissionPromptDecision {
        println!();
        println!("Permission approval required");
        println!("  Tool             {}", request.tool_name);
        println!("  Current mode     {}", self.current_mode.as_str());
        println!("  Required mode    {}", request.required_mode.as_str());
        println!("  Input            {}", request.input);
        print!("Approve this tool call? [y/N]: ");
        let _ = std::io::stdout().flush();

        let mut response = String::new();
        match std::io::stdin().read_line(&mut response) {
            Ok(_) => {
                let normalized = response.trim().to_ascii_lowercase();
                if matches!(normalized.as_str(), "y" | "yes") {
                    aineer_engine::PermissionPromptDecision::Allow
                } else {
                    aineer_engine::PermissionPromptDecision::Deny {
                        reason: format!(
                            "tool '{}' denied by user approval prompt",
                            request.tool_name
                        ),
                    }
                }
            }
            Err(error) => aineer_engine::PermissionPromptDecision::Deny {
                reason: format!("permission approval failed: {error}"),
            },
        }
    }
}

pub(crate) fn permission_policy(
    mode: PermissionMode,
    tool_registry: &GlobalToolRegistry,
    rules: &[PermissionRule],
) -> PermissionPolicy {
    tool_registry.permission_specs(None).into_iter().fold(
        PermissionPolicy::new(mode).with_rules(rules.to_vec()),
        |policy, (name, required_permission)| {
            policy.with_tool_requirement(name, required_permission)
        },
    )
}
