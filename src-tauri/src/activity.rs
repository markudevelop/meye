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

// ---------- multi-conversation store ----------

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn convo_path(id: &str) -> Option<std::path::PathBuf> {
    if valid_id(id) {
        Some(paths::convos_dir().join(format!("{id}.jsonl")))
    } else {
        None
    }
}

/// One-time: if there are no conversations yet but a legacy single thread exists, import it.
fn migrate_legacy() {
    let dir = paths::convos_dir();
    let has_any = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x == "jsonl")))
        .unwrap_or(false);
    if has_any {
        return;
    }
    if let Ok(txt) = std::fs::read_to_string(paths::activity_log()) {
        if !txt.trim().is_empty() {
            let _ = std::fs::create_dir_all(&dir);
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
                .to_string();
            let _ = std::fs::write(dir.join(format!("{id}.jsonl")), txt);
        }
    }
}

/// List conversations as `{ id, title, count }`, newest first.
pub fn convo_list() -> Vec<Value> {
    migrate_legacy();
    let mut out: Vec<(std::time::SystemTime, Value)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(paths::convos_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.extension().is_some_and(|x| x == "jsonl") {
                continue;
            }
            let id = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let txt = std::fs::read_to_string(&p).unwrap_or_default();
            let entries: Vec<Value> = txt
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .collect();
            let title = entries
                .iter()
                .find(|en| en.get("kind").and_then(|k| k.as_str()) == Some("user"))
                .and_then(|en| en.get("text").and_then(|t| t.as_str()))
                .map(|t| t.chars().take(48).collect::<String>())
                .unwrap_or_else(|| "New chat".into());
            let updated = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            let updated_ms = updated
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            out.push((
                updated,
                serde_json::json!({ "id": id, "title": title, "count": entries.len(), "updated": updated_ms }),
            ));
        }
    }
    out.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    out.into_iter().map(|(_, v)| v).collect()
}

pub fn convo_read(id: &str) -> Vec<Value> {
    let Some(p) = convo_path(id) else {
        return Vec::new();
    };
    std::fs::read_to_string(&p)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

pub fn convo_append(id: &str, entry: &Value) -> std::io::Result<()> {
    let Some(p) = convo_path(id) else {
        return Err(std::io::Error::other("invalid conversation id"));
    };
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
    writeln!(f, "{}", serde_json::to_string(entry).unwrap_or_default())
}

pub fn convo_delete(id: &str) -> std::io::Result<()> {
    let Some(p) = convo_path(id) else {
        return Err(std::io::Error::other("invalid conversation id"));
    };
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

/// Archive (move to `conversations/archived/`) — hidden from the list but not destroyed.
pub fn convo_archive(id: &str) -> std::io::Result<()> {
    let Some(p) = convo_path(id) else {
        return Err(std::io::Error::other("invalid conversation id"));
    };
    let arch = paths::convos_dir().join("archived");
    std::fs::create_dir_all(&arch)?;
    if p.exists() {
        std::fs::rename(&p, arch.join(format!("{id}.jsonl")))?;
    }
    Ok(())
}
