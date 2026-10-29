//! Running Xray and sing-box: process control, readiness checks, traffic statistics.

mod process;
mod traffic;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;

use crate::config::LaunchPlan;
use crate::model::LogEntry;
pub use process::{free_ports, wait_for_port, CoreProcess};
use process::{is_own_pid, request_stop};
pub use traffic::{TrafficMonitor, TrafficSample};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoreKind {
    Xray,
    SingBox,
}

impl CoreKind {
    pub fn exe_name(self) -> &'static str {
        match (self, cfg!(windows)) {
            (CoreKind::Xray, true) => "xray.exe",
            (CoreKind::Xray, false) => "xray",
            (CoreKind::SingBox, true) => "sing-box.exe",
            (CoreKind::SingBox, false) => "sing-box",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CoreKind::Xray => "Xray",
            CoreKind::SingBox => "sing-box",
        }
    }
}

/// Location of core executables and geo files (`%LOCALAPPDATA%\V2RaySybau\cores`).
#[derive(Debug, Clone)]
pub struct CorePaths {
    pub dir: PathBuf,
    /// Directory for generated configs.
    pub config_dir: PathBuf,
}

impl CorePaths {
    pub fn exe(&self, kind: CoreKind) -> PathBuf {
        self.dir.join(kind.exe_name())
    }

    pub fn geo_available(&self) -> bool {
        self.dir.join("geoip.dat").exists() && self.dir.join("geosite.dat").exists()
    }

    pub fn ensure(&self, kind: CoreKind) -> Result<PathBuf> {
        let exe = self.exe(kind);
        if !exe.exists() {
            bail!("{} not found at {}", kind.label(), exe.display());
        }
        Ok(exe)
    }
}

/// Events produced by running cores.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    Log(LogEntry),
    Exited { kind: CoreKind, code: Option<i32> },
}

pub type EventSink = Arc<dyn Fn(CoreEvent) + Send + Sync>;

/// Owns the cores of the active connection.
pub struct CoreManager {
    paths: CorePaths,
    sink: EventSink,
    xray: Option<CoreProcess>,
    singbox: Option<CoreProcess>,
    /// Config of the running sing-box, to keep it (and its TUN adapter) across server switches.
    singbox_config: Option<Value>,
    tun_active: bool,
}

/// How to start a connection.
#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    /// Local port that must accept connections before the start counts as successful.
    pub ready_port: Option<u16>,
    /// Local ports the cores will listen on (checked to be free when switching servers).
    pub ports: Vec<u16>,
    /// The plan creates a TUN adapter.
    pub tun: bool,
}

impl StartOptions {
    pub fn for_plan(plan: &LaunchPlan, settings: &crate::settings::Settings) -> Self {
        let tun = settings.proxy.mode == crate::settings::ConnectionMode::Tun;
        // Xray always exposes SOCKS (sing-box TUN forwards to it); sing-box does only in proxy mode.
        let has_socks = plan.xray.is_some() || !tun;
        let mut ports = Vec::new();
        if has_socks {
            ports.extend([settings.proxy.socks_port, settings.proxy.http_port]);
        }
        if plan.xray.is_some() {
            ports.push(settings.advanced.api_port);
        }
        if settings.dns.local_dns {
            ports.push(settings.dns.local_port);
        }
        StartOptions { ready_port: has_socks.then_some(settings.proxy.socks_port), ports, tun }
    }
}

/// Waits until every port can be bound on 127.0.0.1; returns the first one still busy.
pub async fn wait_ports_free(ports: &[u16], timeout: Duration) -> Option<u16> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let busy = ports.iter().copied().find(|port| std::net::TcpListener::bind(("127.0.0.1", *port)).is_err());
        match busy {
            None => return None,
            Some(port) if tokio::time::Instant::now() >= deadline => return Some(port),
            Some(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

impl CoreManager {
    pub fn new(paths: CorePaths, sink: EventSink) -> Self {
        CoreManager { paths, sink, xray: None, singbox: None, singbox_config: None, tun_active: false }
    }

    pub fn paths(&self) -> &CorePaths {
        &self.paths
    }

    /// Process ids of the running Xray and sing-box.
    pub fn pids(&self) -> (Option<u32>, Option<u32>) {
        (self.xray.as_ref().map(CoreProcess::pid), self.singbox.as_ref().map(CoreProcess::pid))
    }

    pub fn is_running(&self) -> bool {
        self.xray.as_ref().is_some_and(CoreProcess::is_alive) || self.singbox.as_ref().is_some_and(CoreProcess::is_alive)
    }

    /// Starts the cores of `plan` and waits until `options.ready_port` accepts connections.
    /// Cores that are already running (e.g. the previous server) are stopped first, and the
    /// local ports / TUN adapter they held are given time to be released.
    pub async fn start(&mut self, plan: &LaunchPlan, options: &StartOptions) -> Result<()> {
        let had_tun = self.tun_active;
        let was_running = self.xray.is_some() || self.singbox.is_some();
        // TUN over Xray: a new server only changes Xray's config. sing-box keeps running with
        // its TUN adapter, so Windows does not tear the adapter and its routes down and build
        // them again on every switch (which became unreliable after a few switches).
        let keep_singbox = plan.xray.is_some()
            && plan.singbox.is_some()
            && self.singbox.as_ref().is_some_and(CoreProcess::is_alive)
            && self.singbox_config.as_ref() == plan.singbox.as_ref();
        if keep_singbox {
            if let Some(mut xray) = self.xray.take() {
                xray.kill().await;
            }
        } else {
            self.stop().await;
        }
        self.ensure_ports_free(&options.ports, was_running).await?;
        if was_running && !keep_singbox && (had_tun || options.tun) {
            wait_tun_removed().await;
        }
        self.tun_active = options.tun;
        let ready_port = options.ready_port;
        if let Some(config) = &plan.xray {
            let process = self.spawn(CoreKind::Xray, config).await?;
            self.xray = Some(process);
            if let Some(port) = ready_port {
                self.wait_ready(CoreKind::Xray, port).await?;
            }
        }
        if let Some(config) = plan.singbox.as_ref().filter(|_| !keep_singbox) {
            let process = self.spawn(CoreKind::SingBox, config).await?;
            self.singbox = Some(process);
            self.singbox_config = Some(config.clone());
            if let Some(port) = ready_port.filter(|_| plan.xray.is_none()) {
                self.wait_ready(CoreKind::SingBox, port).await?;
            } else {
                tokio::time::sleep(Duration::from_millis(800)).await;
                if !self.singbox.as_ref().is_some_and(CoreProcess::is_alive) {
                    self.stop().await;
                    bail!("sing-box exited right after start, see the journal (TUN needs administrator rights)");
                }
            }
        }
        Ok(())
    }

    /// The local ports must be free before a core starts: if another process still listens on
    /// one, the new core cannot bind it and exits, while the readiness check would wrongly
    /// succeed through the other process (e.g. a core left over with an old server, so the
    /// connection "works" but always through that server).
    async fn ensure_ports_free(&self, ports: &[u16], was_running: bool) -> Result<()> {
        let grace = if was_running { Duration::from_secs(3) } else { Duration::from_millis(300) };
        if wait_ports_free(ports, grace).await.is_none() {
            return Ok(());
        }
        let stopped = stop_leftover_cores(&self.paths).await;
        if stopped > 0 {
            self.log(crate::model::LogLevel::Warning, format!("stopped {stopped} core process(es) left over from an earlier run"));
        }
        if let Some(port) = wait_ports_free(ports, Duration::from_secs(3)).await {
            bail!("local port {port} is in use by another program; close it or change the port in the settings");
        }
        Ok(())
    }

    fn log(&self, level: crate::model::LogLevel, message: String) {
        tracing::warn!("{message}");
        (self.sink)(CoreEvent::Log(LogEntry {
            at: chrono::Utc::now(),
            level,
            source: crate::model::LogSource::App,
            message,
            request: None,
        }));
    }

    async fn spawn(&self, kind: CoreKind, config: &Value) -> Result<CoreProcess> {
        let exe = self.paths.ensure(kind)?;
        let file = self.paths.config_dir.join(match kind {
            CoreKind::Xray => "xray-config.json",
            CoreKind::SingBox => "singbox-config.json",
        });
        write_config(&file, config)?;
        let args: Vec<String> = match kind {
            CoreKind::Xray => vec!["run".into(), "-c".into(), file.display().to_string()],
            CoreKind::SingBox => {
                vec!["run".into(), "-c".into(), file.display().to_string(), "-D".into(), self.paths.config_dir.display().to_string()]
            }
        };
        CoreProcess::spawn(kind, &exe, &args, &self.paths.dir, self.sink.clone())
    }

    async fn wait_ready(&mut self, kind: CoreKind, port: u16) -> Result<()> {
        let alive = || match kind {
            CoreKind::Xray => self.xray.as_ref().is_some_and(CoreProcess::is_alive),
            CoreKind::SingBox => self.singbox.as_ref().is_some_and(CoreProcess::is_alive),
        };
        let ready = wait_for_port(port, Duration::from_secs(8), &alive).await && {
            // The port answering is not enough: the core must still be running (and so be the
            // one listening) a moment later.
            tokio::time::sleep(Duration::from_millis(150)).await;
            alive()
        };
        if !ready {
            self.stop().await;
            bail!("{} did not start listening on port {port}, see the journal", kind.label());
        }
        Ok(())
    }

    pub async fn stop(&mut self) {
        self.tun_active = false;
        self.singbox_config = None;
        if let Some(mut p) = self.singbox.take() {
            p.kill().await;
        }
        if let Some(mut p) = self.xray.take() {
            p.kill().await;
        }
    }
}

/// Stops Xray / sing-box processes started from our cores folder that this app instance did
/// not start (left running by an earlier instance). Returns how many were stopped.
pub async fn stop_leftover_cores(paths: &CorePaths) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let exes: Vec<PathBuf> = [CoreKind::Xray, CoreKind::SingBox].iter().map(|k| canonical(&paths.exe(*k))).collect();
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    let leftovers: Vec<u32> = system
        .processes()
        .iter()
        .filter(|(pid, p)| !is_own_pid(pid.as_u32()) && p.exe().is_some_and(|exe| exes.contains(&canonical(exe))))
        .map(|(pid, _)| pid.as_u32())
        .collect();
    for &pid in &leftovers {
        tracing::warn!("stopping leftover core process {pid}");
        request_stop(pid);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    loop {
        system.refresh_processes(ProcessesToUpdate::Some(&leftovers.iter().map(|p| sysinfo::Pid::from_u32(*p)).collect::<Vec<_>>()), true);
        let running: Vec<&sysinfo::Process> = leftovers.iter().filter_map(|p| system.process(sysinfo::Pid::from_u32(*p))).collect();
        if running.is_empty() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            for process in running {
                process.kill();
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    leftovers.len()
}

fn canonical(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path
    }
}

/// After the TUN core stopped, waits for its adapter to disappear before a new one is created.
#[cfg(windows)]
async fn wait_tun_removed() {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    tokio::time::sleep(Duration::from_millis(300)).await;
    while adapter_exists(crate::config::singbox::TUN_INTERFACE) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    // Routes of the removed interface go away slightly after the adapter itself.
    tokio::time::sleep(Duration::from_millis(300)).await;
}

#[cfg(not(windows))]
async fn wait_tun_removed() {
    tokio::time::sleep(Duration::from_millis(1200)).await;
}

#[cfg(windows)]
fn adapter_exists(name: &str) -> bool {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_UNICAST,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::Networking::WinSock::AF_UNSPEC;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_SKIP_UNICAST;
    let mut size = 32 * 1024u32;
    for _ in 0..3 {
        // u64 elements keep the buffer aligned for IP_ADAPTER_ADDRESSES_LH.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let status = unsafe { GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(first), &mut size) };
        if status == 111 {
            continue; // ERROR_BUFFER_OVERFLOW: `size` now holds the needed size
        }
        if status != 0 {
            return false;
        }
        let mut current = first.cast_const();
        while !current.is_null() {
            let adapter = unsafe { &*current };
            if unsafe { adapter.FriendlyName.to_string() }.is_ok_and(|n| n.eq_ignore_ascii_case(name)) {
                return true;
            }
            current = adapter.Next;
        }
        return false;
    }
    false
}

pub fn write_config(path: &Path, config: &Value) -> Result<()> {
    crate::storage::write_atomic(path, serde_json::to_string_pretty(config)?.as_bytes())
}

/// Runs `<core> version` and returns the version number, e.g. `26.3.27` / `1.14.4`.
pub async fn core_version(paths: &CorePaths, kind: CoreKind) -> Option<String> {
    let exe = paths.ensure(kind).ok()?;
    let mut command = tokio::process::Command::new(exe);
    command.arg("version");
    process::hide_window(&mut command);
    let output = tokio::time::timeout(Duration::from_secs(5), command.output()).await.ok()?.ok()?;
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

fn parse_version(output: &str) -> Option<String> {
    let first = output.lines().next()?;
    first
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.contains('.'))
        .map(|w| w.trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_core_versions() {
        assert_eq!(
            super::parse_version("Xray 26.3.27 (Xray, Penetrates Everything.) Custom (go1.26 linux/amd64)\n").as_deref(),
            Some("26.3.27")
        );
        assert_eq!(super::parse_version("sing-box version 1.14.4\n\nEnvironment: go1.26").as_deref(), Some("1.14.4"));
        assert_eq!(super::parse_version("sing-box version unknown"), None);
    }
}
