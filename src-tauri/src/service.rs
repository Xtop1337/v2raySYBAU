//! Application service: owns profiles, settings and the running connection, and runs the
//! background loops (journal flushing, packet analysis, subscription auto-update).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sybau_core::config::{self, ConfigEnv};
use sybau_core::import::{dedupe, import_text, ImportResult};
use sybau_core::logs::Journal;
use sybau_core::model::{DelayInfo, LogEntry, LogLevel, Profile, Subscription, TestMethod};
use sybau_core::runtime::{core_version, CoreEvent, CoreKind, CoreManager, CorePaths, TrafficMonitor, TrafficSample};
use sybau_core::settings::{ConnectionMode, MonitorScope, Settings};
use sybau_core::storage::Storage;
use sybau_core::subscription;
use sybau_core::testing::{self, run_batch, BatchOptions, DelayResult, NodeStats, NodeWindow};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{tray, win};

pub struct Data {
    pub profiles: Vec<Profile>,
    pub subscriptions: Vec<Subscription>,
    pub settings: Settings,
}

struct Active {
    profile_id: Uuid,
    profile_name: String,
    mode: ConnectionMode,
    since: DateTime<Utc>,
    system_proxy: bool,
    socks_port: Option<u16>,
    core: &'static str,
    traffic_cancel: CancellationToken,
}

struct Connection {
    core: CoreManager,
    active: Option<Active>,
    restarts: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub connected: bool,
    pub profile_id: Option<Uuid>,
    pub profile_name: Option<String>,
    pub mode: Option<ConnectionMode>,
    pub since: Option<DateTime<Utc>>,
    pub system_proxy: bool,
    pub core: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub profiles: Vec<Profile>,
    pub subscriptions: Vec<Subscription>,
    pub settings: Settings,
    pub connection: ConnectionInfo,
    pub app_version: &'static str,
    pub elevated: bool,
    pub data_dir: String,
    pub platform: &'static str,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub added: usize,
    pub duplicates: usize,
    pub subscriptions: usize,
    pub restored_backup: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreVersions {
    pub xray: Option<String>,
    pub singbox: Option<String>,
    pub geo: bool,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestProgress {
    done: usize,
    total: usize,
    running: bool,
}

pub struct Service {
    handle: AppHandle,
    storage: Storage,
    paths: CorePaths,
    pub journal: Arc<Journal>,
    data: Mutex<Data>,
    conn: Mutex<Connection>,
    /// Generation number and cancel token of the running latency test.
    test_cancel: std::sync::Mutex<Option<(u64, CancellationToken)>>,
    test_generation: std::sync::atomic::AtomicU64,
    monitor: std::sync::Mutex<HashMap<Uuid, NodeWindow>>,
    log_queue: Arc<std::sync::Mutex<Vec<LogEntry>>>,
    last_traffic: std::sync::Mutex<TrafficSample>,
    clash_api_port: u16,
}

impl Service {
    pub fn new(handle: AppHandle) -> Result<Arc<Self>> {
        let storage = Storage::new(Storage::default_root())?;
        let paths = CorePaths { dir: storage.dir("cores")?, config_dir: storage.dir("run")? };
        let journal = Arc::new(Journal::new(storage.dir("logs").ok(), sybau_core::logs::DEFAULT_CAPACITY));
        install_bundled_cores(&handle, &paths.dir, &journal);

        let settings = storage.load_settings().unwrap_or_else(|e| {
            journal.app(LogLevel::Error, format!("settings: {e:#}"));
            Settings::default()
        });
        let profiles = storage.load_profiles().unwrap_or_else(|e| {
            journal.app(LogLevel::Error, format!("profiles: {e:#}"));
            Vec::new()
        });
        let subscriptions = storage.load_subscriptions().unwrap_or_default();

        let log_queue: Arc<std::sync::Mutex<Vec<LogEntry>>> = Arc::default();
        let sink_handle = handle.clone();
        let sink_journal = journal.clone();
        let sink_queue = log_queue.clone();
        let core = CoreManager::new(
            paths.clone(),
            Arc::new(move |event| match event {
                CoreEvent::Log(entry) => {
                    sink_journal.push(entry.clone());
                    sink_queue.lock().unwrap().push(entry);
                }
                CoreEvent::Exited { kind, code } => {
                    let handle = sink_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(service) = handle.try_state::<Arc<Service>>() {
                            service.on_core_exit(kind, code).await;
                        }
                    });
                }
            }),
        );

        let service = Arc::new(Service {
            handle,
            storage,
            paths,
            journal,
            data: Mutex::new(Data { profiles, subscriptions, settings }),
            conn: Mutex::new(Connection { core, active: None, restarts: 0 }),
            test_cancel: std::sync::Mutex::new(None),
            test_generation: std::sync::atomic::AtomicU64::new(0),
            monitor: std::sync::Mutex::new(HashMap::new()),
            log_queue,
            last_traffic: std::sync::Mutex::new(TrafficSample::default()),
            clash_api_port: 10814,
        });
        service
            .log(LogLevel::Info, format!("V2Ray Sybau {} started, data in {}", sybau_core::APP_VERSION, service.storage.root().display()));
        Ok(service)
    }

    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        let entry = self.journal.app(level, message);
        self.log_queue.lock().unwrap().push(entry);
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        let _ = self.handle.emit(event, payload);
    }

    // ---- startup / background ------------------------------------------------------------

    pub async fn startup(self: &Arc<Self>, tun_connect: bool) {
        self.clone().spawn_log_flusher();
        self.clone().spawn_monitor();
        self.clone().spawn_subscription_updater();

        let settings = self.data.lock().await.settings.clone();
        if win::sysproxy::is_ours(settings.proxy.http_port) {
            // Left over from a crash: the proxy points at a port nothing listens on yet.
            let _ = win::sysproxy::set(false, settings.proxy.http_port, "");
        }
        if settings.general.update_subscriptions_on_start {
            self.update_all_subscriptions(false).await;
        }
        let last = settings.last_profile_id;
        if tun_connect {
            let mut s = settings.clone();
            s.proxy.mode = ConnectionMode::Tun;
            let _ = self.save_settings(s).await;
        }
        if settings.general.auto_connect || tun_connect {
            if let Some(id) = last {
                if let Err(e) = self.connect(id).await {
                    self.log(LogLevel::Error, format!("auto-connect: {e:#}"));
                }
            }
        }
        if settings.general.scan_on_start {
            let _ = self.start_test(None, settings.test.method).await;
        }
    }

    fn spawn_log_flusher(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(300)).await;
                let batch: Vec<LogEntry> = std::mem::take(&mut *self.log_queue.lock().unwrap());
                if !batch.is_empty() {
                    self.emit("logs", batch);
                }
            }
        });
    }

    /// Packet analysis: periodic probes per node → latency, jitter and loss in real time.
    fn spawn_monitor(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                let (analysis, test, targets, active) = {
                    let data = self.data.lock().await;
                    let conn = self.conn.lock().await;
                    let active = conn.active.as_ref().map(|a| (a.profile_id, a.socks_port));
                    let active_group = active.and_then(|(id, _)| data.profiles.iter().find(|p| p.id == id)).map(|p| p.group.clone());
                    let targets: Vec<Profile> = match data.settings.analysis.scope {
                        MonitorScope::Active => data.profiles.iter().filter(|p| Some(p.id) == active.map(|a| a.0)).cloned().collect(),
                        MonitorScope::Group => data.profiles.iter().filter(|p| Some(&p.group) == active_group.as_ref()).cloned().collect(),
                        MonitorScope::All => data.profiles.iter().take(300).cloned().collect(),
                    };
                    (data.settings.analysis.clone(), data.settings.test.clone(), targets, active)
                };
                let interval = Duration::from_secs(analysis.interval_secs.clamp(1, 600) as u64);
                if !analysis.enabled || targets.is_empty() {
                    tokio::time::sleep(interval).await;
                    continue;
                }
                let timeout = Duration::from_millis(test.timeout_ms.clamp(500, 10_000));
                let probes = targets.clone().into_iter().map(|p| {
                    let method = analysis.method;
                    let url = test.url.clone();
                    let socks = active.filter(|(id, _)| *id == p.id).and_then(|(_, port)| port);
                    async move {
                        let rtt = match (method, socks) {
                            (TestMethod::Real, Some(port)) => testing::http_delay(port, &url, timeout).await,
                            (TestMethod::Icmp, _) => testing::icmp_ping(&p.address, timeout).await,
                            _ => testing::tcp_ping(&p.address, p.port, timeout).await,
                        };
                        (p.id, rtt.map(|d| d.as_millis().max(1) as u32))
                    }
                });
                let results = futures_join(probes).await;
                let stats: Vec<NodeStats> = {
                    let mut windows = self.monitor.lock().unwrap();
                    windows.retain(|id, _| targets.iter().any(|p| p.id == *id));
                    results
                        .into_iter()
                        .map(|(id, sample)| {
                            let window = windows.entry(id).or_insert_with(|| NodeWindow::new(analysis.window as usize));
                            window.resize(analysis.window as usize);
                            window.record(sample);
                            window.stats(id)
                        })
                        .collect()
                };
                self.emit("node-stats", stats);
                tokio::time::sleep(interval).await;
            }
        });
    }

    fn spawn_subscription_updater(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(600)).await;
                let due: Vec<Uuid> =
                    self.data.lock().await.subscriptions.iter().filter(|s| subscription::is_due(s)).map(|s| s.id).collect();
                for id in due {
                    let _ = self.update_subscription(id).await;
                }
            }
        });
    }

    // ---- state ---------------------------------------------------------------------------

    pub async fn snapshot(&self) -> Snapshot {
        let data = self.data.lock().await;
        Snapshot {
            profiles: data.profiles.clone(),
            subscriptions: data.subscriptions.clone(),
            settings: data.settings.clone(),
            connection: self.connection_info().await,
            app_version: sybau_core::APP_VERSION,
            elevated: win::is_elevated(),
            data_dir: self.storage.root().display().to_string(),
            platform: std::env::consts::OS,
        }
    }

    pub async fn connection_info(&self) -> ConnectionInfo {
        let conn = self.conn.lock().await;
        match &conn.active {
            Some(a) => ConnectionInfo {
                connected: true,
                profile_id: Some(a.profile_id),
                profile_name: Some(a.profile_name.clone()),
                mode: Some(a.mode),
                since: Some(a.since),
                system_proxy: a.system_proxy,
                core: Some(a.core),
            },
            None => ConnectionInfo::default(),
        }
    }

    async fn notify_connection(&self) {
        let info = self.connection_info().await;
        tray::refresh(&self.handle, &info);
        self.emit("connection", info);
    }

    async fn persist_profiles(&self, data: &Data) {
        if let Err(e) = self.storage.save_profiles(&data.profiles) {
            self.log(LogLevel::Error, format!("save profiles: {e:#}"));
        }
    }

    async fn persist_subscriptions(&self, data: &Data) {
        if let Err(e) = self.storage.save_subscriptions(&data.subscriptions) {
            self.log(LogLevel::Error, format!("save subscriptions: {e:#}"));
        }
    }

    async fn emit_profiles(&self) {
        let data = self.data.lock().await;
        self.emit("profiles", data.profiles.clone());
        self.emit("subscriptions", data.subscriptions.clone());
    }

    // ---- profiles ------------------------------------------------------------------------

    pub async fn import(&self, text: &str) -> Result<ImportSummary> {
        let result: ImportResult = import_text(text);
        let mut summary = ImportSummary { errors: result.errors, ..Default::default() };
        let mut new_subscriptions = Vec::new();
        {
            let mut data = self.data.lock().await;
            let mut incoming = result.profiles;
            if let Some(backup) = result.backup {
                summary.restored_backup = true;
                incoming.extend(backup.profiles);
                for sub in backup.subscriptions {
                    if !data.subscriptions.iter().any(|s| s.url == sub.url) {
                        data.subscriptions.push(sub);
                    }
                }
                if let Some(settings) = backup.settings {
                    data.settings = settings;
                    let _ = self.storage.save_settings(&data.settings);
                    self.emit("settings", data.settings.clone());
                }
            }
            let known: std::collections::HashSet<String> = data.profiles.iter().map(Profile::identity_key).collect();
            let before = incoming.len();
            incoming.retain(|p| !known.contains(&p.identity_key()));
            dedupe(&mut incoming);
            summary.duplicates = before - incoming.len();
            summary.added = incoming.len();
            for p in &mut incoming {
                if data.profiles.iter().any(|existing| existing.id == p.id) {
                    p.id = Uuid::new_v4();
                }
            }
            data.profiles.extend(incoming);

            for url in result.subscription_urls {
                if data.subscriptions.iter().any(|s| s.url == url) {
                    continue;
                }
                let sub = Subscription { url: url.clone(), name: guess_subscription_name(&url), ..Default::default() };
                new_subscriptions.push(sub.id);
                data.subscriptions.push(sub);
            }
            summary.subscriptions = new_subscriptions.len();
            self.persist_profiles(&data).await;
            self.persist_subscriptions(&data).await;
        }
        for id in new_subscriptions {
            match self.update_subscription(id).await {
                Ok(count) => summary.added += count,
                Err(e) => summary.errors.push(format!("{e:#}")),
            }
        }
        self.emit_profiles().await;
        self.log(
            LogLevel::Info,
            format!("import: +{} servers, {} subscriptions, {} duplicates", summary.added, summary.subscriptions, summary.duplicates),
        );
        Ok(summary)
    }

    pub async fn import_file(&self, path: &Path) -> Result<ImportSummary> {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        self.import(&String::from_utf8_lossy(&bytes)).await
    }

    pub async fn save_profile(&self, mut profile: Profile) -> Result<()> {
        let mut data = self.data.lock().await;
        if profile.name.trim().is_empty() {
            profile.name = profile.display_name();
        }
        match data.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile,
            None => data.profiles.push(profile),
        }
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        Ok(())
    }

    pub async fn delete_profiles(&self, ids: &[Uuid]) -> Result<usize> {
        let active = self.conn.lock().await.active.as_ref().map(|a| a.profile_id);
        if active.is_some_and(|id| ids.contains(&id)) {
            self.disconnect().await?;
        }
        let mut data = self.data.lock().await;
        let before = data.profiles.len();
        data.profiles.retain(|p| !ids.contains(&p.id));
        let removed = before - data.profiles.len();
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        Ok(removed)
    }

    pub async fn remove_duplicates(&self) -> usize {
        let mut data = self.data.lock().await;
        let removed = dedupe(&mut data.profiles);
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        removed
    }

    /// Deletes servers whose last test timed out (v2rayNG "remove invalid").
    pub async fn remove_unreachable(&self, group: Option<String>) -> Result<usize> {
        let ids: Vec<Uuid> = {
            let data = self.data.lock().await;
            data.profiles.iter().filter(|p| p.delay.ms == Some(-1) && group.as_ref().is_none_or(|g| &p.group == g)).map(|p| p.id).collect()
        };
        self.delete_profiles(&ids).await
    }

    /// Reorders profiles: tested by delay first, then untested, then timeouts.
    pub async fn sort_by_delay(&self) {
        let mut data = self.data.lock().await;
        data.profiles.sort_by_key(|p| match p.delay.ms {
            Some(ms) if ms >= 0 => (0, ms),
            None => (1, 0),
            Some(_) => (2, 0),
        });
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
    }

    pub async fn reorder(&self, ids: Vec<Uuid>) {
        let mut data = self.data.lock().await;
        let position: HashMap<Uuid, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        data.profiles.sort_by_key(|p| position.get(&p.id).copied().unwrap_or(usize::MAX));
        self.persist_profiles(&data).await;
    }

    pub async fn profile(&self, id: Uuid) -> Result<Profile> {
        self.data.lock().await.profiles.iter().find(|p| p.id == id).cloned().ok_or_else(|| anyhow!("profile not found"))
    }

    pub async fn export_profiles(&self, ids: &[Uuid], path: &Path) -> Result<()> {
        let data = self.data.lock().await;
        let selected: Vec<Profile> = data.profiles.iter().filter(|p| ids.is_empty() || ids.contains(&p.id)).cloned().collect();
        std::fs::write(path, sybau_core::storage::export_profiles(&selected))?;
        Ok(())
    }

    pub async fn export_links(&self, ids: &[Uuid]) -> String {
        let data = self.data.lock().await;
        data.profiles
            .iter()
            .filter(|p| ids.is_empty() || ids.contains(&p.id))
            .map(sybau_core::share::build_link)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Writes the complete core config of a profile (for use in other clients or for debugging).
    pub async fn export_core_config(&self, id: Uuid, path: &Path) -> Result<()> {
        let profile = self.profile(id).await?;
        let settings = self.data.lock().await.settings.clone();
        let plan = config::plan(&profile, &settings, &self.config_env());
        let value = plan.xray.or(plan.singbox).ok_or_else(|| anyhow!("nothing to export"))?;
        std::fs::write(path, config::to_pretty(&value))?;
        Ok(())
    }

    pub async fn export_backup(&self, path: &Path) -> Result<()> {
        let data = self.data.lock().await;
        std::fs::write(path, sybau_core::storage::export_backup(&data.profiles, &data.subscriptions, &data.settings))?;
        Ok(())
    }

    // ---- subscriptions -------------------------------------------------------------------

    pub async fn save_subscription(&self, sub: Subscription) -> Result<()> {
        let mut data = self.data.lock().await;
        let group = sub.name.clone();
        match data.subscriptions.iter_mut().find(|s| s.id == sub.id) {
            Some(existing) => *existing = sub.clone(),
            None => data.subscriptions.push(sub.clone()),
        }
        for p in data.profiles.iter_mut().filter(|p| p.subscription_id == Some(sub.id)) {
            if !group.trim().is_empty() {
                p.group = group.clone();
            }
        }
        self.persist_subscriptions(&data).await;
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        Ok(())
    }

    pub async fn delete_subscription(&self, id: Uuid, with_profiles: bool) -> Result<()> {
        if with_profiles {
            let ids: Vec<Uuid> = self.data.lock().await.profiles.iter().filter(|p| p.subscription_id == Some(id)).map(|p| p.id).collect();
            self.delete_profiles(&ids).await?;
        }
        let mut data = self.data.lock().await;
        data.subscriptions.retain(|s| s.id != id);
        for p in data.profiles.iter_mut().filter(|p| p.subscription_id == Some(id)) {
            p.subscription_id = None;
        }
        self.persist_subscriptions(&data).await;
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        Ok(())
    }

    /// Downloads one subscription (directly, then through the active proxy if that fails).
    pub async fn update_subscription(&self, id: Uuid) -> Result<usize> {
        let (sub, ua) = {
            let data = self.data.lock().await;
            let sub = data.subscriptions.iter().find(|s| s.id == id).cloned().ok_or_else(|| anyhow!("subscription not found"))?;
            let ua = if sub.user_agent.trim().is_empty() {
                data.settings.advanced.subscription_user_agent.clone()
            } else {
                sub.user_agent.clone()
            };
            (sub, ua)
        };
        let proxy = self.conn.lock().await.active.as_ref().and_then(|a| a.socks_port).map(|p| format!("socks5h://127.0.0.1:{p}"));
        let fetched = match subscription::fetch(&sub.url, &ua, None).await {
            Ok(f) => Ok(f),
            Err(direct_error) => match &proxy {
                Some(proxy) => subscription::fetch(&sub.url, &ua, Some(proxy)).await.map_err(|_| direct_error),
                None => Err(direct_error),
            },
        };
        let mut data = self.data.lock().await;
        let result = match fetched {
            Ok(fetched) => {
                let record = data.subscriptions.iter_mut().find(|s| s.id == id).ok_or_else(|| anyhow!("subscription removed"))?;
                subscription::mark_updated(record, &fetched);
                let record = record.clone();
                let count = subscription::merge(&mut data.profiles, &record, fetched.profiles);
                self.log(LogLevel::Info, format!("subscription «{}»: {count} servers", record.name));
                Ok(count)
            }
            Err(e) => {
                if let Some(record) = data.subscriptions.iter_mut().find(|s| s.id == id) {
                    record.last_error = Some(format!("{e:#}"));
                }
                self.log(LogLevel::Error, format!("subscription «{}»: {e:#}", sub.name));
                Err(e)
            }
        };
        self.persist_subscriptions(&data).await;
        self.persist_profiles(&data).await;
        drop(data);
        self.emit_profiles().await;
        result
    }

    pub async fn update_all_subscriptions(&self, include_disabled: bool) -> (usize, usize) {
        let ids: Vec<Uuid> = self.data.lock().await.subscriptions.iter().filter(|s| include_disabled || s.enabled).map(|s| s.id).collect();
        let mut ok = 0;
        for id in &ids {
            if self.update_subscription(*id).await.is_ok() {
                ok += 1;
            }
        }
        (ok, ids.len())
    }

    // ---- connection ----------------------------------------------------------------------

    fn config_env(&self) -> ConfigEnv {
        ConfigEnv { geo_available: self.paths.geo_available(), clash_api_port: self.clash_api_port, ..ConfigEnv::default() }
    }

    pub async fn connect(&self, id: Uuid) -> Result<ConnectionInfo> {
        let (profile, settings) = {
            let data = self.data.lock().await;
            let profile = data.profiles.iter().find(|p| p.id == id).cloned().ok_or_else(|| anyhow!("profile not found"))?;
            (profile, data.settings.clone())
        };
        let tun = settings.proxy.mode == ConnectionMode::Tun;
        if tun && cfg!(windows) && !win::is_elevated() {
            bail!("TUN_NEEDS_ADMIN");
        }
        let plan = config::plan(&profile, &settings, &self.config_env());
        {
            let mut conn = self.conn.lock().await;
            stop_active(&mut conn, &settings).await;
            let ready = if tun { None } else { Some(settings.proxy.socks_port) };
            if let Err(e) = conn.core.start(&plan, ready).await {
                drop(conn);
                self.log(LogLevel::Error, format!("connect «{}»: {e:#}", profile.display_name()));
                self.notify_connection().await;
                return Err(e);
            }
            let system_proxy = !tun && settings.proxy.system_proxy;
            if system_proxy {
                if let Err(e) = win::sysproxy::set(true, settings.proxy.http_port, &settings.proxy.bypass) {
                    self.log(LogLevel::Warning, format!("system proxy: {e:#}"));
                }
            }
            let traffic_cancel = CancellationToken::new();
            let monitor = if plan.xray.is_some() {
                TrafficMonitor::Xray { exe: self.paths.exe(CoreKind::Xray), api_port: settings.advanced.api_port }
            } else {
                TrafficMonitor::SingBox { clash_port: self.clash_api_port }
            };
            let handle = self.handle.clone();
            monitor.spawn(
                traffic_cancel.clone(),
                Arc::new(move |sample| {
                    if let Some(service) = handle.try_state::<Arc<Service>>() {
                        *service.last_traffic.lock().unwrap() = sample;
                        tray::set_speed(&handle, sample);
                    }
                    let _ = handle.emit("traffic", sample);
                }),
            );
            // In native TUN mode there is no local SOCKS inbound to measure through.
            let socks_port = (!(tun && plan.xray.is_none())).then_some(settings.proxy.socks_port);
            conn.active = Some(Active {
                profile_id: profile.id,
                profile_name: profile.display_name(),
                mode: settings.proxy.mode,
                since: Utc::now(),
                system_proxy,
                socks_port,
                core: match (plan.xray.is_some(), plan.singbox.is_some()) {
                    (true, true) => "xray+sing-box",
                    (true, false) => "xray",
                    _ => "sing-box",
                },
                traffic_cancel,
            });
            conn.restarts = 0;
        }
        {
            let mut data = self.data.lock().await;
            data.settings.last_profile_id = Some(profile.id);
            let _ = self.storage.save_settings(&data.settings);
        }
        self.log(LogLevel::Info, format!("connected to «{}» ({:?} mode)", profile.display_name(), settings.proxy.mode));
        self.notify_connection().await;
        Ok(self.connection_info().await)
    }

    pub async fn disconnect(&self) -> Result<()> {
        let settings = self.data.lock().await.settings.clone();
        let was_active = {
            let mut conn = self.conn.lock().await;
            let was = conn.active.is_some();
            stop_active(&mut conn, &settings).await;
            was
        };
        if was_active {
            self.log(LogLevel::Info, "disconnected");
        }
        *self.last_traffic.lock().unwrap() = TrafficSample::default();
        self.notify_connection().await;
        Ok(())
    }

    pub async fn toggle(&self) -> Result<()> {
        let active = self.conn.lock().await.active.is_some();
        if active {
            self.disconnect().await
        } else {
            let last = self.data.lock().await.settings.last_profile_id;
            let first = self.data.lock().await.profiles.first().map(|p| p.id);
            let id = last.or(first).ok_or_else(|| anyhow!("no servers"))?;
            self.connect(id).await.map(|_| ())
        }
    }

    async fn on_core_exit(&self, kind: CoreKind, code: Option<i32>) {
        let (profile_id, restarts) = {
            let conn = self.conn.lock().await;
            match &conn.active {
                Some(a) => (a.profile_id, conn.restarts),
                None => return,
            }
        };
        self.log(LogLevel::Error, format!("{} exited unexpectedly (code {code:?})", kind.label()));
        if restarts < 3 {
            tokio::time::sleep(Duration::from_secs(2 + restarts as u64 * 2)).await;
            self.conn.lock().await.restarts = restarts + 1;
            self.log(LogLevel::Warning, format!("restarting connection (attempt {})", restarts + 1));
            let result = self.connect(profile_id).await;
            if result.is_ok() {
                self.conn.lock().await.restarts = restarts + 1;
                return;
            }
        }
        let _ = self.disconnect().await;
        self.emit("core-error", format!("{} stopped", kind.label()));
    }

    pub async fn shutdown(&self) {
        if let Some((_, cancel)) = self.test_cancel.lock().unwrap().take() {
            cancel.cancel();
        }
        let _ = self.disconnect().await;
    }

    // ---- settings ------------------------------------------------------------------------

    pub async fn save_settings(&self, new: Settings) -> Result<Settings> {
        let (old, reconnect_to) = {
            let mut data = self.data.lock().await;
            let old = std::mem::replace(&mut data.settings, new.clone());
            self.storage.save_settings(&data.settings)?;
            let active = self.conn.lock().await.active.as_ref().map(|a| a.profile_id);
            let connection_changed = old.proxy != new.proxy
                || old.tun != new.tun
                || old.dns != new.dns
                || old.routing != new.routing
                || old.mux != new.mux
                || old.fragment != new.fragment
                || old.advanced != new.advanced;
            (old, active.filter(|_| connection_changed))
        };
        if old.general.autostart != new.general.autostart {
            use tauri_plugin_autostart::ManagerExt;
            let launcher = self.handle.autolaunch();
            let result = if new.general.autostart { launcher.enable() } else { launcher.disable() };
            if let Err(e) = result {
                self.log(LogLevel::Warning, format!("autostart: {e}"));
            }
        }
        if let Some(id) = reconnect_to {
            if let Err(e) = self.connect(id).await {
                self.log(LogLevel::Error, format!("reconnect after settings change: {e:#}"));
            }
        }
        self.emit("settings", new.clone());
        Ok(new)
    }

    pub async fn settings(&self) -> Settings {
        self.data.lock().await.settings.clone()
    }

    // ---- latency tests -------------------------------------------------------------------

    pub async fn start_test(self: &Arc<Self>, ids: Option<Vec<Uuid>>, method: TestMethod) -> Result<usize> {
        let (profiles, settings) = {
            let data = self.data.lock().await;
            let profiles: Vec<Profile> =
                data.profiles.iter().filter(|p| ids.as_ref().is_none_or(|ids| ids.contains(&p.id))).cloned().collect();
            (profiles, data.settings.clone())
        };
        let cancel = CancellationToken::new();
        let generation = self.test_generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if let Some((_, previous)) = self.test_cancel.lock().unwrap().replace((generation, cancel.clone())) {
            previous.cancel();
        }
        let total = profiles.len();
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let results: Arc<std::sync::Mutex<Vec<DelayResult>>> = Arc::default();
            let sink_results = results.clone();
            let handle = service.handle.clone();
            let progress_total = total;
            service.emit("test-progress", TestProgress { done: 0, total, running: true });
            let outcome = run_batch(
                profiles,
                &settings,
                &service.paths,
                &BatchOptions::from_settings(&settings, method),
                cancel.clone(),
                Arc::new(move |result| {
                    let done = {
                        let mut list = sink_results.lock().unwrap();
                        list.push(result.clone());
                        list.len()
                    };
                    let _ = handle.emit("delay-result", result);
                    let _ = handle.emit("test-progress", TestProgress { done, total: progress_total, running: true });
                }),
            )
            .await;
            if let Err(e) = outcome {
                service.log(LogLevel::Error, format!("latency test: {e:#}"));
                service.emit("core-error", format!("{e:#}"));
            }
            let results = std::mem::take(&mut *results.lock().unwrap());
            let ok = results.iter().filter(|r| r.ms >= 0).count();
            {
                let mut data = service.data.lock().await;
                let now = Utc::now();
                for r in &results {
                    if let Some(p) = data.profiles.iter_mut().find(|p| p.id == r.profile_id) {
                        p.delay = DelayInfo { ms: Some(r.ms), method: Some(r.method), tested_at: Some(now) };
                    }
                }
                service.persist_profiles(&data).await;
            }
            service.log(LogLevel::Info, format!("latency test ({method:?}): {ok}/{} servers answered", results.len()));
            service.emit("test-progress", TestProgress { done: results.len(), total, running: false });
            service.emit_profiles().await;
            let mut slot = service.test_cancel.lock().unwrap();
            if slot.as_ref().is_some_and(|(g, _)| *g == generation) {
                *slot = None;
            }
        });
        Ok(total)
    }

    pub fn cancel_test(&self) {
        if let Some((_, cancel)) = self.test_cancel.lock().unwrap().take() {
            cancel.cancel();
        }
    }

    pub fn node_stats(&self) -> Vec<NodeStats> {
        self.monitor.lock().unwrap().iter().map(|(id, w)| w.stats(*id)).collect()
    }

    pub fn traffic(&self) -> TrafficSample {
        *self.last_traffic.lock().unwrap()
    }

    // ---- cores / updates -----------------------------------------------------------------

    pub async fn core_versions(&self) -> CoreVersions {
        CoreVersions {
            xray: core_version(&self.paths, CoreKind::Xray).await,
            singbox: core_version(&self.paths, CoreKind::SingBox).await,
            geo: self.paths.geo_available(),
            dir: self.paths.dir.display().to_string(),
        }
    }

    fn download_proxy(&self, conn: &Connection) -> Option<String> {
        conn.active.as_ref().and_then(|a| a.socks_port).map(|p| format!("socks5h://127.0.0.1:{p}"))
    }

    /// Updates a core (or the geo files with `None`). Downloads through the active proxy when
    /// connected (GitHub may be blocked), stops the cores while replacing files, reconnects.
    pub async fn update_core(&self, kind: Option<CoreKind>) -> Result<String> {
        let proxy = self.download_proxy(&*self.conn.lock().await);
        let client = sybau_core::updates::http_client(proxy.as_deref())?;
        let staging = self.storage.dir("staging")?;
        let label = kind.map_or("geo", CoreKind::label);
        self.log(LogLevel::Info, format!("updating {label}…"));
        let version = match kind {
            Some(kind) => sybau_core::updates::update_core(&client, kind, &staging).await?,
            None => {
                sybau_core::updates::update_geo(&client, &staging).await?;
                "latest".into()
            }
        };
        let active = self.conn.lock().await.active.as_ref().map(|a| a.profile_id);
        if active.is_some() {
            let settings = self.settings().await;
            stop_active(&mut *self.conn.lock().await, &settings).await;
        }
        for entry in std::fs::read_dir(&staging)?.flatten() {
            let target = self.paths.dir.join(entry.file_name());
            std::fs::rename(entry.path(), &target).or_else(|_| std::fs::copy(entry.path(), &target).map(|_| ()))?;
        }
        if let Some(id) = active {
            self.connect(id).await?;
        }
        self.log(LogLevel::Info, format!("{label} updated to {version}"));
        Ok(version)
    }

    pub fn data_dir(&self) -> PathBuf {
        self.storage.root().to_path_buf()
    }
}

async fn stop_active(conn: &mut Connection, settings: &Settings) {
    if let Some(active) = conn.active.take() {
        active.traffic_cancel.cancel();
        if active.system_proxy {
            let _ = win::sysproxy::set(false, settings.proxy.http_port, "");
        }
    }
    conn.core.stop().await;
}

async fn futures_join<F: std::future::Future>(futures: impl Iterator<Item = F>) -> Vec<F::Output> {
    use futures_util::stream::{self, StreamExt};
    stream::iter(futures).buffer_unordered(16).collect().await
}

fn guess_subscription_name(url: &str) -> String {
    url.split("://").nth(1).and_then(|rest| rest.split(['/', '?', ':']).next()).unwrap_or("Subscription").to_string()
}

/// Copies cores shipped with the installer to the writable cores directory on first run and
/// after application updates (marker file holds the app version that installed them).
fn install_bundled_cores(handle: &AppHandle, target: &Path, journal: &Journal) {
    let Ok(resources) = handle.path().resource_dir() else { return };
    let source = resources.join("resources").join("cores");
    let Ok(entries) = std::fs::read_dir(&source) else { return };
    let marker = target.join(".bundled-version");
    let installed_by = std::fs::read_to_string(&marker).unwrap_or_default();
    let fresh_app = installed_by.trim() != sybau_core::APP_VERSION;
    let mut copied = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let dest = target.join(&name);
        if (fresh_app || !dest.exists()) && std::fs::copy(entry.path(), &dest).is_ok() {
            copied += 1;
        }
    }
    if copied > 0 {
        let _ = std::fs::write(&marker, sybau_core::APP_VERSION);
        journal.app(LogLevel::Info, format!("installed {copied} bundled core files"));
    }
}
