use crate::paths;
use std::io;
use std::process::Command;

/// Build the `<array>` of ProgramArguments: the pinned binary + `record` + perf flags.
pub fn program_arguments() -> Vec<String> {
    let mut v = vec![
        paths::recorder_binary().to_string_lossy().into_owned(),
        "record".to_string(),
    ];
    v.extend(extra_args());
    v
}

/// Read the persisted extra `record` flags (performance profile).
fn extra_args() -> Vec<String> {
    let txt = std::fs::read_to_string(paths::record_config()).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&txt)
        .ok()
        .and_then(|v| {
            v.get("args").and_then(|a| a.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect::<Vec<String>>()
            })
        })
        .unwrap_or_default()
}

pub fn get_record_args() -> Vec<String> {
    extra_args()
}

/// Persist new extra `record` flags, regenerate the plist, and restart the agent.
pub fn set_record_args(args: &[String]) -> io::Result<()> {
    if let Some(parent) = paths::record_config().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::json!({ "args": args });
    std::fs::write(
        paths::record_config(),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )?;
    write_plist()?;
    if is_loaded() {
        reload()?;
    }
    Ok(())
}

/// Reload the plist (bootout + bootstrap). Required after changing ProgramArguments —
/// `kickstart` restarts the process but does NOT re-read the plist from disk.
pub fn reload() -> io::Result<()> {
    if is_loaded() {
        let _ = run_launchctl(&bootout_args(current_uid()));
    }
    start()
}

/// One-time migration off the old `com.screenpipe.keeper` agent: bootout the old
/// service (if loaded), delete its plist and its old pinned dir. Best-effort.
pub fn migrate_from_keeper() {
    if paths::legacy_plist_path().exists() {
        let uid = current_uid();
        let _ = run_launchctl(&[
            "bootout".into(),
            format!("gui/{uid}/{}", paths::LEGACY_LABEL),
        ]);
        let _ = std::fs::remove_file(paths::legacy_plist_path());
        let _ = std::fs::remove_dir_all(paths::legacy_pinned_dir());
    }
}

/// Pure: generate the LaunchAgent plist XML.
pub fn generate_plist(program_args: &[String], out_log: &str, err_log: &str) -> String {
    let args_xml: String = program_args
        .iter()
        .map(|a| format!("    <string>{}</string>\n", a))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
{args}  </array>
  <key>KeepAlive</key>
  <true/>
  <key>RunAtLoad</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{out}</string>
  <key>StandardErrorPath</key>
  <string>{err}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>{home}</string>
    <key>PATH</key>
    <string>/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin</string>
  </dict>
</dict>
</plist>
"#,
        label = paths::LABEL,
        args = args_xml,
        out = out_log,
        err = err_log,
        home = std::env::var("HOME").unwrap_or_default(),
    )
}

pub fn current_uid() -> u32 {
    // SAFETY: getuid is always safe; it has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

pub fn bootstrap_args(uid: u32, plist: &str) -> Vec<String> {
    vec!["bootstrap".into(), format!("gui/{uid}"), plist.into()]
}

pub fn bootout_args(uid: u32) -> Vec<String> {
    vec!["bootout".into(), format!("gui/{uid}/{}", paths::LABEL)]
}

pub fn kickstart_args(uid: u32) -> Vec<String> {
    vec!["kickstart".into(), "-k".into(), format!("gui/{uid}/{}", paths::LABEL)]
}

pub fn print_args(uid: u32) -> Vec<String> {
    vec!["print".into(), format!("gui/{uid}/{}", paths::LABEL)]
}

fn run_launchctl(args: &[String]) -> io::Result<std::process::Output> {
    Command::new("launchctl").args(args).output()
}

/// Turn raw launchctl stderr into a friendly message.
fn humanize_launchctl(stderr: &str) -> String {
    let s = stderr.trim();
    if s.contains("Input/output error") || s.contains("already") {
        "Service is already loaded.".into()
    } else if s.contains("No such process") || s.contains("Could not find") {
        "Service is not loaded.".into()
    } else if s.contains("Operation not permitted") {
        "Operation not permitted — check the LaunchAgent plist.".into()
    } else if s.is_empty() {
        "launchctl failed with no message.".into()
    } else {
        format!("launchctl: {s}")
    }
}

pub fn is_installed() -> bool {
    paths::plist_path().exists()
}

/// Write the plist + create the log dir.
pub fn write_plist() -> io::Result<()> {
    std::fs::create_dir_all(paths::log_dir())?;
    if let Some(parent) = paths::plist_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let xml = generate_plist(
        &program_arguments(),
        &paths::out_log().to_string_lossy(),
        &paths::err_log().to_string_lossy(),
    );
    std::fs::write(paths::plist_path(), xml)
}

/// Write plist (if missing) and bootstrap the agent.
pub fn install() -> io::Result<()> {
    migrate_from_keeper();
    write_plist()?;
    start()
}

pub fn start() -> io::Result<()> {
    if is_loaded() {
        return Ok(()); // already running — Start is a no-op
    }
    let plist = paths::plist_path().to_string_lossy().into_owned();
    let out = run_launchctl(&bootstrap_args(current_uid(), &plist))?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Error 5 "Input/output error" / "already" => already bootstrapped, treat as success.
        if stderr.contains("Input/output error") || stderr.contains("already") {
            Ok(())
        } else {
            Err(io::Error::other(humanize_launchctl(&stderr)))
        }
    }
}

pub fn stop() -> io::Result<()> {
    if !is_loaded() {
        return Ok(()); // already stopped
    }
    let out = run_launchctl(&bootout_args(current_uid()))?;
    if out.status.success() || String::from_utf8_lossy(&out.stderr).contains("No such process") {
        Ok(())
    } else {
        Err(io::Error::other(humanize_launchctl(&String::from_utf8_lossy(&out.stderr))))
    }
}

pub fn restart() -> io::Result<()> {
    if !is_loaded() {
        return start(); // can't kickstart an unloaded service; load it instead
    }
    let out = run_launchctl(&kickstart_args(current_uid()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(humanize_launchctl(&String::from_utf8_lossy(&out.stderr))))
    }
}

/// True if `launchctl print` finds the loaded service.
pub fn is_loaded() -> bool {
    run_launchctl(&print_args(current_uid()))
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Pure: from the agent's err.log content, return the permissions screenpipe is
/// still waiting on. Only the LATEST "checking permissions" block is considered,
/// so stale lines from earlier launches are ignored.
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
    fn plist_contains_required_keys() {
        let xml = generate_plist(&["/bin/sp".into(), "record".into()], "/tmp/o.log", "/tmp/e.log");
        assert!(xml.contains("<key>Label</key>"));
        assert!(xml.contains("com.meye.recorder.agent"));
        assert!(xml.contains("<key>KeepAlive</key>"));
        assert!(xml.contains("<key>RunAtLoad</key>"));
        assert!(xml.contains("<string>/bin/sp</string>"));
        assert!(xml.contains("<string>record</string>"));
        assert!(xml.contains("/tmp/o.log"));
        assert!(xml.contains("/tmp/e.log"));
    }

    #[test]
    fn launchctl_args_are_well_formed() {
        assert_eq!(bootstrap_args(501, "/p.plist"), vec!["bootstrap", "gui/501", "/p.plist"]);
        assert_eq!(bootout_args(501), vec!["bootout", "gui/501/com.meye.recorder.agent"]);
        assert_eq!(kickstart_args(501), vec!["kickstart", "-k", "gui/501/com.meye.recorder.agent"]);
        assert_eq!(print_args(501), vec!["print", "gui/501/com.meye.recorder.agent"]);
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
