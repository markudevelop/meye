//! Windows recorder supervision: a Task Scheduler task + hidden VBScript launcher.
//!
//! launchd → Task Scheduler mapping:
//!   plist               → task XML (registered via `schtasks /Create /XML`) + a wscript
//!                         launcher that runs the recorder with NO console window and
//!                         appends stdout/stderr to the log files
//!   RunAtLoad           → logon trigger
//!   KeepAlive           → 1-minute repetition with MultipleInstancesPolicy=IgnoreNew:
//!                         while the recorder is alive the task instance is still
//!                         "Running" (the launcher waits on it), so each tick is a
//!                         no-op; after a crash the next tick relaunches within ~1 min
//!                         (the ThrottleInterval equivalent)
//!   bootstrap / bootout → `schtasks /Change /ENABLE` + `/Run`  /  `/DISABLE` + taskkill
//!
//! Stop must DISABLE the task before killing the process — otherwise the next
//! keep-alive tick resurrects the recorder the user just paused.

use crate::agent::program_arguments;
use crate::{paths, procutil};
use std::io;

fn run_schtasks(args: &[&str]) -> io::Result<std::process::Output> {
    procutil::cmd("schtasks").args(args).output()
}

/// Turn raw schtasks stderr into a friendly message.
fn humanize_schtasks(stderr: &str) -> String {
    let s = stderr.trim();
    if s.contains("cannot find") || s.contains("does not exist") {
        "Recorder task is not installed — run Set up & start.".into()
    } else if s.contains("Access is denied") {
        "Access denied managing the recorder task.".into()
    } else if s.is_empty() {
        "schtasks failed with no message.".into()
    } else {
        format!("schtasks: {s}")
    }
}

fn ok_or_err(out: std::process::Output) -> io::Result<()> {
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(humanize_schtasks(&String::from_utf8_lossy(&out.stderr))))
    }
}

/// Pure: one cmd.exe-quoted argument.
fn q(s: &str) -> String {
    format!("\"{s}\"")
}

/// Pure: the cmd.exe line the launcher runs — recorder + args, output appended to the
/// logs (same append semantics as launchd's StandardOutPath/StandardErrorPath).
pub fn build_cmd_line(program_args: &[String], out_log: &str, err_log: &str) -> String {
    let argv: Vec<String> = program_args.iter().map(|a| q(a)).collect();
    format!("cmd /c \"{} >> {} 2>> {}\"", argv.join(" "), q(out_log), q(err_log))
}

/// Pure: VBScript that runs the recorder with no console window (style 0) and WAITS
/// for it to exit. The wait keeps the task instance "Running" for the recorder's whole
/// lifetime, which is what makes the keep-alive tick's IgnoreNew policy a no-op while
/// it is alive. (cmd is needed for the log redirection; wscript hides cmd's window.)
pub fn build_launcher_vbs(program_args: &[String], out_log: &str, err_log: &str) -> String {
    let line = build_cmd_line(program_args, out_log, err_log).replace('"', "\"\"");
    format!("CreateObject(\"WScript.Shell\").Run \"{line}\", 0, True\r\n")
}

/// Pure: minimal XML text escaping for paths/usernames embedded in the task XML.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Pure: the Task Scheduler definition. Two triggers: logon (RunAtLoad) and a repeating
/// time trigger from a fixed past boundary (KeepAlive — ticks every minute forever,
/// including right after install without a re-logon). InteractiveToken is required:
/// "run whether user is logged on or not" lands in a non-interactive session where
/// screen capture sees no desktop. Priority 7 = below-normal (the Nice 10 equivalent).
pub fn build_task_xml(launcher_vbs: &str, user: &str) -> String {
    let vbs = xml_escape(launcher_vbs);
    let user = xml_escape(user);
    // NOTE: schtasks /XML chokes on UTF-8 files ("task XML is malformed") — it expects
    // the UTF-16 format it itself exports. Declaration + file encoding must both be UTF-16
    // (see write_utf16le), and the content stays ASCII-safe.
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Meye recorder keep-alive - starts at logon and relaunches the recorder if it crashes. Managed by the Meye app.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Repetition>
        <Interval>PT1M</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
    </LogonTrigger>
    <TimeTrigger>
      <Enabled>true</Enabled>
      <StartBoundary>2020-01-01T00:00:00</StartBoundary>
      <Repetition>
        <Interval>PT1M</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>wscript.exe</Command>
      <Arguments>//B //Nologo "{vbs}"</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// "DOMAIN\user" for the task principal (USERDOMAIN is the machine name on home PCs).
fn current_user() -> String {
    let name = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(d) if !d.is_empty() => format!("{d}\\{name}"),
        _ => name,
    }
}

pub fn is_installed() -> bool {
    run_schtasks(&["/Query", "/TN", paths::TASK_NAME])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// True if the task exists and is enabled (the bootout equivalent is /DISABLE).
/// The Status column is localized ("Disabled" is English-only), so a parse miss
/// degrades to "loaded" — harmless: callers then issue a redundant enable/kill.
pub fn is_loaded() -> bool {
    let Ok(out) = run_schtasks(&["/Query", "/TN", paths::TASK_NAME, "/FO", "CSV", "/NH"]) else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    !String::from_utf8_lossy(&out.stdout).contains("\"Disabled\"")
}

/// True if a recorder process is actually alive (the tray/Start guards use this).
pub fn is_running() -> bool {
    procutil::cmd("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", paths::RECORDER_EXE), "/NH"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .to_ascii_lowercase()
                .contains(paths::RECORDER_EXE)
        })
        .unwrap_or(false)
}

/// Write the launcher VBS + create the log dir. The task XML only points at the VBS,
/// so changing record args needs no re-registration — just a recorder restart.
pub fn write_launch_config() -> io::Result<()> {
    std::fs::create_dir_all(paths::log_dir())?;
    if let Some(parent) = paths::launcher_vbs().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let vbs = build_launcher_vbs(
        &program_arguments(),
        &paths::out_log().to_string_lossy(),
        &paths::err_log().to_string_lossy(),
    );
    std::fs::write(paths::launcher_vbs(), vbs)
}

/// UTF-16 LE with BOM — the only file encoding schtasks /XML reliably accepts.
fn write_utf16le(path: &std::path::Path, s: &str) -> io::Result<()> {
    let mut bytes = vec![0xFF, 0xFE];
    for u in s.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(path, bytes)
}

fn register_task() -> io::Result<()> {
    let xml = build_task_xml(&paths::launcher_vbs().to_string_lossy(), &current_user());
    write_utf16le(&paths::task_xml(), &xml)?;
    let xml_path = paths::task_xml().to_string_lossy().into_owned();
    let out = run_schtasks(&["/Create", "/TN", paths::TASK_NAME, "/XML", &xml_path, "/F"])?;
    ok_or_err(out)
}

/// Write the launch config, register the task, and start the recorder.
pub fn install() -> io::Result<()> {
    write_launch_config()?;
    register_task()?;
    start()
}

/// No legacy plist formats on Windows; the macOS self-heal is a no-op here.
pub fn migrate_plist_format() {}

pub fn start() -> io::Result<()> {
    // Re-enable first — Stop disables the task so the keep-alive tick stays quiet.
    let out = run_schtasks(&["/Change", "/TN", paths::TASK_NAME, "/ENABLE"])?;
    ok_or_err(out)?;
    if is_running() {
        return Ok(()); // already alive; /Run would be ignored anyway (IgnoreNew)
    }
    ok_or_err(run_schtasks(&["/Run", "/TN", paths::TASK_NAME])?)
}

/// Kill any live recorder process. Exit code 128 / "not found" = nothing to kill.
/// /T also takes out children; cmd + wscript above it unwind on their own once the
/// recorder dies (they are its parents, waiting on it).
fn kill_recorder() -> io::Result<()> {
    let out = procutil::cmd("taskkill")
        .args(["/F", "/T", "/IM", paths::RECORDER_EXE])
        .output()?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("not found") || err.contains("not running") {
        Ok(()) // already stopped
    } else {
        Err(io::Error::other(format!("taskkill: {}", err.trim())))
    }
}

/// True while a task instance is mid-flight ("Running" status). Used to wait out the
/// launcher chain after a kill, so a following /Run is not swallowed by IgnoreNew.
fn instance_running() -> bool {
    run_schtasks(&["/Query", "/TN", paths::TASK_NAME, "/FO", "CSV", "/NH"])
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("\"Running\""))
        .unwrap_or(false)
}

pub fn stop() -> io::Result<()> {
    if is_installed() {
        // Disable BEFORE killing, or the next 1-minute tick relaunches the recorder.
        ok_or_err(run_schtasks(&["/Change", "/TN", paths::TASK_NAME, "/DISABLE"])?)?;
    }
    kill_recorder()
}

pub fn restart() -> io::Result<()> {
    if !is_loaded() {
        return start(); // disabled or missing — enable + run instead
    }
    kill_recorder()?;
    // Wait for the launcher chain (wscript → cmd) to unwind; while the old instance
    // still shows "Running", /Run is silently ignored under IgnoreNew.
    for _ in 0..50 {
        if !instance_running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    ok_or_err(run_schtasks(&["/Run", "/TN", paths::TASK_NAME])?)
}

/// Restart picks up the rewritten VBS — the launcher is re-read on every spawn,
/// so unlike launchd there is no separate "reload registration" step.
pub fn reload() -> io::Result<()> {
    restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_line_quotes_args_and_redirects() {
        let line = build_cmd_line(
            &["C:\\Apps\\meye-recorder.exe".into(), "record".into(), "--flag".into()],
            "C:\\logs\\out.log",
            "C:\\logs\\err.log",
        );
        assert!(line.starts_with("cmd /c \""));
        assert!(line.contains("\"C:\\Apps\\meye-recorder.exe\" \"record\" \"--flag\""));
        assert!(line.contains(">> \"C:\\logs\\out.log\""));
        assert!(line.contains("2>> \"C:\\logs\\err.log\""));
    }

    #[test]
    fn launcher_vbs_hides_window_and_waits() {
        let args: Vec<String> = vec!["C:\\x.exe".into(), "record".into()];
        let vbs = build_launcher_vbs(&args, "o.log", "e.log");
        // window style 0 (hidden), wait = True — both load-bearing (see module docs)
        assert!(vbs.ends_with(", 0, True\r\n"));
        // the embedded string is exactly the cmd line with every quote doubled for VBS
        let doubled = build_cmd_line(&args, "o.log", "e.log").replace('"', "\"\"");
        assert!(vbs.contains(&format!("Run \"{doubled}\"")));
    }

    #[test]
    fn task_xml_has_required_keys() {
        let xml = build_task_xml("C:\\meye\\launch.vbs", "PC\\mark");
        assert!(xml.contains("<LogonTrigger>")); // RunAtLoad
        assert!(xml.contains("<Interval>PT1M</Interval>")); // KeepAlive tick
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>")); // desktop access
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>")); // no 72h cutoff
        assert!(xml.contains("<Priority>7</Priority>")); // background-ish priority
        assert!(xml.contains("wscript.exe"));
        assert!(xml.contains("//B //Nologo \"C:\\meye\\launch.vbs\""));
        assert!(xml.contains("<UserId>PC\\mark</UserId>"));
        // DisallowStartIfOnBatteries defaults to true and would silently stop
        // recording on laptops — must be explicitly off.
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        // schtasks /XML rejects UTF-8 files as "malformed" — declaration must say UTF-16
        // (write_utf16le provides the matching bytes) and our own content stays ASCII.
        assert!(xml.contains("encoding=\"UTF-16\""));
        assert!(xml.is_ascii());
    }

    /// Real end-to-end setup — exactly what the dashboard's "Set up & start" runs.
    /// Ignored by default: downloads screenpipe via npx, registers the MeyeRecorder
    /// scheduled task, and starts recording. Run manually with:
    ///   cargo test --lib e2e_setup -- --ignored --nocapture
    #[test]
    #[ignore = "side effects: installs + starts the recorder task"]
    fn e2e_setup() {
        let n = crate::binary::pin().expect("pin screenpipe from npx cache");
        assert!(n > 0, "no files pinned");
        assert!(crate::binary::is_pinned(), "recorder exe missing after pin");
        install().expect("register + start the scheduled task");
        assert!(is_installed(), "task not registered");
        assert!(is_loaded(), "task not enabled");
        // give the launcher chain a moment to spawn the recorder
        for _ in 0..20 {
            if is_running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        assert!(is_running(), "recorder process not alive after start");
    }

    #[test]
    fn task_xml_escapes_specials() {
        let xml = build_task_xml("C:\\a & b\\launch.vbs", "PC\\m<rk");
        assert!(xml.contains("a &amp; b"));
        assert!(xml.contains("m&lt;rk"));
    }
}
