use crate::paths;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

fn recorder_pid() -> Option<u32> {
    let out = Command::new("pgrep")
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

/// Disk usage of a path in MB (via `du -sk`).
fn du_mb(path: &Path) -> u64 {
    Command::new("du")
        .arg("-sk")
        .arg(path)
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
        })
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

/// Recorder CPU% + RAM, and screenpipe storage sizes.
pub fn stats() -> Value {
    let pid = recorder_pid();
    let (mut cpu, mut rss_mb) = (0.0_f64, 0_u64);
    if let Some(pid) = pid {
        if let Ok(o) = Command::new("ps")
            .args(["-o", "%cpu=", "-o", "rss=", "-p", &pid.to_string()])
            .output()
        {
            let s = String::from_utf8_lossy(&o.stdout);
            let mut it = s.split_whitespace();
            cpu = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
            rss_mb = it
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .map(|kb| kb / 1024)
                .unwrap_or(0);
        }
    }
    json!({
        "running": pid.is_some(),
        "cpu": cpu,
        "rss_mb": rss_mb,
        "db_mb": du_mb(&paths::data_dir().join("db.sqlite")),
        "data_mb": du_mb(&paths::data_dir().join("data")),
    })
}
