use crate::paths;
use serde_json::Value;
use std::io::Write;

/// Read the activity thread (last 300 entries), each line a JSON object.
pub fn read() -> Vec<Value> {
    let txt = std::fs::read_to_string(paths::activity_log()).unwrap_or_default();
    let mut out: Vec<Value> = txt
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let n = out.len();
    if n > 300 {
        out.drain(0..n - 300);
    }
    out
}

/// Append one entry (opaque JSON) to the thread.
pub fn append(entry: &Value) -> std::io::Result<()> {
    if let Some(p) = paths::activity_log().parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::activity_log())?;
    writeln!(f, "{}", serde_json::to_string(entry).unwrap_or_default())
}

pub fn clear() -> std::io::Result<()> {
    std::fs::write(paths::activity_log(), "")
}
