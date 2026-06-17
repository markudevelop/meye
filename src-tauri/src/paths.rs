use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub const LABEL: &str = "com.meye.recorder.agent";
#[cfg(target_os = "macos")]
pub const RECORDER_BUNDLE_ID: &str = "com.meye.recorder";

/// Windows Scheduled Task name — the launchd label equivalent.
#[cfg(windows)]
pub const TASK_NAME: &str = "MeyeRecorder";
/// The rebranded recorder image name; tasklist/taskkill key off this.
#[cfg(windows)]
pub const RECORDER_EXE: &str = "meye-recorder.exe";

pub const PORT: u16 = 3030;

fn home() -> PathBuf {
    #[cfg(windows)]
    let var = "USERPROFILE";
    #[cfg(not(windows))]
    let var = "HOME";
    PathBuf::from(std::env::var(var).expect("home dir env var must be set"))
}

pub fn app_support() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home().join("AppData/Local"))
            .join("meye")
    }
    #[cfg(not(windows))]
    {
        home().join("Library/Application Support/meye")
    }
}

/// The signed recorder app bundle that launchd launches.
#[cfg(target_os = "macos")]
pub fn recorder_app() -> PathBuf {
    app_support().join("Meye Recorder.app")
}

/// Directory holding the screenpipe executable + mlx.metallib inside the bundle.
#[cfg(target_os = "macos")]
pub fn recorder_macos_dir() -> PathBuf {
    recorder_app().join("Contents/MacOS")
}

/// Directory holding the pinned screenpipe executable (+ its DLLs).
#[cfg(windows)]
pub fn recorder_dir() -> PathBuf {
    app_support().join("recorder")
}

/// The pinned recorder executable. Renamed from upstream `screenpipe` so TCC /
/// Task Manager / process lists show our brand (and taskkill hits only us).
pub fn recorder_binary() -> PathBuf {
    #[cfg(windows)]
    {
        recorder_dir().join(RECORDER_EXE)
    }
    #[cfg(not(windows))]
    {
        recorder_macos_dir().join("meye-recorder")
    }
}

#[cfg(target_os = "macos")]
pub fn recorder_info_plist() -> PathBuf {
    recorder_app().join("Contents/Info.plist")
}

#[cfg(target_os = "macos")]
pub fn plist_path() -> PathBuf {
    home().join("Library/LaunchAgents/com.meye.recorder.agent.plist")
}

/// VBScript launcher the Scheduled Task runs: starts the recorder with no console
/// window and redirects stdout/stderr to the log files.
#[cfg(windows)]
pub fn launcher_vbs() -> PathBuf {
    app_support().join("recorder-launch.vbs")
}

/// The Task Scheduler definition XML we register from.
#[cfg(windows)]
pub fn task_xml() -> PathBuf {
    app_support().join("recorder-task.xml")
}

pub fn log_dir() -> PathBuf {
    #[cfg(windows)]
    {
        app_support().join("logs")
    }
    #[cfg(not(windows))]
    {
        home().join("Library/Logs/meye")
    }
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

/// Persisted GUI preferences (e.g. discreet mode).
pub fn ui_prefs() -> PathBuf {
    app_support().join("ui-prefs.json")
}

/// Legacy single Home thread (migrated into a conversation on first run).
pub fn activity_log() -> PathBuf {
    app_support().join("activity.jsonl")
}

/// Directory of conversation threads (one `<id>.jsonl` per conversation).
pub fn convos_dir() -> PathBuf {
    app_support().join("conversations")
}

// --- legacy keeper paths, used only for one-time migration ---
#[cfg(target_os = "macos")]
pub fn legacy_plist_path() -> PathBuf {
    home().join("Library/LaunchAgents/com.screenpipe.keeper.plist")
}
#[cfg(target_os = "macos")]
pub fn legacy_pinned_dir() -> PathBuf {
    home().join("Library/Application Support/screenpipe-keeper")
}
#[cfg(target_os = "macos")]
pub const LEGACY_LABEL: &str = "com.screenpipe.keeper";

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn paths_are_under_home() {
        let h = home();
        assert!(recorder_binary().starts_with(&h));
        assert!(recorder_binary().ends_with("Meye Recorder.app/Contents/MacOS/meye-recorder"));
        assert!(plist_path().ends_with("com.meye.recorder.agent.plist"));
        assert_eq!(out_log().file_name().unwrap(), "out.log");
        assert!(log_dir().ends_with("Logs/meye"));
        assert!(data_dir().ends_with(".screenpipe"));
    }

    #[cfg(windows)]
    #[test]
    fn paths_are_under_profile() {
        assert!(recorder_binary().ends_with("meye\\recorder\\meye-recorder.exe"));
        assert!(launcher_vbs().ends_with("meye\\recorder-launch.vbs"));
        assert_eq!(out_log().file_name().unwrap(), "out.log");
        assert!(log_dir().ends_with("meye\\logs"));
        assert!(data_dir().ends_with(".screenpipe"));
    }
}
