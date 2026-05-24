use crate::{agent, binary, health, paths};

#[derive(serde::Serialize)]
pub struct AppState {
    pub installed: bool,
    pub pinned: bool,
    pub loaded: bool,
}

#[tauri::command]
pub fn get_state() -> AppState {
    AppState {
        installed: agent::is_installed(),
        pinned: binary::is_pinned(),
        loaded: agent::is_loaded(),
    }
}

#[tauri::command]
pub async fn get_health() -> Option<health::Health> {
    health::fetch_full().await
}

#[tauri::command]
pub fn setup() -> Result<(), String> {
    binary::pin().map_err(|e| e.to_string())?;
    agent::install().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start() -> Result<(), String> {
    agent::start().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop() -> Result<(), String> {
    agent::stop().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restart() -> Result<(), String> {
    agent::restart().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_screenpipe() -> Result<usize, String> {
    binary::update().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_data_dir() -> Result<(), String> {
    open_path(&paths::data_dir().to_string_lossy())
}

#[tauri::command]
pub fn open_logs() -> Result<(), String> {
    open_path(&paths::log_dir().to_string_lossy())
}

#[tauri::command]
pub fn tail_logs(lines: usize) -> String {
    let read_tail = |p: std::path::PathBuf| -> String {
        std::fs::read_to_string(&p)
            .map(|s| {
                s.lines()
                    .rev()
                    .take(lines)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    };
    format!(
        "=== stderr ===\n{}\n\n=== stdout ===\n{}",
        read_tail(paths::err_log()),
        read_tail(paths::out_log())
    )
}

fn open_path(path: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| if s.success() { Ok(()) } else { Err("open failed".into()) })
}
