use crate::paths;
use std::io;
use std::process::Command;

/// Build the `<array>` of ProgramArguments: the pinned binary + `record`.
pub fn program_arguments() -> Vec<String> {
    vec![
        paths::pinned_binary().to_string_lossy().into_owned(),
        "record".to_string(),
    ]
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
    write_plist()?;
    start()
}

pub fn start() -> io::Result<()> {
    let plist = paths::plist_path().to_string_lossy().into_owned();
    let out = run_launchctl(&bootstrap_args(current_uid(), &plist))?;
    // bootstrap returns non-zero if already loaded; treat "already bootstrapped" as ok.
    if out.status.success() || String::from_utf8_lossy(&out.stderr).contains("already") {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

pub fn stop() -> io::Result<()> {
    let out = run_launchctl(&bootout_args(current_uid()))?;
    if out.status.success() || String::from_utf8_lossy(&out.stderr).contains("No such process") {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

pub fn restart() -> io::Result<()> {
    let out = run_launchctl(&kickstart_args(current_uid()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

/// True if `launchctl print` finds the loaded service.
pub fn is_loaded() -> bool {
    run_launchctl(&print_args(current_uid()))
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_required_keys() {
        let xml = generate_plist(&["/bin/sp".into(), "record".into()], "/tmp/o.log", "/tmp/e.log");
        assert!(xml.contains("<key>Label</key>"));
        assert!(xml.contains("com.screenpipe.keeper"));
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
        assert_eq!(bootout_args(501), vec!["bootout", "gui/501/com.screenpipe.keeper"]);
        assert_eq!(kickstart_args(501), vec!["kickstart", "-k", "gui/501/com.screenpipe.keeper"]);
        assert_eq!(print_args(501), vec!["print", "gui/501/com.screenpipe.keeper"]);
    }
}
