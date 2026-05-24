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
pub async fn setup() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        binary::pin().map_err(|e| e.to_string())?;
        agent::install().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
pub async fn update_screenpipe() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(|| binary::update().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
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

#[derive(serde::Serialize)]
pub struct Permissions {
    pub waiting: Vec<String>,
}

#[tauri::command]
pub fn get_permissions() -> Permissions {
    Permissions { waiting: agent::missing_permissions() }
}

#[tauri::command]
pub fn open_settings(pane: String) -> Result<(), String> {
    let url = match pane.as_str() {
        "screen" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        _ => "x-apple.systempreferences:com.apple.preference.security?Privacy",
    };
    open_path(url)
}

#[tauri::command]
pub fn recheck() -> Result<(), String> {
    agent::restart().map_err(|e| e.to_string())
}

use crate::screenpipe_api::{self, SearchParams};
use serde_json::Value;

#[tauri::command]
pub async fn api_search(params: SearchParams) -> Result<Value, String> {
    screenpipe_api::search(&params).await
}

#[tauri::command]
pub async fn api_keyword(query: String, limit: u32) -> Result<Value, String> {
    screenpipe_api::keyword(&query, limit).await
}

#[tauri::command]
pub async fn api_audio_devices() -> Result<Value, String> {
    screenpipe_api::audio_devices().await
}

#[tauri::command]
pub async fn api_monitors() -> Result<Value, String> {
    screenpipe_api::monitors().await
}

#[tauri::command]
pub async fn api_frame_ocr(id: i64) -> Result<Value, String> {
    screenpipe_api::frame_ocr(id).await
}

#[tauri::command]
pub async fn api_audio_start() -> Result<Value, String> {
    screenpipe_api::audio_start().await
}

#[tauri::command]
pub async fn api_audio_stop() -> Result<Value, String> {
    screenpipe_api::audio_stop().await
}

#[tauri::command]
pub async fn api_raw_sql(query: String) -> Result<Value, String> {
    screenpipe_api::raw_sql(&query).await
}

#[tauri::command]
pub async fn api_add_tags(kind: String, id: i64, tags: Vec<String>) -> Result<Value, String> {
    screenpipe_api::add_tags(&kind, id, tags).await
}

#[tauri::command]
pub async fn api_remove_tags(kind: String, id: i64, tags: Vec<String>) -> Result<Value, String> {
    screenpipe_api::remove_tags(&kind, id, tags).await
}

#[tauri::command]
pub async fn api_pipe_list() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(crate::pipes::list)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_run(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::run_once(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_enable(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::enable(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_disable(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::disable(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_logs(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::logs(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_models_list() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(crate::pipes::models_list)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_models_create(
    id: String,
    provider: String,
    model: String,
    url: Option<String>,
    api_key: Option<String>,
    set_default: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::pipes::models_create(
            &id,
            &provider,
            &model,
            url.as_deref(),
            api_key.as_deref(),
            set_default,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_models_set_default(id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::models_set_default(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_models_delete(id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::models_delete(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_set_preset(name: String, presets: Vec<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::set_preset(&name, &presets))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn api_pipe_config_read(name: String) -> Result<String, String> {
    crate::pipes::config_read(&name)
}

#[tauri::command]
pub fn api_pipe_config_write(name: String, content: String) -> Result<(), String> {
    crate::pipes::config_write(&name, &content)
}

#[tauri::command]
pub async fn api_registry_search(query: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::registry_search(&query))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_registry_info(slug: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::registry_info(&slug))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_registry_install(source: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::install(&source))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_pipe_delete(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pipes::delete(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_chat(question: String) -> Result<crate::chat::ChatReply, String> {
    crate::chat::chat(&question).await
}

#[tauri::command]
pub fn api_open_pipe_dir(name: String) -> Result<(), String> {
    let dir = crate::pipes::pipe_dir(&name)?;
    open_path(&dir.to_string_lossy())
}

#[tauri::command]
pub fn api_get_record_args() -> Vec<String> {
    crate::agent::get_record_args()
}

#[tauri::command]
pub async fn api_set_record_args(args: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::agent::set_record_args(&args).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn api_perf_stats() -> Value {
    tauri::async_runtime::spawn_blocking(crate::perf::stats)
        .await
        .unwrap_or_else(|_| serde_json::json!({}))
}

#[tauri::command]
pub fn api_activity_read() -> Vec<Value> {
    crate::activity::read()
}

#[tauri::command]
pub fn api_activity_append(entry: Value) -> Result<(), String> {
    crate::activity::append(&entry).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn api_activity_clear() -> Result<(), String> {
    crate::activity::clear().map_err(|e| e.to_string())
}
