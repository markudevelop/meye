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

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

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
            commands::api_pipe_list,
            commands::api_pipe_run,
            commands::api_pipe_enable,
            commands::api_pipe_disable,
            commands::api_pipe_logs,
        ])
        .setup(|app| {
            let open_i = MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
            let start_i = MenuItem::with_id(app, "start", "Start", true, None::<&str>)?;
            let stop_i = MenuItem::with_id(app, "stop", "Stop", true, None::<&str>)?;
            let restart_i = MenuItem::with_id(app, "restart", "Restart", true, None::<&str>)?;
            let data_i = MenuItem::with_id(app, "data", "Open Data Folder", true, None::<&str>)?;
            let logs_i = MenuItem::with_id(app, "logs", "Open Logs", true, None::<&str>)?;
            let update_i = MenuItem::with_id(app, "update", "Update screenpipe", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open_i, &start_i, &stop_i, &restart_i, &data_i, &logs_i, &update_i, &quit_i],
            )?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "start" => {
                        let _ = agent::start();
                    }
                    "stop" => {
                        let _ = agent::stop();
                    }
                    "restart" => {
                        let _ = agent::restart();
                    }
                    "data" => {
                        let _ = commands::open_data_dir();
                    }
                    "logs" => {
                        let _ = commands::open_logs();
                    }
                    "update" => {
                        tauri::async_runtime::spawn_blocking(|| {
                            let _ = binary::update();
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Health poll loop: emit status to the frontend every 5s.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
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
                    let _ = handle.emit("status", &status);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Menu-bar behavior: closing the window hides it; the app stays in the tray.
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
                api.prevent_close();
            }
        });
}
