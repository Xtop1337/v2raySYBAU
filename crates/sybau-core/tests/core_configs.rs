//! Validates generated configs with real cores. Runs only when `XRAY_BIN` / `SINGBOX_BIN` are set:
//! `XRAY_BIN=/path/xray SINGBOX_BIN=/path/sing-box cargo test -p sybau-core --test core_configs`

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use sybau_core::config::{self, singbox, xray, ConfigEnv};
use sybau_core::model::{RoutingRule, RuleKind, RuleOutbound};
use sybau_core::settings::{ConnectionMode, RoutingPreset, Settings, TunAppMode};
use sybau_core::share::parse_link;

const LINKS: &[&str] = &[
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@de.example.com:443?encryption=none&flow=xtls-rprx-vision&security=reality&sni=www.microsoft.com&fp=chrome&pbk=Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw&sid=6ba85179e30d4fc2&type=tcp#reality",
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@1.2.3.4:443?type=ws&security=tls&path=%2Fws&host=cdn.example.com&sni=cdn.example.com#ws",
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@a.com:443?type=grpc&serviceName=svc&mode=multi&security=tls#grpc",
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@a.com:443?type=xhttp&path=%2Fx&mode=packet-up&security=reality&pbk=Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw&sni=a.com&sid=ab#xhttp",
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@a.com:80?type=httpupgrade&path=%2Fu#hu",
    "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@a.com:80?type=tcp&headerType=http&host=a.com&path=%2F#tcp-http",
    "vmess://eyJ2IjoiMiIsInBzIjoidm0iLCJhZGQiOiJ2bS5leGFtcGxlLmNvbSIsInBvcnQiOiI0NDMiLCJpZCI6ImI4MzEzODFkLTYzMjQtNGQ1My1hZDRmLThjZGE0OGIzMDgxMSIsImFpZCI6IjAiLCJzY3kiOiJhdXRvIiwibmV0Ijoid3MiLCJ0eXBlIjoibm9uZSIsImhvc3QiOiJ2bS5leGFtcGxlLmNvbSIsInBhdGgiOiIvdm0iLCJ0bHMiOiJ0bHMiLCJzbmkiOiJ2bS5leGFtcGxlLmNvbSJ9",
    "trojan://password@t.example.com:443?sni=t.example.com#trojan",
    "ss://YWVzLTI1Ni1nY206cGFzcw@ss.example.com:8388#ss",
    "ss://2022-blake3-aes-128-gcm:MTIzNDU2Nzg5MDEyMzQ1Ng%3D%3D@1.2.3.4:443#ss2022",
];
const SINGBOX_LINKS: &[&str] = &[
    "hy2://pw@hy.example.com:443/?sni=hy.example.com&obfs=salamander&obfs-password=x&mport=20000-30000#hy2",
    "tuic://2dd9ab4f-0b34-4b84-a4a0-1df2f5e5f4c6:pw@tu.example.com:443?congestion_control=bbr&alpn=h3&sni=tu.example.com#tuic",
];

fn bin(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.exists())
}

fn check(bin: &PathBuf, args: &[&str], config: &Value, name: &str) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, serde_json::to_string_pretty(config).unwrap()).unwrap();
    let mut command = Command::new(bin);
    command.args(args).arg(&path);
    if let Some(assets) = bin.parent() {
        command.env("XRAY_LOCATION_ASSET", assets);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{name}: config rejected\n{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        serde_json::to_string_pretty(config).unwrap()
    );
}

fn variants() -> Vec<(String, Settings)> {
    let mut list = vec![("default".to_string(), Settings::default())];
    let mut all = Settings::default();
    all.mux.enabled = true;
    all.fragment.enabled = true;
    all.dns.local_dns = true;
    all.dns.fake_dns = true;
    all.dns.hosts = "example.com 1.2.3.4".into();
    all.routing.preset = RoutingPreset::BypassLanRu;
    all.routing.route_only = true;
    all.routing.rules = vec![
        RoutingRule { kind: RuleKind::Domain, values: vec!["domain:example.org".into(), "full:a.b.c".into(), "keyword:ads".into()], outbound: RuleOutbound::Block, ..Default::default() },
        RoutingRule { kind: RuleKind::Ip, values: vec!["8.8.4.4/32".into()], outbound: RuleOutbound::Direct, ..Default::default() },
        RoutingRule { kind: RuleKind::Port, values: vec!["6881-6889,51413".into()], outbound: RuleOutbound::Direct, ..Default::default() },
        RoutingRule { kind: RuleKind::Protocol, values: vec!["bittorrent".into()], outbound: RuleOutbound::Direct, ..Default::default() },
        RoutingRule { kind: RuleKind::Network, values: vec!["udp".into()], outbound: RuleOutbound::Proxy, ..Default::default() },
        RoutingRule { kind: RuleKind::Process, values: vec!["steam.exe".into()], outbound: RuleOutbound::Direct, ..Default::default() },
    ];
    list.push(("everything".into(), all.clone()));
    let mut direct = all.clone();
    direct.routing.preset = RoutingPreset::DirectByDefault;
    direct.dns.direct = "https://77.88.8.8/dns-query".into();
    list.push(("direct-default".into(), direct));
    let mut tun = all.clone();
    tun.proxy.mode = ConnectionMode::Tun;
    tun.tun.app_mode = TunAppMode::Include;
    tun.tun.apps = vec!["chrome.exe".into()];
    list.push(("tun-include".into(), tun.clone()));
    tun.tun.app_mode = TunAppMode::Exclude;
    tun.routing.preset = RoutingPreset::Global;
    list.push(("tun-exclude".into(), tun));
    list
}

#[test]
fn xray_accepts_generated_configs() {
    let Some(xray_bin) = bin("XRAY_BIN") else {
        eprintln!("XRAY_BIN not set, skipping");
        return;
    };
    let env = ConfigEnv { geo_available: xray_bin.with_file_name("geosite.dat").exists(), ..ConfigEnv::default() };
    for (variant, settings) in variants() {
        for link in LINKS {
            let profile = parse_link(link).unwrap();
            let config = xray::build_client(&profile, &settings, &env, settings.proxy.mode == ConnectionMode::Proxy);
            check(&xray_bin, &["run", "-test", "-c"], &config, &format!("{variant}/{}", profile.name));
        }
        let profiles: Vec<_> = LINKS.iter().map(|l| parse_link(l).unwrap()).collect();
        let entries: Vec<_> = profiles.iter().enumerate().map(|(i, p)| (p, 30000 + i as u16)).collect();
        check(&xray_bin, &["run", "-test", "-c"], &xray::build_batch(&entries, &settings), &format!("{variant}/batch"));
    }
}

#[test]
fn singbox_accepts_generated_configs() {
    let Some(singbox_bin) = bin("SINGBOX_BIN") else {
        eprintln!("SINGBOX_BIN not set, skipping");
        return;
    };
    let env = ConfigEnv::default();
    for (variant, settings) in variants() {
        for link in SINGBOX_LINKS.iter().chain(LINKS.iter()) {
            let profile = parse_link(link).unwrap();
            let plan = config::plan(&profile, &settings, &env);
            if let Some(sb) = plan.singbox {
                check(&singbox_bin, &["check", "-c"], &sb, &format!("{variant}/{}", profile.name));
            }
        }
        let profiles: Vec<_> = SINGBOX_LINKS.iter().map(|l| parse_link(l).unwrap()).collect();
        let entries: Vec<_> = profiles.iter().enumerate().map(|(i, p)| (p, 31000 + i as u16)).collect();
        check(&singbox_bin, &["check", "-c"], &singbox::build_batch(&entries, &settings), &format!("{variant}/batch"));
    }
}
