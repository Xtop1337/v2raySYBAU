mod commands;
mod service;
mod tray;
mod win;

use std::sync::Arc;

use service::Service;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

/// Disconnects (restoring the system proxy) and exits.
pub async fn quit(app: &AppHandle) {
    if let Some(service) = app.try_state::<Arc<Service>>() {
        service.shutdown().await;
    }
    app.exit(0);
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.iter().any(|a| a == "--minimized");
    let tun_connect = args.iter().any(|a| a == "--tun-connect");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(tray::TrayLanguage::default())
        .setup(move |app| {
            let handle = app.handle().clone();
            let service = Service::new(handle.clone())?;
            app.manage(service.clone());
            tray::create(&handle)?;

            let minimized = start_minimized || tauri::async_runtime::block_on(service.settings()).general.start_minimized;
            if let Some(window) = app.get_webview_window("main") {
                if !minimized {
                    let _ = window.show();
                }
            }
            tauri::async_runtime::spawn(async move { service.startup(tun_connect).await });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle().clone();
                let to_tray = app
                    .try_state::<Arc<Service>>()
                    .map(|s| tauri::async_runtime::block_on(s.settings()).general.minimize_to_tray)
                    .unwrap_or(true);
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    api.prevent_close();
                    tauri::async_runtime::spawn(async move { quit(&app).await });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::import_text,
            commands::import_file,
            commands::save_profile,
            commands::delete_profiles,
            commands::remove_duplicates,
            commands::remove_unreachable,
            commands::sort_by_delay,
            commands::reorder_profiles,
            commands::share_links,
            commands::export_profiles,
            commands::export_core_config,
            commands::export_backup,
            commands::save_subscription,
            commands::delete_subscription,
            commands::update_subscription,
            commands::update_all_subscriptions,
            commands::connect,
            commands::disconnect,
            commands::save_settings,
            commands::start_test,
            commands::cancel_test,
            commands::node_stats,
            commands::traffic,
            commands::get_logs,
            commands::clear_logs,
            commands::export_logs,
            commands::list_processes,
            commands::restart_as_admin,
            commands::core_versions,
            commands::update_core,
            commands::set_ui_language,
            commands::open_data_dir,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Last chance to restore the system proxy and stop the cores.
                if let Some(service) = app.try_state::<Arc<Service>>() {
                    tauri::async_runtime::block_on(service.shutdown());
                }
            }
        });
}
