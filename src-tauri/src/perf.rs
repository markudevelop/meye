use crate::{paths, procutil};
use serde_json::{json, Value};
use std::path::Path;

#[cfg(not(windows))]
fn recorder_pid() -> Option<u32> {
    let out = procutil::cmd("pgrep")
        .arg("-f")
        .arg(paths::recorder_binary().to_string_lossy().as_ref())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(windows)]
fn recorder_pid() -> Option<u32> {
    // CSV row: "meye-recorder.exe","1234","Console","1","123,456 K"
    let out = procutil::cmd("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", paths::RECORDER_EXE), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.to_ascii_lowercase().contains("meye-recorder"))?;
    line.split(',').nth(1)?.trim().trim_matches('"').parse().ok()
}

/// Recorder CPU% (ps-style: % of one core, can exceed 100) + resident memory in MB.
#[cfg(not(windows))]
fn cpu_rss(pid: u32) -> (f64, u64) {
    let Ok(o) = procutil::cmd("ps")
        .args(["-o", "%cpu=", "-o", "rss=", "-p", &pid.to_string()])
        .output()
    else {
        return (0.0, 0);
    };
    let s = String::from_utf8_lossy(&o.stdout);
    let mut it = s.split_whitespace();
    let cpu = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let rss_mb = it
        .next()
        .and_then(|x| x.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0);
    (cpu, rss_mb)
}

#[cfg(windows)]
fn cpu_rss(_pid: u32) -> (f64, u64) {
    // Win32_PerfFormattedData gives an instantaneous CPU% like `ps`; WorkingSet is bytes.
    // LIKE matches "meye-recorder" and "meye-recorder#1" (perf names drop the .exe).
    let Ok(o) = procutil::cmd("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter \"Name LIKE 'meye-recorder%'\" | Select-Object PercentProcessorTime,WorkingSet | ConvertTo-Json",
        ])
        .output()
    else {
        return (0.0, 0);
    };
    let v: Value = match serde_json::from_slice(&o.stdout) {
        Ok(v) => v,
        Err(_) => return (0.0, 0),
    };
    // single match → object; multiple → array (take the first)
    let item = v.as_array().and_then(|a| a.first()).unwrap_or(&v);
    let cpu = item.get("PercentProcessorTime").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let rss_mb = item
        .get("WorkingSet")
        .and_then(|x| x.as_u64())
        .map(|b| b / (1024 * 1024))
        .unwrap_or(0);
    (cpu, rss_mb)
}

/// Total size of a file or directory tree, in bytes (the portable `du`).
fn size_bytes(path: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|e| size_bytes(&e.path()))
        .sum()
}

fn du_mb(path: &Path) -> u64 {
    size_bytes(path) / (1024 * 1024)
}

/// Recorder CPU% + RAM, and screenpipe storage sizes.
pub fn stats() -> Value {
    let pid = recorder_pid();
    let (cpu, rss_mb) = pid.map(cpu_rss).unwrap_or((0.0, 0));
    json!({
        "running": pid.is_some(),
        "cpu": cpu,
        "rss_mb": rss_mb,
        "db_mb": du_mb(&paths::data_dir().join("db.sqlite")),
        "data_mb": du_mb(&paths::data_dir().join("data")),
    })
}
