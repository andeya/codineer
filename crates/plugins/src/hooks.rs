use runtime::{HookCommandSource, HookRunResult, HookRunner};

use crate::{PluginError, PluginHooks, PluginRegistry};

impl HookCommandSource for PluginHooks {
    fn pre_tool_use_commands(&self) -> &[String] {
        &self.pre_tool_use
    }

    fn post_tool_use_commands(&self) -> &[String] {
        &self.post_tool_use
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookRunner {
    inner: HookRunner<PluginHooks>,
}

impl PluginHookRunner {
    #[must_use]
    pub fn new(source: PluginHooks) -> Self {
        Self {
            inner: HookRunner::new(source),
        }
    }

    pub fn from_registry(registry: &PluginRegistry) -> Result<Self, PluginError> {
        Ok(Self::new(registry.aggregated_hooks()?))
    }

    #[must_use]
    pub fn run_pre_tool_use(&self, tool_name: &str, tool_input: &str) -> HookRunResult {
        self.inner.run_pre_tool_use(tool_name, tool_input)
    }

    #[must_use]
    pub fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &str,
        tool_output: &str,
        is_error: bool,
    ) -> HookRunResult {
        self.inner
            .run_post_tool_use(tool_name, tool_input, tool_output, is_error)
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::PluginHookRunner;
    use crate::{PluginManager, PluginManagerConfig};
    use runtime::HookRunResult;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("plugins-hook-runner-{label}-{nanos}"))
    }

    fn write_hook_plugin(root: &Path, name: &str, pre_message: &str, post_message: &str) {
        fs::create_dir_all(root.join(".codineer-plugin")).expect("manifest dir");
        fs::create_dir_all(root.join("hooks")).expect("hooks dir");
        fs::write(
            root.join("hooks").join("pre.sh"),
            format!("#!/bin/sh\nprintf '%s\\n' '{pre_message}'\n"),
        )
        .expect("write pre hook");
        fs::write(
            root.join("hooks").join("post.sh"),
            format!("#!/bin/sh\nprintf '%s\\n' '{post_message}'\n"),
        )
        .expect("write post hook");
        fs::write(
            root.join(".codineer-plugin").join("plugin.json"),
            format!(
                "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"description\": \"hook plugin\",\n  \"hooks\": {{\n    \"PreToolUse\": [\"./hooks/pre.sh\"],\n    \"PostToolUse\": [\"./hooks/post.sh\"]\n  }}\n}}"
            ),
        )
        .expect("write plugin manifest");
    }

    #[test]
    fn collects_and_runs_hooks_from_enabled_plugins() {
        let config_home = temp_dir("config");
        let first_source_root = temp_dir("source-a");
        let second_source_root = temp_dir("source-b");
        write_hook_plugin(
            &first_source_root,
            "first",
            "plugin pre one",
            "plugin post one",
        );
        write_hook_plugin(
            &second_source_root,
            "second",
            "plugin pre two",
            "plugin post two",
        );

        let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
        manager
            .install(first_source_root.to_str().expect("utf8 path"))
            .expect("first plugin install should succeed");
        manager
            .install(second_source_root.to_str().expect("utf8 path"))
            .expect("second plugin install should succeed");
        let registry = manager.plugin_registry().expect("registry should build");

        let runner = PluginHookRunner::from_registry(&registry).expect("plugin hooks should load");

        assert_eq!(
            runner.run_pre_tool_use("Read", r#"{"path":"README.md"}"#),
            HookRunResult::allow(vec![
                "plugin pre one".to_string(),
                "plugin pre two".to_string(),
            ])
        );
        assert_eq!(
            runner.run_post_tool_use("Read", r#"{"path":"README.md"}"#, "ok", false),
            HookRunResult::allow(vec![
                "plugin post one".to_string(),
                "plugin post two".to_string(),
            ])
        );

        let _ = fs::remove_dir_all(config_home);
        let _ = fs::remove_dir_all(first_source_root);
        let _ = fs::remove_dir_all(second_source_root);
    }

    #[test]
    fn pre_tool_use_denies_when_plugin_hook_exits_two() {
        let runner = PluginHookRunner::new(crate::PluginHooks {
            pre_tool_use: vec!["printf 'blocked by plugin'; exit 2".to_string()],
            post_tool_use: Vec::new(),
        });

        let result = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);

        assert!(result.is_denied());
        assert_eq!(result.messages(), &["blocked by plugin".to_string()]);
    }
}
