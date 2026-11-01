//! Switching servers while connected, against local Xray servers. Runs only when `XRAY_BIN` is
//! set (`SINGBOX_BIN` too for the TUN-over-Xray part). Each server leaves from its own loopback
//! address, so the address an echo service sees tells which server carried the request.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use sybau_core::config::{self, ConfigEnv, LaunchPlan};
use sybau_core::model::Profile;
use sybau_core::runtime::{free_ports, wait_for_port, CoreKind, CoreManager, CorePaths, StartOptions};
use sybau_core::settings::{RoutingPreset, Settings};
use sybau_core::share::parse_link;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const UUID: &str = "3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a";

/// Answers every request with the caller's address.
async fn echo_server() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, peer)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let body = format!("ip={}\n", peer.ip());
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    port
}

/// Address the echo service sees for a request through the local SOCKS port.
async fn exit_ip(socks: u16, echo: u16) -> Option<String> {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks}")).unwrap())
        .timeout(Duration::from_secs(4))
        .pool_max_idle_per_host(0)
        .build()
        .unwrap();
    let body = client.get(format!("http://127.0.0.1:{echo}/")).send().await.ok()?.text().await.ok()?;
    body.trim().strip_prefix("ip=").map(str::to_string)
}

fn server_config(port: u16, send_through: &str) -> Value {
    json!({
        "log": {"loglevel": "warning"},
        "inbounds": [{"listen": "127.0.0.1", "port": port, "protocol": "vless", "settings": {"clients": [{"id": UUID}], "decryption": "none"}}],
        "outbounds": [{"protocol": "freedom", "sendThrough": send_through, "settings": {"finalRules": [{"action": "allow", "ip": ["127.0.0.0/8"]}]}}]
    })
}

struct Servers {
    _children: Vec<std::process::Child>,
}

impl Drop for Servers {
    fn drop(&mut self) {
        for child in &mut self._children {
            let _ = child.kill();
        }
    }
}

fn run_xray(exe: &PathBuf, file: &std::path::Path, config: &Value) -> std::process::Child {
    std::fs::write(file, config.to_string()).unwrap();
    Command::new(exe).args(["run", "-c"]).arg(file).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap()
}

#[tokio::test]
async fn switching_servers_always_changes_the_exit_address() {
    let Some(xray) = std::env::var_os("XRAY_BIN").map(PathBuf::from) else {
        eprintln!("XRAY_BIN not set, skipping");
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let cores = work.path().join("cores");
    std::fs::create_dir_all(&cores).unwrap();
    let core_exe = cores.join(CoreKind::Xray.exe_name());
    std::fs::copy(&xray, &core_exe).unwrap();
    let singbox = std::env::var_os("SINGBOX_BIN").map(PathBuf::from);
    if let Some(singbox) = &singbox {
        std::fs::copy(singbox, cores.join(CoreKind::SingBox.exe_name())).unwrap();
    }
    let paths = CorePaths { dir: cores.clone(), config_dir: work.path().to_path_buf() };

    let ports = free_ports(3).unwrap();
    let exits = ["127.0.0.3", "127.0.0.4", "127.0.0.5"];
    let mut children = Vec::new();
    for (i, port) in ports.iter().enumerate() {
        children.push(run_xray(&xray, &work.path().join(format!("server-{i}.json")), &server_config(*port, exits[i])));
    }
    let _servers = Servers { _children: children };
    for port in &ports {
        assert!(wait_for_port(*port, Duration::from_secs(5), || true).await);
    }
    let echo = echo_server().await;
    let profiles: Vec<Profile> =
        ports.iter().enumerate().map(|(i, port)| parse_link(&format!("vless://{UUID}@127.0.0.1:{port}?type=tcp#S{i}")).unwrap()).collect();

    let mut settings = Settings::default();
    let local = free_ports(3).unwrap();
    settings.proxy.socks_port = local[0];
    settings.proxy.http_port = local[1];
    settings.advanced.api_port = local[2];
    settings.routing.preset = RoutingPreset::Global;
    let env = ConfigEnv { geo_available: false, ..ConfigEnv::default() };
    let plan_of = |p: &Profile| config::plan(p, &settings, &env);
    let options = |plan: &LaunchPlan| StartOptions::for_plan(plan, &settings);

    // A core left running by an earlier app instance (e.g. closed by the installer) holds the
    // local ports with server C. Before the fix the new core failed to bind, the readiness
    // check succeeded through the old one, and every server "connected" through server C.
    let leftover_file = work.path().join("leftover.json");
    let mut leftover = run_xray(&core_exe, &leftover_file, &plan_of(&profiles[2]).xray.unwrap());
    assert!(wait_for_port(settings.proxy.socks_port, Duration::from_secs(5), || true).await);
    assert_eq!(exit_ip(settings.proxy.socks_port, echo).await.as_deref(), Some(exits[2]));

    let mut manager = CoreManager::new(paths.clone(), std::sync::Arc::new(|_| {}));
    let plan = plan_of(&profiles[0]);
    manager.start(&plan, &options(&plan)).await.unwrap();
    assert_eq!(exit_ip(settings.proxy.socks_port, echo).await.as_deref(), Some(exits[0]), "leftover core still carries the traffic");
    assert!(leftover.try_wait().unwrap().is_some(), "leftover core was not stopped");

    // Many switches in a row: every one must move the traffic to the chosen server.
    for round in 0..8 {
        let i = round % 2;
        let plan = plan_of(&profiles[i]);
        manager.start(&plan, &options(&plan)).await.unwrap();
        assert_eq!(exit_ip(settings.proxy.socks_port, echo).await.as_deref(), Some(exits[i]), "switch {round}");
    }
    manager.stop().await;

    // Another program on the port: connecting must fail instead of "working" through it.
    let blocker = std::net::TcpListener::bind(("127.0.0.1", settings.proxy.socks_port)).unwrap();
    let plan = plan_of(&profiles[0]);
    let error = manager.start(&plan, &options(&plan)).await.expect_err("started on a busy port");
    assert!(format!("{error:#}").contains("in use"), "{error:#}");
    drop(blocker);
    manager.stop().await;

    // TUN over Xray (here with a plain sing-box inbound instead of a TUN adapter): switching
    // servers restarts only Xray; sing-box keeps running while its config stays the same.
    let Some(_) = singbox else {
        eprintln!("SINGBOX_BIN not set, skipping the sing-box part");
        return;
    };
    let sb_port = free_ports(1).unwrap()[0];
    let sb_config = |port: u16| {
        json!({
            "log": {"level": "warn"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": port}],
            "outbounds": [{"type": "socks", "server": "127.0.0.1", "server_port": settings.proxy.socks_port}]
        })
    };
    let with_singbox = |p: &Profile, port: u16| LaunchPlan { xray: plan_of(p).xray, singbox: Some(sb_config(port)) };
    let plan = with_singbox(&profiles[0], sb_port);
    manager.start(&plan, &options(&plan)).await.unwrap();
    let (xray_a, sb_a) = manager.pids();
    assert!(wait_for_port(sb_port, Duration::from_secs(5), || true).await);
    assert_eq!(exit_ip(sb_port, echo).await.as_deref(), Some(exits[0]));

    let plan = with_singbox(&profiles[1], sb_port);
    manager.start(&plan, &options(&plan)).await.unwrap();
    let (xray_b, sb_b) = manager.pids();
    assert_eq!(sb_a, sb_b, "sing-box was restarted for a server switch");
    assert_ne!(xray_a, xray_b);
    assert_eq!(exit_ip(sb_port, echo).await.as_deref(), Some(exits[1]), "traffic through the kept sing-box");

    let other_port = free_ports(1).unwrap()[0];
    let plan = with_singbox(&profiles[0], other_port);
    manager.start(&plan, &options(&plan)).await.unwrap();
    assert_ne!(manager.pids().1, sb_b, "a different sing-box config must restart it");
    assert!(wait_for_port(other_port, Duration::from_secs(5), || true).await);
    assert_eq!(exit_ip(other_port, echo).await.as_deref(), Some(exits[0]));
    manager.stop().await;
}
