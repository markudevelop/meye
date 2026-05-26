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

/// Pure: turn a run-log filename ("20260525_085953.json") into a readable
/// "YYYY-MM-DD HH:MM" stamp. screenpipe names each run log by its start time.
pub fn parse_log_stamp(filename: &str) -> Option<String> {
    let stem = filename.strip_suffix(".json").unwrap_or(filename);
    let b = stem.as_bytes();
    // Expect YYYYMMDD_HHMMSS
    if stem.len() >= 15 && b.get(8) == Some(&b'_') && stem[..8].bytes().all(|c| c.is_ascii_digit()) {
        let d = &stem[0..8];
        let t = &stem[9..15];
        Some(format!("{}-{}-{} {}:{}", &d[0..4], &d[4..6], &d[6..8], &t[0..2], &t[2..4]))
    } else {
        None
    }
}

/// The most recent run time for a pipe, derived from its `logs/` directory — screenpipe's
/// `pipe list` JSON reports `last_run: null` even for pipes that run on schedule, so we read
/// the newest timestamped run log instead.
fn last_run_from_logs(name: &str) -> Option<String> {
    let dir = pipe_dir(name).ok()?.join("logs");
    let newest = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .max()?; // filename is YYYYMMDD_HHMMSS → lexicographic order == chronological
    parse_log_stamp(&newest)
}

/// List all pipes as parsed JSON (`screenpipe pipe list --json`), enriching `last_run` from
/// each pipe's run logs when screenpipe reports it as null.
pub fn list() -> Result<Value, String> {
    let stdout = run_pipe(&["list", "--json"])?;
    let mut v: Value =
        serde_json::from_str(&stdout).map_err(|e| format!("could not parse pipe list JSON: {e}"))?;
    if let Some(arr) = v.as_array_mut() {
        for p in arr.iter_mut() {
            let needs = p.get("last_run").map(|x| x.is_null()).unwrap_or(true);
            if !needs {
                continue;
            }
            if let Some(name) = p.pointer("/config/name").and_then(|n| n.as_str()).map(String::from) {
                if let Some(stamp) = last_run_from_logs(&name) {
                    p["last_run"] = Value::String(stamp);
                }
            }
        }
    }
    Ok(v)
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

/// Full preset config incl. raw api key (`pipe models show <id> --json`).
pub fn models_show(id: &str) -> Result<Value, String> {
    let stdout = run_pipe(&["models", "show", id, "--json"])?;
    serde_json::from_str(&stdout).map_err(|e| format!("could not parse models show JSON: {e}"))
}

pub fn set_preset(name: &str, presets: &[String]) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["set-preset", name];
    for p in presets {
        args.push(p.as_str());
    }
    run_pipe(&args)
}

/// Resolve `~/.screenpipe/pipes/<name>`, rejecting names that could escape the dir.
pub fn pipe_dir(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("invalid pipe name".into());
    }
    Ok(paths::data_dir().join("pipes").join(name))
}

fn pipe_md_path(name: &str) -> Result<PathBuf, String> {
    Ok(pipe_dir(name)?.join("pipe.md"))
}

pub fn config_read(name: &str) -> Result<String, String> {
    std::fs::read_to_string(pipe_md_path(name)?).map_err(|e| e.to_string())
}

pub fn config_write(name: &str, content: &str) -> Result<(), String> {
    std::fs::write(pipe_md_path(name)?, content).map_err(|e| e.to_string())
}

/// Pure: rewrite the `schedule:` value inside a pipe.md's YAML frontmatter,
/// preserving everything else (other frontmatter keys, the body). If the
/// frontmatter has no `schedule:` line, one is inserted just before the closing
/// fence. If there is no (valid) frontmatter at all, a minimal one is prepended.
/// A `schedule:` occurring in the body (after the closing fence) is never touched.
pub fn rewrite_schedule(md: &str, schedule: &str) -> String {
    // The value MUST be double-quoted: cron macros (`@daily`) and raw cron
    // expressions (`*/30 * * * *`) start with YAML indicator characters (`@`, `*`)
    // and break the frontmatter parser when left bare — which silently drops the
    // pipe from the registry. Quoting makes every schedule string YAML-safe.
    let new_line = format!("schedule: \"{}\"", schedule.replace('"', "\\\""));
    let starts_fm = md.starts_with("---\n") || md.starts_with("---\r\n");
    if !starts_fm {
        return format!("---\n{new_line}\nenabled: true\n---\n\n{md}");
    }
    let trailing_nl = md.ends_with('\n');
    let mut out: Vec<String> = Vec::new();
    let mut in_fm = false;
    let mut fm_closed = false;
    let mut replaced = false;
    for (i, line) in md.lines().enumerate() {
        if i == 0 {
            out.push(line.to_string()); // opening "---"
            in_fm = true;
            continue;
        }
        if in_fm && line.trim() == "---" {
            if !replaced {
                out.push(new_line.clone()); // no schedule key existed — insert before fence
            }
            out.push(line.to_string());
            in_fm = false;
            fm_closed = true;
            continue;
        }
        if in_fm && line.trim_start().starts_with("schedule:") {
            out.push(new_line.clone());
            replaced = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !fm_closed {
        // frontmatter never closed — treat as malformed, prepend a fresh one
        return format!("---\n{new_line}\nenabled: true\n---\n\n{md}");
    }
    let mut s = out.join("\n");
    if trailing_nl {
        s.push('\n');
    }
    s
}

/// True if the named pipe is currently enabled (reads `pipe list --json`).
fn is_enabled(name: &str) -> Result<bool, String> {
    let v = list()?;
    let enabled = v
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|p| p.pointer("/config/name").and_then(|n| n.as_str()) == Some(name))
        })
        .and_then(|p| p.pointer("/config/enabled").and_then(|e| e.as_bool()))
        .unwrap_or(false);
    Ok(enabled)
}

/// Set a pipe's schedule by editing its pipe.md frontmatter, then force the
/// running scheduler to re-read it via disable+enable (only when the pipe is
/// enabled, so we never silently turn a disabled pipe back on).
pub fn set_schedule(name: &str, schedule: &str) -> Result<(), String> {
    let md = config_read(name)?;
    config_write(name, &rewrite_schedule(&md, schedule))?;
    if is_enabled(name).unwrap_or(false) {
        let _ = disable(name);
        let _ = enable(name);
    }
    Ok(())
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

    #[test]
    fn parse_log_stamp_formats_run_filename() {
        assert_eq!(parse_log_stamp("20260525_085953.json").as_deref(), Some("2026-05-25 08:59"));
        assert_eq!(parse_log_stamp("20260101_000000.json").as_deref(), Some("2026-01-01 00:00"));
        assert_eq!(parse_log_stamp("not-a-stamp.json"), None);
        assert_eq!(parse_log_stamp("README.md"), None);
    }

    #[test]
    fn rewrite_schedule_replaces_existing_line_and_quotes() {
        let md = "---\nschedule: manual\nenabled: true\ntitle: AI Habits\n---\n\nBody text here.\n";
        let out = rewrite_schedule(md, "every 30m");
        assert!(out.contains("schedule: \"every 30m\""));
        assert!(!out.contains("schedule: manual"));
        assert!(out.contains("enabled: true"));
        assert!(out.contains("title: AI Habits"));
        assert!(out.ends_with("Body text here.\n"));
        // exactly one schedule line
        assert_eq!(out.matches("schedule:").count(), 1);
    }

    #[test]
    fn rewrite_schedule_quotes_cron_macros() {
        // bare `@daily` / `*/30 ...` break YAML — must be quoted.
        let md = "---\nschedule: manual\n---\n\nBody.\n";
        assert!(rewrite_schedule(md, "@daily").contains("schedule: \"@daily\""));
        assert!(rewrite_schedule(md, "*/30 * * * *").contains("schedule: \"*/30 * * * *\""));
    }

    #[test]
    fn rewrite_schedule_inserts_when_missing() {
        let md = "---\nenabled: true\ntitle: X\n---\n\nBody.\n";
        let out = rewrite_schedule(md, "@daily");
        assert!(out.contains("schedule: \"@daily\""));
        assert!(out.contains("enabled: true"));
        assert!(out.contains("title: X"));
        // inserted before the closing fence, body intact
        assert!(out.ends_with("Body.\n"));
    }

    #[test]
    fn rewrite_schedule_prepends_when_no_frontmatter() {
        let md = "Just a body, no frontmatter.\n";
        let out = rewrite_schedule(md, "every 1h");
        assert!(out.starts_with("---\nschedule: \"every 1h\"\nenabled: true\n---\n\n"));
        assert!(out.ends_with("Just a body, no frontmatter.\n"));
    }

    #[test]
    fn rewrite_schedule_ignores_schedule_word_in_body() {
        let md = "---\nschedule: manual\n---\n\nMy schedule: do not touch this line.\n";
        let out = rewrite_schedule(md, "@hourly");
        assert!(out.contains("schedule: \"@hourly\""));
        assert!(out.contains("My schedule: do not touch this line."));
        // frontmatter schedule replaced, body schedule untouched => 2 total
        assert_eq!(out.matches("schedule:").count(), 2);
    }
}
