//! Programs that break the connection when they run next to the client: other VPNs take the
//! traffic first, DPI bypass tools rewrite packets (and break TLS/Reality), other proxy clients
//! hold their own TUN, the system proxy or the same ports. Found at launch, the user decides
//! whether to stop them.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictKind {
    /// Another VPN that routes all traffic.
    Vpn,
    /// A DPI bypass tool rewriting packets (WinDivert).
    Dpi,
    /// Another proxy client (own TUN, system proxy, ports).
    Client,
    /// Xray or sing-box started by another program.
    Core,
}

struct Known {
    id: &'static str,
    name: &'static str,
    kind: ConflictKind,
    /// Executable names, lower case.
    exes: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        id: "warp",
        name: "Cloudflare WARP (1.1.1.1)",
        kind: ConflictKind::Vpn,
        exes: &["warp-svc.exe", "cloudflare warp.exe", "warp-taskbar.exe"],
    },
    Known { id: "zapret", name: "zapret (winws)", kind: ConflictKind::Dpi, exes: &["winws.exe"] },
    Known { id: "goodbyedpi", name: "GoodbyeDPI", kind: ConflictKind::Dpi, exes: &["goodbyedpi.exe"] },
    Known { id: "byedpi", name: "ByeDPI", kind: ConflictKind::Dpi, exes: &["ciadpi.exe", "byedpi.exe"] },
    Known { id: "v2rayn", name: "v2rayN", kind: ConflictKind::Client, exes: &["v2rayn.exe"] },
    Known {
        id: "nekoray",
        name: "NekoRay / NekoBox",
        kind: ConflictKind::Client,
        exes: &["nekoray.exe", "nekobox.exe", "nekobox_core.exe"],
    },
    Known { id: "hiddify", name: "Hiddify", kind: ConflictKind::Client, exes: &["hiddify.exe", "hiddifycli.exe", "hiddifynext.exe"] },
    Known {
        id: "clash",
        name: "Clash / mihomo",
        kind: ConflictKind::Client,
        exes: &["clash-verge.exe", "verge-mihomo.exe", "mihomo.exe", "clash for windows.exe", "clash-meta.exe", "clash.exe", "flclash.exe"],
    },
    Known { id: "happ", name: "Happ", kind: ConflictKind::Client, exes: &["happ.exe"] },
    Known { id: "throne", name: "Throne", kind: ConflictKind::Client, exes: &["throne.exe"] },
    Known { id: "outline", name: "Outline", kind: ConflictKind::Vpn, exes: &["outline.exe", "outlineservice.exe"] },
    Known { id: "amnezia", name: "AmneziaVPN", kind: ConflictKind::Vpn, exes: &["amneziavpn.exe", "amneziavpn-service.exe"] },
    Known { id: "openvpn", name: "OpenVPN", kind: ConflictKind::Vpn, exes: &["openvpn.exe", "openvpn-gui.exe"] },
    Known { id: "wireguard", name: "WireGuard", kind: ConflictKind::Vpn, exes: &["wireguard.exe"] },
    Known { id: "psiphon", name: "Psiphon", kind: ConflictKind::Vpn, exes: &["psiphon3.exe"] },
];

/// Cores of other programs (ours, from the cores folder, are never reported).
const FOREIGN_CORES: &[&str] = &["xray.exe", "sing-box.exe"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictProcess {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ConflictKind,
    pub processes: Vec<ConflictProcess>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopResult {
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// A running process: id, executable name and path (when readable).
pub type ProcessInfo = (u32, String, Option<PathBuf>);

/// Which known programs are among `processes`. `own_dir` is our cores folder.
pub fn match_conflicts(processes: &[ProcessInfo], own_dir: &Path, own_pid: u32) -> Vec<Conflict> {
    let own_dir = normalize(own_dir);
    let mut found: Vec<Conflict> = Vec::new();
    for (pid, name, exe) in processes {
        if *pid == own_pid {
            continue;
        }
        let lower = name.to_lowercase();
        // On other systems the same programs run without ".exe".
        let key = if lower.ends_with(".exe") { lower.clone() } else { format!("{lower}.exe") };
        let (id, label, kind) = if let Some(known) = KNOWN.iter().find(|k| k.exes.contains(&key.as_str())) {
            (known.id, known.name, known.kind)
        } else if FOREIGN_CORES.contains(&key.as_str()) && !exe.as_deref().is_some_and(|e| normalize(e).starts_with(&own_dir)) {
            ("cores", "Xray / sing-box", ConflictKind::Core)
        } else {
            continue;
        };
        let process = ConflictProcess { pid: *pid, name: name.clone() };
        match found.iter_mut().find(|c| c.id == id) {
            Some(conflict) => conflict.processes.push(process),
            None => found.push(Conflict { id, name: label, kind, processes: vec![process] }),
        }
    }
    found.sort_by_key(|c| (c.kind as u8, c.name));
    found
}

fn normalize(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path
    }
}

fn running() -> (System, Vec<ProcessInfo>) {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet));
    let list = system
        .processes()
        .iter()
        .map(|(pid, p)| (pid.as_u32(), p.name().to_string_lossy().to_string(), p.exe().map(Path::to_path_buf)))
        .collect();
    (system, list)
}

/// Known conflicting programs running now.
pub fn detect(cores_dir: &Path) -> Vec<Conflict> {
    let (_, processes) = running();
    match_conflicts(&processes, cores_dir, std::process::id())
}

/// Stops the chosen programs. WARP is disconnected with `warp-cli` (its service runs as
/// SYSTEM and cannot be stopped without administrator rights; disconnecting needs none).
pub fn stop(ids: &[String], cores_dir: &Path) -> Vec<StopResult> {
    let (mut system, processes) = running();
    let conflicts = match_conflicts(&processes, cores_dir, std::process::id());
    let mut results = Vec::new();
    for conflict in conflicts.iter().filter(|c| ids.iter().any(|id| id == c.id)) {
        let outcome = if conflict.id == "warp" { disconnect_warp() } else { kill(&mut system, &conflict.processes) };
        results.push(StopResult { id: conflict.id.into(), name: conflict.name.into(), ok: outcome.is_ok(), error: outcome.err() });
    }
    results
}

fn kill(system: &mut System, processes: &[ConflictProcess]) -> Result<(), String> {
    let pids: Vec<Pid> = processes.iter().map(|p| Pid::from_u32(p.pid)).collect();
    for pid in &pids {
        if let Some(process) = system.process(*pid) {
            process.kill();
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(600));
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);
    let alive: Vec<String> = pids.iter().filter_map(|pid| system.process(*pid)).map(|p| p.name().to_string_lossy().to_string()).collect();
    if alive.is_empty() {
        Ok(())
    } else {
        Err(format!("NEEDS_ADMIN: {}", alive.join(", ")))
    }
}

#[cfg(windows)]
fn disconnect_warp() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    let installed = program_files.join("Cloudflare").join("Cloudflare WARP").join("warp-cli.exe");
    let cli = if installed.exists() { installed } else { PathBuf::from("warp-cli.exe") };
    let output = std::process::Command::new(&cli)
        .args(["--accept-tos", "disconnect"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("warp-cli: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(windows))]
fn disconnect_warp() -> Result<(), String> {
    let output =
        std::process::Command::new("warp-cli").args(["--accept-tos", "disconnect"]).output().map_err(|e| format!("warp-cli: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, name: &str, exe: Option<&str>) -> ProcessInfo {
        (pid, name.into(), exe.map(PathBuf::from))
    }

    #[test]
    fn finds_known_programs_and_foreign_cores_only() {
        let cores = PathBuf::from("/opt/sybau/cores");
        let processes = vec![
            p(10, "warp-svc.exe", None),
            p(11, "Cloudflare WARP.exe", None),
            p(12, "winws.exe", Some("C:/zapret/winws.exe")),
            p(13, "xray.exe", Some("/opt/sybau/cores/xray.exe")),
            p(14, "xray.exe", Some("/opt/v2rayN/bin/xray.exe")),
            p(15, "chrome.exe", None),
            p(16, "v2rayN.exe", None),
            p(99, "winws.exe", None),
        ];
        let found = match_conflicts(&processes, &cores, 99);
        let summary: Vec<(&str, Vec<u32>)> = found.iter().map(|c| (c.id, c.processes.iter().map(|p| p.pid).collect())).collect();
        assert_eq!(summary, vec![("warp", vec![10, 11]), ("zapret", vec![12]), ("v2rayn", vec![16]), ("cores", vec![14])]);
        assert_eq!(found[0].kind, ConflictKind::Vpn);
        assert_eq!(found[1].kind, ConflictKind::Dpi);
    }

    #[test]
    fn names_without_exe_match_too() {
        let found = match_conflicts(&[p(5, "winws", None)], Path::new("/nonexistent"), 1);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "zapret");
    }
}
