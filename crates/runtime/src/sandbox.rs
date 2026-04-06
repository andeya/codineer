use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemIsolationMode {
    Off,
    #[default]
    WorkspaceOnly,
    AllowList,
}

impl FilesystemIsolationMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::WorkspaceOnly => "workspace-only",
            Self::AllowList => "allow-list",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SandboxConfig {
    pub enabled: Option<bool>,
    pub namespace_restrictions: Option<bool>,
    pub network_isolation: Option<bool>,
    pub filesystem_mode: Option<FilesystemIsolationMode>,
    pub allowed_mounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SandboxRequest {
    pub enabled: bool,
    pub namespace_restrictions: bool,
    pub network_isolation: bool,
    pub filesystem_mode: FilesystemIsolationMode,
    pub allowed_mounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContainerEnvironment {
    pub in_container: bool,
    pub markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FeatureStatus {
    pub supported: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SandboxStatus {
    pub enabled: bool,
    pub requested: SandboxRequest,
    pub sandbox: FeatureStatus,
    pub namespace: FeatureStatus,
    pub network: FeatureStatus,
    pub filesystem_mode: FilesystemIsolationMode,
    pub filesystem_active: bool,
    pub allowed_mounts: Vec<String>,
    pub in_container: bool,
    pub container_markers: Vec<String>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxDetectionInputs<'a> {
    pub env_pairs: Vec<(String, String)>,
    pub dockerenv_exists: bool,
    pub containerenv_exists: bool,
    pub proc_1_cgroup: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl SandboxConfig {
    #[must_use]
    pub fn resolve_request(
        &self,
        enabled_override: Option<bool>,
        namespace_override: Option<bool>,
        network_override: Option<bool>,
        filesystem_mode_override: Option<FilesystemIsolationMode>,
        allowed_mounts_override: Option<Vec<String>>,
    ) -> SandboxRequest {
        SandboxRequest {
            enabled: enabled_override.unwrap_or(self.enabled.unwrap_or(true)),
            namespace_restrictions: namespace_override
                .unwrap_or(self.namespace_restrictions.unwrap_or(true)),
            network_isolation: network_override.unwrap_or(self.network_isolation.unwrap_or(false)),
            filesystem_mode: filesystem_mode_override
                .or(self.filesystem_mode)
                .unwrap_or_default(),
            allowed_mounts: allowed_mounts_override.unwrap_or_else(|| self.allowed_mounts.clone()),
        }
    }
}

#[must_use]
pub fn detect_container_environment() -> ContainerEnvironment {
    let proc_1_cgroup = fs::read_to_string("/proc/1/cgroup").ok();
    detect_container_environment_from(SandboxDetectionInputs {
        env_pairs: env::vars().collect(),
        dockerenv_exists: Path::new("/.dockerenv").exists(),
        containerenv_exists: Path::new("/run/.containerenv").exists(),
        proc_1_cgroup: proc_1_cgroup.as_deref(),
    })
}

#[must_use]
pub fn detect_container_environment_from(
    inputs: SandboxDetectionInputs<'_>,
) -> ContainerEnvironment {
    let mut markers = Vec::new();
    if inputs.dockerenv_exists {
        markers.push("/.dockerenv".to_string());
    }
    if inputs.containerenv_exists {
        markers.push("/run/.containerenv".to_string());
    }
    for (key, value) in inputs.env_pairs {
        let normalized = key.to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "container" | "docker" | "podman" | "kubernetes_service_host"
        ) && !value.is_empty()
        {
            markers.push(format!("env:{key}={value}"));
        }
    }
    if let Some(cgroup) = inputs.proc_1_cgroup {
        for needle in ["docker", "containerd", "kubepods", "podman", "libpod"] {
            if cgroup.contains(needle) {
                markers.push(format!("/proc/1/cgroup:{needle}"));
            }
        }
    }
    markers.sort();
    markers.dedup();
    ContainerEnvironment {
        in_container: !markers.is_empty(),
        markers,
    }
}

#[must_use]
pub fn resolve_sandbox_status(config: &SandboxConfig, cwd: &Path) -> SandboxStatus {
    let request = config.resolve_request(None, None, None, None, None);
    resolve_sandbox_status_for_request(&request, cwd)
}

#[must_use]
pub fn resolve_sandbox_status_for_request(request: &SandboxRequest, cwd: &Path) -> SandboxStatus {
    let container = detect_container_environment();
    let linux_supported = cfg!(target_os = "linux") && command_exists("unshare");
    let macos_supported = cfg!(target_os = "macos") && command_exists("sandbox-exec");
    let namespace_supported = linux_supported || macos_supported;
    let network_supported = namespace_supported;
    let filesystem_active =
        request.enabled && request.filesystem_mode != FilesystemIsolationMode::Off;
    let mut fallback_reasons = Vec::new();

    if request.enabled && request.namespace_restrictions && !namespace_supported {
        fallback_reasons.push(
            "process isolation unavailable (requires Linux `unshare` or macOS `sandbox-exec`)"
                .to_string(),
        );
    }
    if request.enabled && request.network_isolation && !network_supported {
        fallback_reasons.push(
            "network isolation unavailable (requires Linux `unshare` or macOS `sandbox-exec`)"
                .to_string(),
        );
    }
    if request.enabled
        && request.filesystem_mode == FilesystemIsolationMode::AllowList
        && request.allowed_mounts.is_empty()
    {
        fallback_reasons
            .push("filesystem allow-list requested without configured mounts".to_string());
    }

    let active = request.enabled
        && (!request.namespace_restrictions || namespace_supported)
        && (!request.network_isolation || network_supported);

    let allowed_mounts = normalize_mounts(&request.allowed_mounts, cwd);

    SandboxStatus {
        enabled: request.enabled,
        requested: request.clone(),
        sandbox: FeatureStatus {
            supported: namespace_supported,
            active,
        },
        namespace: FeatureStatus {
            supported: namespace_supported,
            active: request.enabled && request.namespace_restrictions && namespace_supported,
        },
        network: FeatureStatus {
            supported: network_supported,
            active: request.enabled && request.network_isolation && network_supported,
        },
        filesystem_mode: request.filesystem_mode,
        filesystem_active,
        allowed_mounts,
        in_container: container.in_container,
        container_markers: container.markers,
        fallback_reason: (!fallback_reasons.is_empty()).then(|| fallback_reasons.join("; ")),
    }
}

#[must_use]
pub fn build_sandbox_command(
    command: &str,
    cwd: &Path,
    status: &SandboxStatus,
) -> Option<SandboxCommand> {
    if !status.enabled || (!status.namespace.active && !status.network.active) {
        return None;
    }

    if cfg!(target_os = "linux") {
        build_linux_sandbox_command(command, cwd, status)
    } else if cfg!(target_os = "macos") {
        build_macos_sandbox_command(command, cwd, status)
    } else {
        None
    }
}

fn build_linux_sandbox_command(
    command: &str,
    cwd: &Path,
    status: &SandboxStatus,
) -> Option<SandboxCommand> {
    if !command_exists("unshare") {
        return None;
    }

    let mut args = vec![
        "--user".to_string(),
        "--map-root-user".to_string(),
        "--mount".to_string(),
        "--ipc".to_string(),
        "--pid".to_string(),
        "--uts".to_string(),
        "--fork".to_string(),
    ];
    if status.network.active {
        args.push("--net".to_string());
    }
    args.push("sh".to_string());
    args.push("-lc".to_string());
    args.push(command.to_string());

    Some(SandboxCommand {
        program: "unshare".to_string(),
        args,
        env: sandbox_env(cwd, status),
    })
}

fn build_macos_sandbox_command(
    command: &str,
    cwd: &Path,
    status: &SandboxStatus,
) -> Option<SandboxCommand> {
    if !command_exists("sandbox-exec") {
        return None;
    }

    let profile = generate_seatbelt_profile(cwd, status);
    let args = vec![
        "-p".to_string(),
        profile,
        "sh".to_string(),
        "-lc".to_string(),
        command.to_string(),
    ];

    Some(SandboxCommand {
        program: "sandbox-exec".to_string(),
        args,
        env: sandbox_env(cwd, status),
    })
}

fn sandbox_dirs(cwd: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let codineer_dir = crate::codineer_runtime_dir(cwd);
    (
        codineer_dir.join("sandbox-home"),
        codineer_dir.join("sandbox-tmp"),
    )
}

fn sandbox_env(cwd: &Path, status: &SandboxStatus) -> Vec<(String, String)> {
    let (sandbox_home, sandbox_tmp) = sandbox_dirs(cwd);
    let mut env = vec![
        ("HOME".to_string(), sandbox_home.display().to_string()),
        ("TMPDIR".to_string(), sandbox_tmp.display().to_string()),
        (
            "CODINEER_SANDBOX_FILESYSTEM_MODE".to_string(),
            status.filesystem_mode.as_str().to_string(),
        ),
        (
            "CODINEER_SANDBOX_ALLOWED_MOUNTS".to_string(),
            status.allowed_mounts.join(":"),
        ),
    ];
    if let Ok(path) = env::var("PATH") {
        env.push(("PATH".to_string(), path));
    }
    env
}

#[must_use]
pub fn generate_seatbelt_profile(cwd: &Path, status: &SandboxStatus) -> String {
    fn escape_seatbelt_path(path: &str) -> String {
        path.replace('\\', "\\\\").replace('"', "\\\"")
    }

    let cwd_str = escape_seatbelt_path(&cwd.display().to_string());
    let (sandbox_home, sandbox_tmp) = sandbox_dirs(cwd);

    let mut rules = vec![
        "(version 1)".to_string(),
        "(deny default)".to_string(),
        "(allow process-exec*)".to_string(),
        "(allow process-fork)".to_string(),
        "(allow sysctl-read)".to_string(),
        "(allow mach-lookup)".to_string(),
        "(allow signal (target self))".to_string(),
        "(allow ipc-posix-shm*)".to_string(),
        "(allow file-read* (subpath \"/usr\"))".to_string(),
        "(allow file-read* (subpath \"/bin\"))".to_string(),
        "(allow file-read* (subpath \"/sbin\"))".to_string(),
        "(allow file-read* (subpath \"/Library\"))".to_string(),
        "(allow file-read* (subpath \"/System\"))".to_string(),
        "(allow file-read* (subpath \"/private\"))".to_string(),
        "(allow file-read* (subpath \"/dev\"))".to_string(),
        "(allow file-read* (subpath \"/var\"))".to_string(),
        "(allow file-read* (subpath \"/etc\"))".to_string(),
        "(allow file-read* (subpath \"/opt\"))".to_string(),
        "(allow file-read* (subpath \"/tmp\"))".to_string(),
        "(allow file-read* (subpath \"/Applications\"))".to_string(),
    ];

    if let Some(home) = env::var_os("HOME") {
        let home_str = home
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        rules.push(format!(
            "(allow file-read* (subpath \"{home_str}/.cargo\"))"
        ));
        rules.push(format!(
            "(allow file-read* (subpath \"{home_str}/.rustup\"))"
        ));
    }

    match status.filesystem_mode {
        FilesystemIsolationMode::Off => {
            rules.push("(allow file-read*)".to_string());
            rules.push("(allow file-write*)".to_string());
        }
        FilesystemIsolationMode::WorkspaceOnly => {
            let sh = escape_seatbelt_path(&sandbox_home.display().to_string());
            let st = escape_seatbelt_path(&sandbox_tmp.display().to_string());
            rules.push(format!("(allow file-read* (subpath \"{cwd_str}\"))"));
            rules.push(format!("(allow file-write* (subpath \"{cwd_str}\"))"));
            rules.push(format!("(allow file-write* (subpath \"{sh}\"))"));
            rules.push(format!("(allow file-write* (subpath \"{st}\"))"));
        }
        FilesystemIsolationMode::AllowList => {
            let sh = escape_seatbelt_path(&sandbox_home.display().to_string());
            let st = escape_seatbelt_path(&sandbox_tmp.display().to_string());
            rules.push(format!("(allow file-read* (subpath \"{cwd_str}\"))"));
            rules.push(format!("(allow file-write* (subpath \"{sh}\"))"));
            rules.push(format!("(allow file-write* (subpath \"{st}\"))"));
            for mount in &status.allowed_mounts {
                let escaped = escape_seatbelt_path(mount);
                rules.push(format!("(allow file-read* (subpath \"{escaped}\"))"));
                rules.push(format!("(allow file-write* (subpath \"{escaped}\"))"));
            }
        }
    }

    if status.network.active {
        rules.push("(deny network*)".to_string());
    } else {
        rules.push("(allow network*)".to_string());
    }

    rules.join("\n")
}

fn normalize_mounts(mounts: &[String], cwd: &Path) -> Vec<String> {
    let cwd = cwd.to_path_buf();
    mounts
        .iter()
        .map(|mount| {
            let path = PathBuf::from(mount);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .map(|path| path.display().to_string())
        .collect()
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|path| path.join(command).exists()))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{
        build_sandbox_command, detect_container_environment_from, generate_seatbelt_profile,
        FilesystemIsolationMode, SandboxConfig, SandboxDetectionInputs,
    };
    use std::path::Path;

    #[test]
    fn detects_container_markers_from_multiple_sources() {
        let detected = detect_container_environment_from(SandboxDetectionInputs {
            env_pairs: vec![("container".to_string(), "docker".to_string())],
            dockerenv_exists: true,
            containerenv_exists: false,
            proc_1_cgroup: Some("12:memory:/docker/abc"),
        });

        assert!(detected.in_container);
        assert!(detected
            .markers
            .iter()
            .any(|marker| marker == "/.dockerenv"));
        assert!(detected
            .markers
            .iter()
            .any(|marker| marker == "env:container=docker"));
        assert!(detected
            .markers
            .iter()
            .any(|marker| marker == "/proc/1/cgroup:docker"));
    }

    #[test]
    fn resolves_request_with_overrides() {
        let config = SandboxConfig {
            enabled: Some(true),
            namespace_restrictions: Some(true),
            network_isolation: Some(false),
            filesystem_mode: Some(FilesystemIsolationMode::WorkspaceOnly),
            allowed_mounts: vec!["logs".to_string()],
        };

        let request = config.resolve_request(
            Some(true),
            Some(false),
            Some(true),
            Some(FilesystemIsolationMode::AllowList),
            Some(vec!["tmp".to_string()]),
        );

        assert!(request.enabled);
        assert!(!request.namespace_restrictions);
        assert!(request.network_isolation);
        assert_eq!(request.filesystem_mode, FilesystemIsolationMode::AllowList);
        assert_eq!(request.allowed_mounts, vec!["tmp"]);
    }

    #[test]
    fn builds_sandbox_command_for_current_platform() {
        let config = SandboxConfig::default();
        let status = super::resolve_sandbox_status_for_request(
            &config.resolve_request(
                Some(true),
                Some(true),
                Some(true),
                Some(FilesystemIsolationMode::WorkspaceOnly),
                None,
            ),
            Path::new("/workspace"),
        );

        if let Some(launcher) = build_sandbox_command("printf hi", Path::new("/workspace"), &status)
        {
            if cfg!(target_os = "linux") {
                assert_eq!(launcher.program, "unshare");
                assert!(launcher.args.iter().any(|arg| arg == "--mount"));
                assert!(launcher.args.iter().any(|arg| arg == "--net") == status.network.active);
            } else if cfg!(target_os = "macos") {
                assert_eq!(launcher.program, "sandbox-exec");
                assert!(launcher.args.iter().any(|arg| arg == "-p"));
            }
        }
    }

    #[test]
    fn seatbelt_profile_denies_by_default() {
        let config = SandboxConfig::default();
        let status = super::resolve_sandbox_status_for_request(
            &config.resolve_request(
                Some(true),
                Some(true),
                Some(false),
                Some(FilesystemIsolationMode::WorkspaceOnly),
                None,
            ),
            Path::new("/workspace"),
        );
        let profile = generate_seatbelt_profile(Path::new("/workspace"), &status);
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow file-write* (subpath \"/workspace\"))"));
        assert!(profile.contains("(allow network*)"));
    }

    #[test]
    fn seatbelt_profile_denies_network_when_isolated() {
        let config = SandboxConfig::default();
        let mut status = super::resolve_sandbox_status_for_request(
            &config.resolve_request(
                Some(true),
                Some(true),
                Some(true),
                Some(FilesystemIsolationMode::WorkspaceOnly),
                None,
            ),
            Path::new("/workspace"),
        );
        status.network.active = true;
        let profile = generate_seatbelt_profile(Path::new("/workspace"), &status);
        assert!(profile.contains("(deny network*)"));
        assert!(!profile.contains("(allow network*)"));
    }

    #[test]
    fn seatbelt_profile_allow_list_restricts_writes() {
        let config = SandboxConfig::default();
        let status = super::resolve_sandbox_status_for_request(
            &config.resolve_request(
                Some(true),
                Some(true),
                Some(false),
                Some(FilesystemIsolationMode::AllowList),
                Some(vec!["/extra/mount".to_string()]),
            ),
            Path::new("/workspace"),
        );
        let profile = generate_seatbelt_profile(Path::new("/workspace"), &status);
        assert!(profile.contains("(allow file-write* (subpath \"/extra/mount\"))"));
        assert!(!profile.contains("(allow file-write* (subpath \"/workspace\"))"));
    }

    #[test]
    fn disabled_sandbox_returns_none() {
        let config = SandboxConfig::default();
        let status = super::resolve_sandbox_status_for_request(
            &config.resolve_request(Some(false), None, None, None, None),
            Path::new("/workspace"),
        );
        assert!(build_sandbox_command("echo hi", Path::new("/workspace"), &status).is_none());
    }
}
