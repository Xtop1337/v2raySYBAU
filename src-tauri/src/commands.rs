//! Tauri commands: thin wrappers over [`Service`], errors are returned to the UI as strings.

use std::path::PathBuf;
use std::sync::Arc;

use sybau_core::logs::{export_csv, export_json};
use sybau_core::model::{LogEntry, Profile, Subscription, TestMethod};
use sybau_core::runtime::{CoreKind, TrafficSample};
use sybau_core::settings::Settings;
use sybau_core::testing::NodeStats;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::service::{ConnectionInfo, CoreVersions, ImportSummary, Service, Snapshot};
use crate::tray::TrayLanguage;
use crate::win;

type Svc<'a> = State<'a, Arc<Service>>;
type CmdResult<T> = Result<T, String>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
pub async fn get_state(service: Svc<'_>) -> CmdResult<Snapshot> {
    Ok(service.snapshot().await)
}

#[tauri::command]
pub async fn import_text(service: Svc<'_>, text: String) -> CmdResult<ImportSummary> {
    service.import(&text).await.map_err(err)
}

#[tauri::command]
pub async fn import_file(service: Svc<'_>, path: PathBuf) -> CmdResult<ImportSummary> {
    service.import_file(&path).await.map_err(err)
}

#[tauri::command]
pub async fn save_profile(service: Svc<'_>, profile: Profile) -> CmdResult<()> {
    service.save_profile(profile).await.map_err(err)
}

#[tauri::command]
pub async fn delete_profiles(service: Svc<'_>, ids: Vec<Uuid>) -> CmdResult<usize> {
    service.delete_profiles(&ids).await.map_err(err)
}

#[tauri::command]
pub async fn remove_duplicates(service: Svc<'_>) -> CmdResult<usize> {
    Ok(service.remove_duplicates().await)
}

#[tauri::command]
pub async fn remove_unreachable(service: Svc<'_>, group: Option<String>) -> CmdResult<usize> {
    service.remove_unreachable(group).await.map_err(err)
}

#[tauri::command]
pub async fn sort_by_delay(service: Svc<'_>) -> CmdResult<()> {
    service.sort_by_delay().await;
    Ok(())
}

#[tauri::command]
pub async fn reorder_profiles(service: Svc<'_>, ids: Vec<Uuid>) -> CmdResult<()> {
    service.reorder(ids).await;
    Ok(())
}

#[tauri::command]
pub async fn share_links(service: Svc<'_>, ids: Vec<Uuid>) -> CmdResult<String> {
    Ok(service.export_links(&ids).await)
}

#[tauri::command]
pub async fn export_profiles(service: Svc<'_>, ids: Vec<Uuid>, path: PathBuf) -> CmdResult<()> {
    service.export_profiles(&ids, &path).await.map_err(err)
}

#[tauri::command]
pub async fn export_core_config(service: Svc<'_>, id: Uuid, path: PathBuf) -> CmdResult<()> {
    service.export_core_config(id, &path).await.map_err(err)
}

#[tauri::command]
pub async fn export_backup(service: Svc<'_>, path: PathBuf) -> CmdResult<()> {
    service.export_backup(&path).await.map_err(err)
}

#[tauri::command]
pub async fn save_subscription(service: Svc<'_>, subscription: Subscription) -> CmdResult<()> {
    service.save_subscription(subscription).await.map_err(err)
}

#[tauri::command]
pub async fn delete_subscription(service: Svc<'_>, id: Uuid, with_profiles: bool) -> CmdResult<()> {
    service.delete_subscription(id, with_profiles).await.map_err(err)
}

#[tauri::command]
pub async fn update_subscription(service: Svc<'_>, id: Uuid) -> CmdResult<usize> {
    service.update_subscription(id).await.map_err(err)
}

#[tauri::command]
pub async fn update_all_subscriptions(service: Svc<'_>) -> CmdResult<(usize, usize)> {
    Ok(service.update_all_subscriptions(true).await)
}

#[tauri::command]
pub async fn connect(service: Svc<'_>, id: Uuid) -> CmdResult<ConnectionInfo> {
    service.connect(id).await.map_err(err)
}

#[tauri::command]
pub async fn select_profile(service: Svc<'_>, id: Uuid) -> CmdResult<()> {
    service.select_profile(id).await.map_err(err)
}

#[tauri::command]
pub async fn disconnect(service: Svc<'_>) -> CmdResult<()> {
    service.disconnect().await.map_err(err)
}

#[tauri::command]
pub async fn save_settings(service: Svc<'_>, settings: Settings) -> CmdResult<Settings> {
    service.save_settings(settings).await.map_err(err)
}

#[tauri::command]
pub async fn start_test(service: Svc<'_>, ids: Option<Vec<Uuid>>, method: TestMethod) -> CmdResult<usize> {
    service.inner().start_test(ids, method).await.map_err(err)
}

#[tauri::command]
pub fn cancel_test(service: Svc<'_>) {
    service.cancel_test();
}

#[tauri::command]
pub fn node_stats(service: Svc<'_>) -> Vec<NodeStats> {
    service.node_stats()
}

#[tauri::command]
pub fn traffic(service: Svc<'_>) -> TrafficSample {
    service.traffic()
}

#[tauri::command]
pub fn get_logs(service: Svc<'_>, limit: Option<usize>) -> Vec<LogEntry> {
    service.journal.snapshot(limit.unwrap_or(2000))
}

#[tauri::command]
pub fn clear_logs(service: Svc<'_>) {
    service.journal.clear();
}

#[tauri::command]
pub fn export_logs(service: Svc<'_>, path: PathBuf, format: String) -> CmdResult<()> {
    let entries = service.journal.snapshot(usize::MAX);
    let text = if format == "json" { export_json(&entries) } else { export_csv(&entries) };
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_processes() -> Vec<String> {
    win::processes::running_executables()
}

#[tauri::command]
pub async fn restart_as_admin(app: AppHandle, service: Svc<'_>) -> CmdResult<()> {
    win::restart_elevated("--tun-connect").map_err(err)?;
    service.shutdown().await;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn core_versions(service: Svc<'_>) -> CmdResult<CoreVersions> {
    Ok(service.core_versions().await)
}

/// `kind`: `xray`, `singbox` or `geo`.
#[tauri::command]
pub async fn update_core(service: Svc<'_>, kind: String) -> CmdResult<String> {
    let kind = match kind.as_str() {
        "xray" => Some(CoreKind::Xray),
        "singbox" => Some(CoreKind::SingBox),
        _ => None,
    };
    service.update_core(kind).await.map_err(err)
}

#[tauri::command]
pub fn set_ui_language(app: AppHandle, language: String, state: State<'_, TrayLanguage>, service: Svc<'_>) {
    *state.0.lock().unwrap() = language;
    let service = service.inner().clone();
    tauri::async_runtime::spawn(async move {
        crate::tray::refresh(&app, &service.connection_info().await);
    });
}

#[tauri::command]
pub fn open_data_dir(service: Svc<'_>) -> CmdResult<()> {
    tauri_plugin_opener::open_path(service.data_dir(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    crate::quit(&app).await;
}
