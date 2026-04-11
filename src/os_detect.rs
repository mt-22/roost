use serde::{Deserialize, Serialize};

/// OS and hardware information stored in local.toml.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsInfo {
    /// "unix" or "windows"
    pub family: String,
    /// Distro/OS slug: "macos", "arch", "ubuntu", "debian", "fedora", "nixos", etc.
    pub name: String,
    /// Version string when detectable (e.g. "15.3.1" for macOS, "22.04" for Ubuntu)
    pub version: Option<String>,
    /// CPU architecture from the Rust target triple: "x86_64", "aarch64", etc.
    pub arch: String,
}

impl Default for OsInfo {
    fn default() -> Self {
        detect()
    }
}

/// Detect OS information at runtime.
pub fn detect() -> OsInfo {
    OsInfo {
        family: detect_family(),
        name: detect_name(),
        version: detect_version(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

// ── family ──────────────────────────────────────────────────────────────────

fn detect_family() -> String {
    detect_family_inner()
}

#[cfg(windows)]
fn detect_family_inner() -> String {
    "windows".to_string()
}

#[cfg(not(windows))]
fn detect_family_inner() -> String {
    "unix".to_string()
}

// ── name ────────────────────────────────────────────────────────────────────

fn detect_name() -> String {
    detect_name_inner()
}

#[cfg(target_os = "macos")]
fn detect_name_inner() -> String {
    use std::process::Command;
    Command::new("sw_vers")
        .arg("-productName")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_lowercase().replace(' ', "-"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "macos".to_string())
}

#[cfg(target_os = "linux")]
fn detect_name_inner() -> String {
    parse_os_release().0
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn detect_name_inner() -> String {
    std::env::consts::OS.to_string()
}

// ── version ─────────────────────────────────────────────────────────────────

fn detect_version() -> Option<String> {
    detect_version_inner()
}

#[cfg(target_os = "macos")]
fn detect_version_inner() -> Option<String> {
    use std::process::Command;
    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "linux")]
fn detect_version_inner() -> Option<String> {
    parse_os_release().1
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn detect_version_inner() -> Option<String> {
    None
}

// ── /etc/os-release parser ───────────────────────────────────────────────────

/// Parse /etc/os-release (or /usr/lib/os-release as fallback) and return
/// (distro_id, version_id).
#[cfg(target_os = "linux")]
fn parse_os_release() -> (String, Option<String>) {
    let content = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();

    let mut id: Option<String> = None;
    let mut version_id: Option<String> = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("ID=") {
            id = Some(val.trim_matches('"').to_lowercase());
        } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
            version_id = Some(val.trim_matches('"').to_string());
        }
    }

    let name = id.unwrap_or_else(|| "linux".to_string());
    (name, version_id)
}
