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
}

impl CoreManager {
    pub fn new(paths: CorePaths, sink: EventSink) -> Self {
        CoreManager { paths, sink, xray: None, singbox: None }
    }

    pub fn paths(&self) -> &CorePaths {
        &self.paths
    }

    pub fn is_running(&self) -> bool {
        self.xray.as_ref().is_some_and(CoreProcess::is_alive) || self.singbox.as_ref().is_some_and(CoreProcess::is_alive)
    }

    /// Starts the cores of `plan` and waits until `ready_port` accepts connections.
    pub async fn start(&mut self, plan: &LaunchPlan, ready_port: Option<u16>) -> Result<()> {
        self.stop().await;
        if let Some(config) = &plan.xray {
            let process = self.spawn(CoreKind::Xray, config).await?;
            self.xray = Some(process);
            if let Some(port) = ready_port.filter(|_| plan.singbox.is_none()) {
                self.wait_ready(CoreKind::Xray, port).await?;
            }
        }
        if let Some(config) = &plan.singbox {
            if plan.xray.is_some() {
                // sing-box forwards to the Xray SOCKS inbound: give Xray a moment to listen.
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            let process = self.spawn(CoreKind::SingBox, config).await?;
            self.singbox = Some(process);
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
        let ready = wait_for_port(port, Duration::from_secs(8), alive).await;
        if !ready {
            self.stop().await;
            bail!("{} did not start listening on port {port}, see the journal", kind.label());
        }
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(mut p) = self.singbox.take() {
            p.kill().await;
        }
        if let Some(mut p) = self.xray.take() {
            p.kill().await;
        }
    }
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
