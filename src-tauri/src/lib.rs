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
#[allow(dead_code)]
mod procutil;

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

/// Apply discreet mode: hide/show Meye's own taskbar/dock + tray presence. Never touches the
/// OS recording indicator, and never hides the *process* (that stays in Task Manager — hiding a
/// running process is malware behaviour, not what this does). The recorder + automations keep
/// running regardless; only Meye's visible GUI footprint changes.
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
    // Windows: drop the taskbar button (the dock-hide equivalent). Previously discreet mode
    // only hid the tray here, so the taskbar entry stayed and it looked like nothing happened.
    #[cfg(windows)]
    {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_skip_taskbar(on);
        }
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
            commands::api_get_remote_enabled,
            commands::api_set_remote_enabled,
            commands::api_remote_pairing,
            commands::api_remote_latest,
            commands::api_remote_audio,
            commands::api_get_obsidian_vault,
            commands::api_set_obsidian_vault,
            commands::api_remote_frame,
            commands::api_remote_comment,
        ])
        .setup(|app| {
            // Upgrade old plists in-place before anything else touches the agent.
            agent::migrate_plist_format();
            // Pre-rebrand installs have `screenpipe` instead of `meye-recorder` in the
            // pinned bundle; the rewritten plist then points at a path that does not
            // exist and launchd wedges with EX_CONFIG. Rename + re-sign so they match.
            binary::migrate_binary_name();
            // Auto-resume the recorder on app launch. `open_settings` intentionally
            // stops the agent so the TCC prompt does not race the Settings window, and
            // a previous Stop click leaves it bootout'd too. Without this, users have
            // to click Start every time they open the app — which they do not.
            if agent::is_installed() && binary::is_pinned() {
                let _ = agent::start();
            }

            // Seed bundled default pipes into ~/.screenpipe/pipes (idempotent; never clobbers an
            // existing pipe). Lets a fresh install — e.g. a second machine — get the same set of
            // automations instead of an empty Pipes tab. obsidian-sync's {{VAULT}} placeholder is
            // resolved to the configured (or per-OS default) Obsidian vault path.
            if let Ok(res) = app.path().resource_dir() {
                let _ = pipes::seed_defaults(
                    &res.join("pipes"),
                    &paths::data_dir().join("pipes"),
                    &prefs::get_obsidian_vault(),
                );
            }

            // Minimal tray: open the app, one state-aware recording toggle, quit.
            // Everything else (logs, data folder, update) lives in the Dashboard.
            let open_i = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
            // Initial label: only show "Pause" if launchd has the agent AND the recorder
            // process is actually alive. `is_loaded()` alone lies after EX_CONFIG limbo or
            // a screenpipe self-exit — the service is registered but no process is running.
            let toggle_label = if agent::is_running() { PAUSE_LABEL } else { RESUME_LABEL };
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
            let tray_toggle = toggle_i.clone();
            tauri::async_runtime::spawn(async move {
                let mut prev_running = false;
                let mut prev_alive = false;
                loop {
                    let status = if !agent::is_installed() {
                        health::Status::NotInstalled
                    } else {
                        match health::fetch_full().await {
                            Some(h) => health::classify(&h),
                            None => {
                                if agent::is_loaded() && !agent::missing_permissions().is_empty() {
                                    health::Status::WaitingPermissions
                                } else if agent::is_running() {
                                    // Process alive but API not up: booting / first-run
                                    // downloads. Without this the dashboard claims
                                    // "Stopped" for minutes right after Start.
                                    health::Status::Starting
                                } else {
                                    health::Status::Down
                                }
                            }
                        }
                    };
                    // On the edge into "API up" (app launch, or a recorder restart while
                    // the dashboard is open), re-arm any scheduled pipe that drifted to
                    // disabled so scheduled agents keep firing. Fire-and-forget; never
                    // blocks the status emit. Manual pipes are left untouched. (Starting
                    // doesn't count — pipe CLI calls need the API.)
                    let running =
                        matches!(status, health::Status::Healthy | health::Status::Degraded);
                    if running && !prev_running {
                        let _ = tauri::async_runtime::spawn_blocking(|| {
                            let _ = crate::pipes::reassert_scheduled();
                        });
                    }
                    prev_running = running;
                    // Keep tray toggle label honest: "Pause" whenever a recorder process is
                    // actually alive (including boot). Edge-only so we are not re-setting
                    // the same string.
                    let alive = running || matches!(status, health::Status::Starting);
                    if alive != prev_alive {
                        let _ = tray_toggle.set_text(if alive { PAUSE_LABEL } else { RESUME_LABEL });
                    }
                    prev_alive = alive;
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
            #[cfg(target_os = "macos")]
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
