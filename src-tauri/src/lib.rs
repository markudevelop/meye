#[allow(dead_code)]
mod paths;
#[allow(dead_code)]
mod screenpipe_api;
#[allow(dead_code)]
mod agent;
#[allow(dead_code)]
mod health;
#[allow(dead_code)]
mod binary;
#[allow(dead_code)]
mod commands;
#[allow(dead_code)]
mod pipes;
#[allow(dead_code)]
mod chat;
#[allow(dead_code)]
mod perf;
#[allow(dead_code)]
mod activity;
#[allow(dead_code)]
mod prefs;
#[allow(dead_code)]
mod voice;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

/// Tray toggle labels — reflect whether the recorder is currently running.
const PAUSE_LABEL: &str = "Pause recording";
const RESUME_LABEL: &str = "Resume recording";

/// Managed handle to the tray icon so commands can show/hide it (discreet mode).
pub struct AppTray(pub tauri::tray::TrayIcon);

/// Apply discreet mode: hide/show Meye's own dock icon + tray icon. Never touches the
/// macOS recording indicator.
pub fn apply_discreet(app: &tauri::AppHandle, on: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if on {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        let _ = app.set_activation_policy(policy);
    }
    if let Some(tray) = app.try_state::<AppTray>() {
        let _ = tray.0.set_visible(!on);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_health,
            commands::setup,
            commands::start,
            commands::stop,
            commands::restart,
            commands::update_screenpipe,
            commands::open_data_dir,
            commands::open_logs,
            commands::tail_logs,
            commands::get_permissions,
            commands::open_settings,
            commands::recheck,
            commands::api_search,
            commands::api_keyword,
            commands::api_audio_devices,
            commands::api_monitors,
            commands::api_frame_ocr,
            commands::api_audio_start,
            commands::api_audio_stop,
            commands::api_raw_sql,
            commands::api_add_tags,
            commands::api_remove_tags,
            commands::api_retention_status,
            commands::api_retention_configure,
            commands::api_storage_preview,
            commands::api_delete_range,
            commands::api_pipe_list,
            commands::api_pipe_run,
            commands::api_pipe_enable,
            commands::api_pipe_disable,
            commands::api_pipe_logs,
            commands::api_pipe_set_schedule,
            commands::api_models_list,
            commands::api_models_create,
            commands::api_models_set_default,
            commands::api_models_delete,
            commands::api_pipe_set_preset,
            commands::api_pipe_config_read,
            commands::api_pipe_config_write,
            commands::api_registry_search,
            commands::api_registry_info,
            commands::api_registry_install,
            commands::api_pipe_delete,
            commands::api_chat,
            commands::api_open_pipe_dir,
            commands::api_get_record_args,
            commands::api_set_record_args,
            commands::api_perf_stats,
            commands::api_activity_read,
            commands::api_activity_append,
            commands::api_activity_clear,
            commands::api_convo_list,
            commands::api_convo_read,
            commands::api_convo_append,
            commands::api_convo_delete,
            commands::api_convo_archive,
            commands::api_convo_list_archived,
            commands::api_convo_unarchive,
            commands::api_get_discreet,
            commands::api_set_discreet,
            commands::api_parse_voice_command,
        ])
        .setup(|app| {
            // Upgrade old plists in-place before anything else touches the agent.
            agent::migrate_plist_format();

            // Minimal tray: open the app, one state-aware recording toggle, quit.
            // Everything else (logs, data folder, update) lives in the Dashboard.
            let open_i = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
            let toggle_label = if agent::is_loaded() { PAUSE_LABEL } else { RESUME_LABEL };
            let toggle_i = MenuItem::with_id(app, "toggle", toggle_label, true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_i, &toggle_i, &sep, &quit_i])?;

            // Clone the toggle into the handler so it can flip its own label after acting.
            let toggle_handle = toggle_i.clone();
            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "toggle" => {
                        if agent::is_loaded() {
                            let _ = agent::stop();
                            let _ = toggle_handle.set_text(RESUME_LABEL);
                        } else {
                            let _ = agent::start();
                            let _ = toggle_handle.set_text(PAUSE_LABEL);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(AppTray(tray));

            // Restore discreet mode from the saved preference at launch.
            apply_discreet(&app.handle().clone(), prefs::get_discreet());

            // Always show the window when the app is launched. The window is configured
            // hidden (no white flash on boot) and is normally revealed by the tray; but in
            // discreet mode there is no tray, so launching the app from Applications/Spotlight
            // is the way back in — it must reveal the window or the user is locked out.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }

            // Health poll loop: emit status to the frontend every 5s.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut prev_running = false;
                loop {
                    let status = if !agent::is_installed() {
                        health::Status::NotInstalled
                    } else {
                        match health::fetch_full().await {
                            Some(h) => health::classify(&h),
                            None => {
                                if agent::is_loaded() && !agent::missing_permissions().is_empty() {
                                    health::Status::WaitingPermissions
                                } else {
                                    health::Status::Down
                                }
                            }
                        }
                    };
                    // On the edge into "running" (app launch, or a recorder restart while
                    // the dashboard is open), re-arm any scheduled pipe that drifted to
                    // disabled so scheduled agents keep firing. Fire-and-forget; never
                    // blocks the status emit. Manual pipes are left untouched.
                    let running =
                        matches!(status, health::Status::Healthy | health::Status::Degraded);
                    if running && !prev_running {
                        let _ = tauri::async_runtime::spawn_blocking(|| {
                            let _ = crate::pipes::reassert_scheduled();
                        });
                    }
                    prev_running = running;
                    let _ = handle.emit("status", &status);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // macOS "reopen" (clicking the dock icon, or `open -a Meye` while it's already
            // running) — re-show the window. Critical for getting back in from discreet mode.
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            // Menu-bar behavior: closing the window hides it; the app stays in the tray.
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            {
                if prefs::get_discreet() {
                    // No tray to restore from in discreet mode — closing fully quits the GUI.
                    // Recording is a separate LaunchAgent, so it keeps running. Reopen from Applications.
                    app.exit(0);
                } else {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                    api.prevent_close();
                }
            }
        });
}
