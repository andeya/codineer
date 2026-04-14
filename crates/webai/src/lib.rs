pub mod auth_store;
pub mod engine;
pub mod error;
pub mod page;
pub mod page_manager;
pub mod provider;
pub mod providers;
pub mod sse_parser;
pub mod tool_calling;
pub mod webauth;

pub use engine::{OpenAiStreamResult, WebAiEngine};
pub use error::{WebAiError, WebAiResult};
pub use page::WebAiPage;
pub use page_manager::WebAiPageManager;
pub use provider::{ModelInfo, ProviderConfig, StreamResult, WebProviderClient};

/// Return a browser-compatible user-agent string for the embedded WebView.
///
/// Each platform's default WebView UA has quirks that cause some websites
/// (e.g. claude.ai) to serve incompatible JS bundles:
///
/// - **macOS** — WKWebView omits `Version/x.x Safari/x.x`; sites think it
///   is an embedded view and may ship a stripped-down (or cutting-edge) bundle.
/// - **Windows** — WebView2 appends `Edg/x.x`; some sites apply Edge-specific
///   workarounds that can differ from standard Chrome behaviour.
/// - **Linux** — WebKitGTK may report an outdated engine version.
///
/// The result is cached so the (potentially expensive) version detection only
/// runs once per process.
pub fn browser_user_agent() -> Option<&'static str> {
    use std::sync::OnceLock;
    static UA: OnceLock<Option<String>> = OnceLock::new();
    UA.get_or_init(detect_browser_ua).as_deref()
}

fn detect_browser_ua() -> Option<String> {
    #[cfg(target_os = "macos")]
    return detect_macos_ua();

    #[cfg(target_os = "windows")]
    return detect_windows_ua();

    #[cfg(target_os = "linux")]
    return detect_linux_ua();

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return None;
}

// ── macOS: read local Safari version, build a real Safari fingerprint ────────

#[cfg(target_os = "macos")]
fn detect_macos_ua() -> Option<String> {
    let version = cmd_first_line(
        "/usr/bin/defaults",
        &[
            "read",
            "/Applications/Safari.app/Contents/Info",
            "CFBundleShortVersionString",
        ],
    )
    .unwrap_or_else(|| "17.0".into());

    // Safari always reports "Intel Mac OS X 10_15_7" even on Apple Silicon.
    Some(format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
         AppleWebKit/605.1.15 (KHTML, like Gecko) \
         Version/{version} Safari/605.1.15"
    ))
}

// ── Windows: read Edge (Chromium) version, emit a clean Chrome UA ────────────

#[cfg(target_os = "windows")]
fn detect_windows_ua() -> Option<String> {
    let chrome_ver = windows_edge_version().unwrap_or_else(|| "130.0.0.0".into());
    let arch = if std::env::consts::ARCH == "aarch64" {
        "Windows NT 10.0; ARM64"
    } else {
        "Windows NT 10.0; Win64; x64"
    };
    Some(format!(
        "Mozilla/5.0 ({arch}) \
         AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{chrome_ver} Safari/537.36"
    ))
}

#[cfg(target_os = "windows")]
fn windows_edge_version() -> Option<String> {
    // Fast path: registry query (no PowerShell overhead).
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Edge\BLBeacon",
            "/v",
            "version",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    // Output: "    version    REG_SZ    130.0.2849.56"
    text.lines()
        .find(|l| l.contains("version"))
        .and_then(|l| l.split_whitespace().last())
        .filter(|v| v.contains('.'))
        .map(String::from)
}

// ── Linux: detect WebKitGTK version, build a Safari-like UA ──────────────────

#[cfg(target_os = "linux")]
fn detect_linux_ua() -> Option<String> {
    let safari_ver = linux_webkit_to_safari_version().unwrap_or_else(|| "17.0".into());
    let arch = std::env::consts::ARCH; // "x86_64" or "aarch64"
    Some(format!(
        "Mozilla/5.0 (X11; Linux {arch}) \
         AppleWebKit/605.1.15 (KHTML, like Gecko) \
         Version/{safari_ver} Safari/605.1.15"
    ))
}

#[cfg(target_os = "linux")]
fn linux_webkit_to_safari_version() -> Option<String> {
    // Try webkit2gtk-4.1 first (GTK4 / newer distros), fall back to 4.0.
    let gtk_ver = cmd_first_line("pkg-config", &["--modversion", "webkit2gtk-4.1"])
        .or_else(|| cmd_first_line("pkg-config", &["--modversion", "webkit2gtk-4.0"]))?;

    // Rough mapping: WebKitGTK minor version → equivalent Safari version.
    // WebKitGTK uses 2.MINOR.PATCH; the minor number tracks WebKit trunk.
    let minor: u32 = gtk_ver.split('.').nth(1)?.parse().ok()?;
    let safari = match minor {
        46.. => "18.2",
        44..=45 => "18.0",
        42..=43 => "17.0",
        40..=41 => "16.4",
        _ => "16.0",
    };
    Some(safari.into())
}

// ── shared helper ────────────────────────────────────────────────────────────

fn cmd_first_line(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
