//! System tray: quick connect/disconnect, mode switch, current server and speed in the tooltip.

use std::sync::{Arc, Mutex};

use sybau_core::runtime::TrafficSample;
use sybau_core::settings::ConnectionMode;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::service::{ConnectionCheck, ConnectionInfo, Service};

const TRAY_ID: &str = "main";

/// UI language chosen in the frontend (`ru` / `en`), used for tray labels.
#[derive(Default)]
pub struct TrayLanguage(pub Mutex<String>);

fn ru(app: &AppHandle) -> bool {
    app.try_state::<TrayLanguage>().map(|l| l.0.lock().unwrap().clone()).unwrap_or_default() != "en"
}

fn label(app: &AppHandle, ru_text: &'static str, en_text: &'static str) -> &'static str {
    if ru(app) {
        ru_text
    } else {
        en_text
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_menu(app: &AppHandle, info: &ConnectionInfo, mode: ConnectionMode, system_proxy: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let status = match &info.profile_name {
        Some(name) if info.connected => format!("● {name}"),
        _ => label(app, "○ Не подключено", "○ Disconnected").to_string(),
    };
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "status", status, false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "toggle",
                if info.connected { label(app, "Отключить", "Disconnect") } else { label(app, "Подключить", "Connect") },
                true,
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(
                app,
                "mode-proxy",
                label(app, "Режим: прокси", "Mode: proxy"),
                true,
                mode == ConnectionMode::Proxy,
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(
                app,
                "mode-tun",
                label(app, "Режим: TUN (VPN)", "Mode: TUN (VPN)"),
                true,
                mode == ConnectionMode::Tun,
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(app, "system-proxy", label(app, "Системный прокси", "System proxy"), true, system_proxy, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "show", label(app, "Открыть окно", "Show window"), true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", label(app, "Выход", "Quit"), true, None::<&str>)?,
        ],
    )?;
    Ok(menu)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, &ConnectionInfo::default(), ConnectionMode::Proxy, true)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("V2Ray Sybau")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app = app.clone();
            let id = event.id().as_ref().to_string();
            tauri::async_runtime::spawn(async move { handle_menu(&app, &id).await });
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

async fn handle_menu(app: &AppHandle, id: &str) {
    let Some(service) = app.try_state::<Arc<Service>>().map(|s| s.inner().clone()) else { return };
    match id {
        "toggle" => {
            if let Err(e) = service.toggle().await {
                use tauri::Emitter;
                let _ = app.emit("core-error", format!("{e:#}"));
                show_main(app);
            }
        }
        "mode-proxy" | "mode-tun" | "system-proxy" => {
            let mut settings = service.settings().await;
            match id {
                "mode-proxy" => settings.proxy.mode = ConnectionMode::Proxy,
                "mode-tun" => settings.proxy.mode = ConnectionMode::Tun,
                _ => settings.proxy.system_proxy = !settings.proxy.system_proxy,
            }
            let _ = service.save_settings(settings).await;
            refresh(app, &service.connection_info().await);
        }
        "show" => show_main(app),
        "quit" => crate::quit(app).await,
        _ => {}
    }
}

/// Rebuilds the menu after connection or settings changes.
pub fn refresh(app: &AppHandle, info: &ConnectionInfo) {
    let app = app.clone();
    let info = info.clone();
    tauri::async_runtime::spawn(async move {
        let Some(service) = app.try_state::<Arc<Service>>().map(|s| s.inner().clone()) else { return };
        let settings = service.settings().await;
        let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
        if let Ok(menu) = build_menu(&app, &info, settings.proxy.mode, settings.proxy.system_proxy) {
            let _ = tray.set_menu(Some(menu));
        }
        let tooltip = match &info.profile_name {
            Some(name) if info.connected => format!("V2Ray Sybau — {name}"),
            _ => "V2Ray Sybau".to_string(),
        };
        let _ = tray.set_tooltip(Some(tooltip));
    });
}

/// A Windows notification, shown only while the window is hidden (otherwise the app shows its own).
fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let visible = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
    if !visible {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

pub fn notify_connecting(app: &AppHandle, name: &str) {
    notify(app, label(app, "Создаётся туннель…", "Creating the tunnel…"), name);
}

pub fn notify_check(app: &AppHandle, check: &ConnectionCheck) {
    let exit = check.exit.as_ref().map(|e| if e.country.is_empty() { e.ip.clone() } else { format!("{} ({})", e.ip, e.country) });
    let ru = ru(app);
    let (title, body) = match (check.problem, ru) {
        (None, true) => ("Туннель создан успешно".to_string(), format!("{} · IP {}", check.profile_name, exit.unwrap_or_default())),
        (None, false) => ("Tunnel created".to_string(), format!("{} · IP {}", check.profile_name, exit.unwrap_or_default())),
        (Some("no-traffic"), true) => {
            ("Подключено, но трафик не проходит".to_string(), format!("Сервер «{}» не отвечает", check.profile_name))
        }
        (Some("no-traffic"), false) => {
            ("Connected, but no traffic".to_string(), format!("Server «{}» does not respond", check.profile_name))
        }
        (Some(_), true) => ("Трафик идёт мимо туннеля".to_string(), "Откройте приложение, чтобы узнать подробности".to_string()),
        (Some(_), false) => ("Traffic bypasses the tunnel".to_string(), "Open the app for details".to_string()),
    };
    notify(app, &title, &body);
}

pub fn set_speed(app: &AppHandle, sample: TrafficSample) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("V2Ray Sybau\n↑ {}/s  ↓ {}/s", human(sample.up), human(sample.down))));
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
