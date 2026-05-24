use crate::paths;
use serde_json::Value;
use std::process::Command;

/// Run `screenpipe pipe <args...>` via the pinned binary, returning stdout on success.
fn run_pipe(args: &[&str]) -> Result<String, String> {
    let out = Command::new(paths::recorder_binary())
        .arg("pipe")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(if err.trim().is_empty() { "pipe command failed".into() } else { err.trim().to_string() })
    }
}

/// List all pipes as parsed JSON (`screenpipe pipe list --json`).
pub fn list() -> Result<Value, String> {
    let stdout = run_pipe(&["list", "--json"])?;
    serde_json::from_str(&stdout).map_err(|e| format!("could not parse pipe list JSON: {e}"))
}

pub fn run_once(name: &str) -> Result<String, String> {
    run_pipe(&["run", name])
}

pub fn enable(name: &str) -> Result<String, String> {
    run_pipe(&["enable", name])
}

pub fn disable(name: &str) -> Result<String, String> {
    run_pipe(&["disable", name])
}

/// Current logs for a pipe (no follow).
pub fn logs(name: &str) -> Result<String, String> {
    run_pipe(&["logs", name])
}
