use crate::paths;
use serde_json::Value;
use std::path::PathBuf;
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

pub fn models_list() -> Result<Value, String> {
    let stdout = run_pipe(&["models", "list", "--json"])?;
    serde_json::from_str(&stdout).map_err(|e| format!("could not parse models list JSON: {e}"))
}

pub fn models_create(
    id: &str,
    provider: &str,
    model: &str,
    url: Option<&str>,
    api_key: Option<&str>,
    set_default: bool,
) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["models", "create", id, "--provider", provider, "--model", model];
    if let Some(u) = url {
        if !u.is_empty() {
            args.push("--url");
            args.push(u);
        }
    }
    if let Some(k) = api_key {
        if !k.is_empty() {
            args.push("--api-key");
            args.push(k);
        }
    }
    if set_default {
        args.push("--set-default");
    }
    run_pipe(&args)
}

pub fn models_set_default(id: &str) -> Result<String, String> {
    run_pipe(&["models", "set-default", id])
}

pub fn models_delete(id: &str) -> Result<String, String> {
    run_pipe(&["models", "delete", id, "--force"])
}

pub fn set_preset(name: &str, presets: &[String]) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["set-preset", name];
    for p in presets {
        args.push(p.as_str());
    }
    run_pipe(&args)
}

/// Resolve `~/.screenpipe/pipes/<name>/pipe.md`, rejecting names that could escape the dir.
fn pipe_md_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("invalid pipe name".into());
    }
    Ok(paths::data_dir().join("pipes").join(name).join("pipe.md"))
}

pub fn config_read(name: &str) -> Result<String, String> {
    std::fs::read_to_string(pipe_md_path(name)?).map_err(|e| e.to_string())
}

pub fn config_write(name: &str, content: &str) -> Result<(), String> {
    std::fs::write(pipe_md_path(name)?, content).map_err(|e| e.to_string())
}

/// Pure: parse `screenpipe pipe search` text table (cols separated by 2+ spaces)
/// into `[{slug, category, installs, description}]`.
pub fn parse_search_table(stdout: &str) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("SLUG")
            || trimmed.starts_with("---")
            || trimmed.ends_with("found")
        {
            continue;
        }
        let cols: Vec<&str> = trimmed
            .split("  ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if cols.len() < 2 {
            continue;
        }
        rows.push(serde_json::json!({
            "slug": cols.first().copied().unwrap_or(""),
            "category": cols.get(1).copied().unwrap_or(""),
            "installs": cols.get(2).copied().unwrap_or(""),
            "description": cols.get(3..).map(|r| r.join(" ")).unwrap_or_default(),
        }));
    }
    Value::Array(rows)
}

/// Search the pipe registry (`pipe search <query>`), returned as parsed JSON.
pub fn registry_search(query: &str) -> Result<Value, String> {
    let stdout = run_pipe(&["search", query])?;
    Ok(parse_search_table(&stdout))
}

/// Registry detail for a slug (`pipe info <slug>`), raw text.
pub fn registry_info(slug: &str) -> Result<String, String> {
    run_pipe(&["info", slug])
}

/// Install a pipe from a slug, local path, or URL (`pipe install <source>`).
pub fn install(source: &str) -> Result<String, String> {
    run_pipe(&["install", source])
}

/// Delete an installed pipe (`pipe delete <name>`).
pub fn delete(name: &str) -> Result<String, String> {
    run_pipe(&["delete", name])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_md_path_rejects_traversal_and_accepts_valid() {
        assert!(pipe_md_path("obsidian-sync").is_ok());
        assert!(pipe_md_path("a_b-1").is_ok());
        assert!(pipe_md_path("../etc").is_err());
        assert!(pipe_md_path("a/b").is_err());
        assert!(pipe_md_path("").is_err());
        let p = pipe_md_path("obsidian-sync").unwrap();
        assert!(p.ends_with("pipes/obsidian-sync/pipe.md"));
    }

    #[test]
    fn parse_search_table_extracts_rows() {
        let sample = "SLUG                           CATEGORY        INSTALLS   DESCRIPTION\n\
-----------------------------------------------------------------\n\
wisprflow-sync                 productivity    45         Synchronize Wisprflow notes, record...\n\
notion-crm-sync                productivity    25         Auto-detect business calls and sync...\n\
\n\
2 pipe(s) found\n";
        let v = parse_search_table(sample);
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["slug"], "wisprflow-sync");
        assert_eq!(arr[0]["category"], "productivity");
        assert_eq!(arr[0]["installs"], "45");
        assert!(arr[0]["description"].as_str().unwrap().contains("Synchronize"));
    }
}
