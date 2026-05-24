use std::path::PathBuf;

pub const LABEL: &str = "com.meye.recorder.agent";
pub const RECORDER_BUNDLE_ID: &str = "com.meye.recorder";
pub const PORT: u16 = 3030;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME must be set"))
}

fn app_support() -> PathBuf {
    home().join("Library/Application Support/meye")
}

/// The signed recorder app bundle that launchd launches.
pub fn recorder_app() -> PathBuf {
    app_support().join("Meye Recorder.app")
}

/// Directory holding the screenpipe executable + mlx.metallib inside the bundle.
pub fn recorder_macos_dir() -> PathBuf {
    recorder_app().join("Contents/MacOS")
}

/// The pinned screenpipe executable (inside the bundle).
pub fn recorder_binary() -> PathBuf {
    recorder_macos_dir().join("screenpipe")
}

pub fn recorder_info_plist() -> PathBuf {
    recorder_app().join("Contents/Info.plist")
}

pub fn plist_path() -> PathBuf {
    home().join("Library/LaunchAgents/com.meye.recorder.agent.plist")
}

pub fn log_dir() -> PathBuf {
    home().join("Library/Logs/meye")
}

pub fn out_log() -> PathBuf {
    log_dir().join("out.log")
}

pub fn err_log() -> PathBuf {
    log_dir().join("err.log")
}

/// screenpipe's own data dir (unchanged — owned by screenpipe, not us).
pub fn data_dir() -> PathBuf {
    home().join(".screenpipe")
}

/// Persisted extra `record` flags (performance profile) the agent launches with.
pub fn record_config() -> PathBuf {
    app_support().join("record-config.json")
}

/// Persistent Home activity/chat thread (JSONL, one entry per line).
pub fn activity_log() -> PathBuf {
    app_support().join("activity.jsonl")
}

// --- legacy keeper paths, used only for one-time migration ---
pub fn legacy_plist_path() -> PathBuf {
    home().join("Library/LaunchAgents/com.screenpipe.keeper.plist")
}
pub fn legacy_pinned_dir() -> PathBuf {
    home().join("Library/Application Support/screenpipe-keeper")
}
pub const LEGACY_LABEL: &str = "com.screenpipe.keeper";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_home() {
        let h = home();
        assert!(recorder_binary().starts_with(&h));
        assert!(recorder_binary().ends_with("Meye Recorder.app/Contents/MacOS/screenpipe"));
        assert!(plist_path().ends_with("com.meye.recorder.agent.plist"));
        assert_eq!(out_log().file_name().unwrap(), "out.log");
        assert!(log_dir().ends_with("Logs/meye"));
        assert!(data_dir().ends_with(".screenpipe"));
    }
}
