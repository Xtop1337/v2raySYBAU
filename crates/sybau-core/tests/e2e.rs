//! End-to-end test against a local Xray server. Runs only when `XRAY_BIN` is set.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use sybau_core::config::{self, ConfigEnv};
use sybau_core::model::TestMethod;
use sybau_core::runtime::{
    free_ports, CoreEvent, CoreKind, CoreManager, CorePaths, CoreProcess, StartOptions, TrafficMonitor, TrafficSample,
};
use sybau_core::settings::{RoutingPreset, Settings};
use sybau_core::share::parse_link;
use sybau_core::testing::{http_delay, run_batch, BatchOptions, DelayResult};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

const UUID: &str = "3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a";

/// Minimal HTTP server answering `204 No Content`, standing in for generate_204.
async fn http_204() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            });
        }
    });
    port
}

#[tokio::test]
async fn batch_test_connect_and_traffic() {
    let Some(xray) = std::env::var_os("XRAY_BIN").map(PathBuf::from) else {
        eprintln!("XRAY_BIN not set, skipping");
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let cores = work.path().join("cores");
    std::fs::create_dir_all(&cores).unwrap();
    std::fs::copy(&xray, cores.join(CoreKind::Xray.exe_name())).unwrap();
    let paths = CorePaths { dir: cores.clone(), config_dir: work.path().to_path_buf() };

    let ports = free_ports(4).unwrap();
    let (vless_port, vmess_port, ss_port, dead_port) = (ports[0], ports[1], ports[2], ports[3]);
    let server = json!({
        "log": {"loglevel": "warning"},
        "inbounds": [
            {"listen": "127.0.0.1", "port": vless_port, "protocol": "vless", "settings": {"clients": [{"id": UUID}], "decryption": "none"}, "streamSettings": {"network": "ws", "wsSettings": {"path": "/ws"}}},
            {"listen": "127.0.0.1", "port": vmess_port, "protocol": "vmess", "settings": {"clients": [{"id": UUID}]}},
            {"listen": "127.0.0.1", "port": ss_port, "protocol": "shadowsocks", "settings": {"method": "aes-128-gcm", "password": "secret", "network": "tcp,udp"}}
        ],
        // Xray 26.10+ blocks private targets for VLESS/VMess/SS inbounds unless allowed.
        "outbounds": [{"protocol": "freedom", "settings": {"finalRules": [{"action": "allow", "ip": ["127.0.0.0/8"]}]}}]
    });
    let server_file = work.path().join("server.json");
    std::fs::write(&server_file, server.to_string()).unwrap();
    let logs: Arc<Mutex<Vec<String>>> = Arc::default();
    let server_logs = logs.clone();
    let mut server = CoreProcess::spawn(
        CoreKind::Xray,
        &cores.join(CoreKind::Xray.exe_name()),
        &["run".into(), "-c".into(), server_file.display().to_string()],
        &cores,
        Arc::new(move |e| {
            if let CoreEvent::Log(entry) = e {
                server_logs.lock().unwrap().push(entry.message);
            }
        }),
    )
    .unwrap();
    assert!(
        sybau_core::runtime::wait_for_port(ss_port, Duration::from_secs(5), || true).await,
        "server did not start: {:?}",
        logs.lock().unwrap()
    );

    let target = http_204().await;
    let vmess_json = json!({"v": "2", "ps": "vmess", "add": "127.0.0.1", "port": vmess_port.to_string(), "id": UUID, "aid": "0", "net": "tcp", "tls": ""});
    let profiles = vec![
        parse_link(&format!("vless://{UUID}@127.0.0.1:{vless_port}?type=ws&path=%2Fws#vless")).unwrap(),
        parse_link(&format!("vmess://{}", sybau_core::util::b64_encode(vmess_json.to_string().as_bytes()))).unwrap(),
        parse_link(&format!("ss://{}@127.0.0.1:{ss_port}#ss", sybau_core::util::b64_encode_url(b"aes-128-gcm:secret"))).unwrap(),
        parse_link(&format!("vless://{UUID}@127.0.0.1:{dead_port}#dead")).unwrap(),
        // Reality without a public key makes Xray reject the config: the batch must isolate it.
        parse_link(&format!("vless://{UUID}@127.0.0.1:{vless_port}?security=reality&sni=a.com#broken")).unwrap(),
    ];

    let mut settings = Settings::default();
    settings.test.url = format!("http://127.0.0.1:{target}/generate_204");
    settings.test.timeout_ms = 3000;
    let results: Arc<Mutex<Vec<DelayResult>>> = Arc::default();
    let sink_results = results.clone();
    run_batch(
        profiles.clone(),
        &settings,
        &paths,
        &BatchOptions::from_settings(&settings, TestMethod::Real),
        CancellationToken::new(),
        Arc::new(move |r| sink_results.lock().unwrap().push(r)),
    )
    .await
    .unwrap();
    let results = results.lock().unwrap().clone();
    let delay = |name: &str| {
        let id = profiles.iter().find(|p| p.name == name).unwrap().id;
        results.iter().find(|r| r.profile_id == id).unwrap_or_else(|| panic!("no result for {name}")).ms
    };
    assert!(delay("vless") > 0, "{results:?}");
    assert!(delay("vmess") > 0, "{results:?}");
    assert!(delay("ss") > 0, "{results:?}");
    assert_eq!(delay("dead"), -1);
    assert_eq!(delay("broken"), -1);

    // TCP ping of the live and dead servers.
    let tcp: Arc<Mutex<Vec<DelayResult>>> = Arc::default();
    let tcp_sink = tcp.clone();
    run_batch(
        profiles[2..4].to_vec(),
        &settings,
        &paths,
        &BatchOptions::from_settings(&settings, TestMethod::Tcp),
        CancellationToken::new(),
        Arc::new(move |r| tcp_sink.lock().unwrap().push(r)),
    )
    .await
    .unwrap();
    let tcp = tcp.lock().unwrap().clone();
    assert_eq!(tcp.len(), 2);
    assert!(tcp.iter().any(|r| r.ms > 0) && tcp.iter().any(|r| r.ms == -1));

    // Connect through the VLESS profile with the global preset and measure traffic.
    let client_ports = free_ports(3).unwrap();
    settings.proxy.socks_port = client_ports[0];
    settings.proxy.http_port = client_ports[1];
    settings.advanced.api_port = client_ports[2];
    settings.routing.preset = RoutingPreset::Global;
    let access: Arc<Mutex<Vec<String>>> = Arc::default();
    let access_sink = access.clone();
    let mut manager = CoreManager::new(
        paths.clone(),
        Arc::new(move |e| {
            if let CoreEvent::Log(entry) = e {
                if let Some(r) = entry.request {
                    access_sink.lock().unwrap().push(format!("{} {}", r.destination, r.outbound));
                }
            }
        }),
    );
    let env = ConfigEnv { geo_available: false, ..ConfigEnv::default() };
    let plan = config::plan(&profiles[0], &settings, &env);
    manager.start(&plan, &StartOptions::for_plan(&plan, &settings)).await.unwrap();
    assert!(manager.is_running());

    let samples: Arc<Mutex<Vec<TrafficSample>>> = Arc::default();
    let samples_sink = samples.clone();
    let cancel = CancellationToken::new();
    TrafficMonitor::Xray { exe: paths.exe(CoreKind::Xray), api_port: settings.advanced.api_port }
        .spawn(cancel.clone(), Arc::new(move |s| samples_sink.lock().unwrap().push(s)));
    for _ in 0..3 {
        assert!(http_delay(settings.proxy.socks_port, &settings.test.url, Duration::from_secs(3)).await.is_some());
    }
    tokio::time::sleep(Duration::from_millis(2500)).await;
    cancel.cancel();
    let last = *samples.lock().unwrap().last().expect("traffic samples");
    assert!(last.total_up > 0 && last.total_down > 0, "{:?}", samples.lock().unwrap());
    let access = access.lock().unwrap().clone();
    assert!(access.iter().any(|a| a == &format!("127.0.0.1:{target} proxy")), "{access:?}");

    // Switch to another location while connected: same local ports, new server B.
    let b_port = free_ports(1).unwrap()[0];
    let b_config = json!({
        "log": {"loglevel": "warning"},
        "inbounds": [{"listen": "127.0.0.1", "port": b_port, "protocol": "vless", "settings": {"clients": [{"id": UUID}], "decryption": "none"}}],
        "outbounds": [{"protocol": "freedom", "settings": {"finalRules": [{"action": "allow", "ip": ["127.0.0.0/8"]}]}}]
    });
    let b_file = work.path().join("server-b.json");
    std::fs::write(&b_file, b_config.to_string()).unwrap();
    let b_logs: Arc<Mutex<Vec<String>>> = Arc::default();
    let b_sink = b_logs.clone();
    let mut server_b = CoreProcess::spawn(
        CoreKind::Xray,
        &cores.join(CoreKind::Xray.exe_name()),
        &["run".into(), "-c".into(), b_file.display().to_string()],
        &cores,
        Arc::new(move |e| {
            if let CoreEvent::Log(entry) = e {
                b_sink.lock().unwrap().push(entry.message);
            }
        }),
    )
    .unwrap();
    assert!(sybau_core::runtime::wait_for_port(b_port, Duration::from_secs(5), || true).await);
    let profile_b = parse_link(&format!("vless://{UUID}@127.0.0.1:{b_port}?type=tcp#B")).unwrap();
    let plan_b = config::plan(&profile_b, &settings, &env);
    manager.start(&plan_b, &StartOptions::for_plan(&plan_b, &settings)).await.unwrap();
    assert!(http_delay(settings.proxy.socks_port, &settings.test.url, Duration::from_secs(3)).await.is_some());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        b_logs.lock().unwrap().iter().any(|l| l.contains(&format!("127.0.0.1:{target}"))),
        "server B saw no traffic: {:?}",
        b_logs.lock().unwrap()
    );

    manager.stop().await;
    assert!(!manager.is_running());
    server.kill().await;
    server_b.kill().await;
}
