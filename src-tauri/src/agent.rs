use crate::paths;
use std::io;

// Platform supervision backends. Each exports the same surface:
//   install, start, stop, restart, reload, is_installed, is_loaded, is_running,
//   write_launch_config, migrate_plist_format
// macOS: launchd LaunchAgent. Windows: Task Scheduler task + hidden launcher.
#[cfg(target_os = "macos")]
#[path = "agent_macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "agent_windows.rs"]
mod platform;
pub use platform::*;

/// Build the recorder command line: the pinned binary + `record` + perf flags.
pub fn program_arguments() -> Vec<String> {
    let mut v = vec![
        paths::recorder_binary().to_string_lossy().into_owned(),
        "record".to_string(),
    ];
    v.extend(extra_args());
    // Bound DB/disk growth. screenpipe writes a per-frame accessibility blob plus decomposed
    // OCR/AX `elements` rows + an FTS5 index, all unbounded without retention. A bloated DB
    // (observed at 11GB / 2.4M element rows) causes write amplification that throttles capture
    // to a near-frozen fps. Retention is the only durable bound, so force it on even for older
    // saved record-config.json files written before this flag existed — otherwise upgrading
    // users keep growing forever. A config that already pins --retention-days is left untouched.
    if !v.iter().any(|a| a == "--retention-days") {
        v.push("--retention-days".into());
        v.push("30".into());
        v.push("--retention-mode".into());
        v.push("all".into());
    }
    // Remote viewing (opt-in, off by default): serve the API on the LAN with a bearer
    // token required for non-localhost callers. The host's own app keeps using
    // 127.0.0.1, which screenpipe always allows. Toggled via `set_remote_enabled`.
    if crate::prefs::get_remote_enabled() {
        v.push("--listen-on-lan".into());
        v.push("--api-auth".into());
    }
    v
}

/// Default profile for fresh installs: Best model + input-latency priority,
/// capturing screen + computer (system) audio but NOT the microphone. The mic is the heavy,
/// flaky source (its stream gaps force constant CPU resampling), so it's off by default; the
/// user can enable it in Performance → Capture sources. "System Audio (output)" is screenpipe's
/// standard macOS system-audio device.
#[cfg(target_os = "macos")]
pub fn default_record_args() -> Vec<String> {
    vec![
        "--audio-transcription-engine".into(),
        "whisper-large-v3-turbo".into(),
        "--prioritize-input-latency".into(),
        "--audio-device".into(),
        "System Audio (output)".into(),
        // Cap DB/disk growth — parity with the Windows default. Without this the macOS DB
        // grew unbounded (11GB), and the resulting write amplification throttled capture.
        // POST /retention/configure does not persist across recorder restarts, so it must
        // be a launch flag. The user can lower this in Performance → Capture sources.
        "--retention-days".into(),
        "30".into(),
        "--retention-mode".into(),
        "all".into(),
    ]
}

/// Windows default: same engine + latency priority, but no explicit audio device —
/// device names are hardware-specific here ("Speakers (Realtek…)"), so let screenpipe
/// pick its defaults. The user can refine in Performance → Capture sources.
#[cfg(windows)]
pub fn default_record_args() -> Vec<String> {
    vec![
        "--audio-transcription-engine".into(),
        "whisper-large-v3-turbo".into(),
        "--prioritize-input-latency".into(),
        // Cap DB/disk growth: screenpipe always writes a ~68KB accessibility-tree blob per
        // frame (no flag disables it) plus media, so retention is the only durable bound.
        // Configured here as a launch flag because POST /retention/configure does NOT persist
        // across recorder restarts (resets to disabled every logon).
        "--retention-days".into(),
        "30".into(),
        "--retention-mode".into(),
        "all".into(),
    ]
}

/// Read the persisted extra `record` flags (performance profile), or the default if none saved.
fn extra_args() -> Vec<String> {
    let Ok(txt) = std::fs::read_to_string(paths::record_config()) else {
        return default_record_args(); // no config yet — fresh install
    };
    serde_json::from_str::<serde_json::Value>(&txt)
        .ok()
        .and_then(|v| {
            v.get("args").and_then(|a| a.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect::<Vec<String>>()
            })
        })
        .unwrap_or_else(default_record_args)
}

pub fn get_record_args() -> Vec<String> {
    extra_args()
}

/// Persist new extra `record` flags, regenerate the launch config, and restart the agent.
pub fn set_record_args(args: &[String]) -> io::Result<()> {
    if let Some(parent) = paths::record_config().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::json!({ "args": args });
    std::fs::write(
        paths::record_config(),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )?;
    write_launch_config()?;
    if is_loaded() {
        reload()?;
    }
    Ok(())
}

/// Toggle LAN remote-viewing exposure: persist the pref, regenerate the launch config
/// (so the relaunched recorder picks up / drops --listen-on-lan), and restart it.
pub fn set_remote_enabled(on: bool) -> io::Result<()> {
    crate::prefs::set_remote_enabled(on).map_err(io::Error::other)?;
    write_launch_config()?;
    if is_loaded() {
        reload()?;
    }
    Ok(())
}

/// Pure: from the agent's err.log content, return the permissions screenpipe is
/// still waiting on. Only the LATEST "checking permissions" block is considered,
/// so stale lines from earlier launches are ignored. (macOS TCC markers; on
/// Windows screenpipe never logs these, so this stays empty there.)
pub fn parse_missing_permissions(log: &str) -> Vec<String> {
    let tail = match log.rfind("checking permissions") {
        Some(idx) => &log[idx..],
        None => log,
    };
    let mut out = Vec::new();
    if tail.contains("screen recording: waiting") {
        out.push("Screen Recording".to_string());
    }
    if tail.contains("microphone: waiting") {
        out.push("Microphone".to_string());
    }
    if tail.contains("accessibility: waiting") {
        out.push("Accessibility".to_string());
    }
    out
}

/// Read the agent err.log and report which permissions screenpipe is waiting on.
pub fn missing_permissions() -> Vec<String> {
    let log = std::fs::read_to_string(paths::err_log()).unwrap_or_default();
    parse_missing_permissions(&log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_best_model() {
        let d = default_record_args();
        assert!(d.contains(&"whisper-large-v3-turbo".to_string()));
        assert!(d.contains(&"--prioritize-input-latency".to_string()));
    }

    #[test]
    fn missing_permissions_uses_latest_block_only() {
        // Earlier block: both waiting. Latest block: only microphone still waiting.
        let log = "\
checking permissions...
  screen recording: waiting — grant access
  microphone: waiting — grant access

[some other log lines]
checking permissions...
  microphone: waiting — grant access
";
        let missing = parse_missing_permissions(log);
        assert_eq!(missing, vec!["Microphone".to_string()]);

        assert!(parse_missing_permissions("no markers here").is_empty());
    }
}
