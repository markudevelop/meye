//! Small persisted GUI preferences (discreet mode). Stored as JSON in app support.
use crate::paths;
use serde_json::Value;

fn read() -> Value {
    std::fs::read_to_string(paths::ui_prefs())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn write(v: &Value) -> Result<(), String> {
    if let Some(parent) = paths::ui_prefs().parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(paths::ui_prefs(), serde_json::to_string_pretty(v).unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// Discreet mode: hide Meye's own dock + tray icons (does NOT touch the macOS
/// recording indicator — that stays so anyone you screen-share with is still informed).
pub fn get_discreet() -> bool {
    read().get("discreet").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn set_discreet(on: bool) -> Result<(), String> {
    let mut v = read();
    v["discreet"] = Value::Bool(on);
    write(&v)
}

/// Remote viewing: when on, the recorder is launched with --listen-on-lan + --api-auth
/// so another Meye instance on the LAN can view this machine (bearer-token required for
/// non-localhost). Off by default — see `agent::program_arguments`.
pub fn get_remote_enabled() -> bool {
    read().get("remote_enabled").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn set_remote_enabled(on: bool) -> Result<(), String> {
    let mut v = read();
    v["remote_enabled"] = Value::Bool(on);
    write(&v)
}

/// Where the seeded `obsidian-sync` pipe writes daily notes. Falls back to a per-OS default
/// (the same value onboarding/Settings pre-fill) so seeding has a target even before the user
/// picks one. The value is substituted into the bundled pipe's `{{VAULT}}` placeholder.
pub fn get_obsidian_vault() -> String {
    read()
        .get("obsidian_vault")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_obsidian_vault)
}

pub fn set_obsidian_vault(path: &str) -> Result<(), String> {
    let mut v = read();
    v["obsidian_vault"] = Value::String(path.to_string());
    write(&v)
}

/// Best-guess Obsidian vault location per OS: macOS → the iCloud Obsidian path, Windows →
/// `%USERPROFILE%\iCloudDrive\Obsidian`, otherwise `~/Documents/Obsidian`.
pub fn default_obsidian_vault() -> String {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE")
    } else {
        std::env::var("HOME")
    }
    .unwrap_or_default();
    #[cfg(target_os = "macos")]
    let rel = "Library/Mobile Documents/com~apple~CloudDocs/Obsidian/Obsidian Vault iCloud";
    #[cfg(windows)]
    let rel = "iCloudDrive\\Obsidian";
    #[cfg(not(any(target_os = "macos", windows)))]
    let rel = "Documents/Obsidian";
    std::path::Path::new(&home).join(rel).to_string_lossy().into_owned()
}
