use serde_json::Value;

use super::*;
use crate::model::{RoutingRule, RuleKind, RuleOutbound};
use crate::settings::{RoutingPreset, TunAppMode};
use crate::share::parse_link;

fn reality() -> Profile {
    parse_link("vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@de.example.com:443?encryption=none&flow=xtls-rprx-vision&security=reality&sni=www.microsoft.com&fp=chrome&pbk=Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw&sid=6ba85179e30d4fc2&type=tcp#DE").unwrap()
}

fn find<'a>(items: &'a Value, key: &str, value: &str) -> &'a Value {
    items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item[key] == value)
        .unwrap_or_else(|| panic!("no item with {key}={value}"))
}

#[test]
fn proxy_mode_uses_xray_only() {
    let plan = plan(&reality(), &Settings::default(), &ConfigEnv::default());
    assert!(plan.singbox.is_none());
    let config = plan.xray.unwrap();
    let proxy = &config["outbounds"][0];
    assert_eq!(proxy["tag"], "proxy");
    assert_eq!(proxy["settings"]["vnext"][0]["users"][0]["flow"], "xtls-rprx-vision");
    assert_eq!(proxy["streamSettings"]["security"], "reality");
    assert_eq!(proxy["streamSettings"]["realitySettings"]["publicKey"], "Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw");
    assert_eq!(find(&config["inbounds"], "tag", "socks")["port"], 10808);
    assert_eq!(find(&config["inbounds"], "tag", "http")["port"], 10809);
    // Server domain resolved by the direct DNS.
    let servers = config["dns"]["servers"].as_array().unwrap();
    assert!(servers.iter().any(|s| s["domains"].as_array().is_some_and(|d| d.contains(&"full:de.example.com".into()))));
}

#[test]
fn mux_is_skipped_for_vision_and_applied_otherwise() {
    let mut settings = Settings::default();
    settings.mux.enabled = true;
    assert!(xray::outbound(&reality(), &settings).get("mux").is_none());
    let ws = parse_link("vless://id@a.com:443?type=ws&security=tls&path=%2Fws#ws").unwrap();
    assert_eq!(xray::outbound(&ws, &settings)["mux"]["concurrency"], 8);
}

#[test]
fn fragment_adds_dialer_proxy() {
    let mut settings = Settings::default();
    settings.fragment.enabled = true;
    let config = xray::build_client(&reality(), &settings, &ConfigEnv::default(), true);
    assert_eq!(config["outbounds"][0]["streamSettings"]["sockopt"]["dialerProxy"], "fragment");
    let fragment = find(&config["outbounds"], "tag", "fragment");
    assert_eq!(fragment["settings"]["fragment"]["packets"], "tlshello");
}

#[test]
fn dns_options() {
    let mut settings = Settings::default();
    settings.dns.local_dns = true;
    settings.dns.fake_dns = true;
    let mut profile = reality();
    profile.dns = "https://9.9.9.9/dns-query".into();
    let config = xray::build_client(&profile, &settings, &ConfigEnv::default(), true);
    assert_eq!(config["dns"]["servers"][0], "fakedns");
    assert_eq!(config["dns"]["servers"][1], "https://9.9.9.9/dns-query");
    assert_eq!(find(&config["inbounds"], "tag", "dns-in")["port"], 10853);
    assert!(config["fakedns"].is_array());
    settings.dns.use_profile_dns = false;
    let config = xray::build_client(&profile, &settings, &ConfigEnv::default(), true);
    assert_eq!(config["dns"]["servers"][1], "https://1.1.1.1/dns-query");
}

#[test]
fn routing_presets_and_custom_rules() {
    let mut settings = Settings::default();
    settings.routing.preset = RoutingPreset::BypassLanRu;
    settings.routing.route_only = true;
    settings.routing.rules.push(RoutingRule {
        kind: RuleKind::Domain,
        values: vec!["domain:example.org".into()],
        outbound: RuleOutbound::Block,
        ..RoutingRule::default()
    });
    settings.routing.rules.push(RoutingRule {
        kind: RuleKind::Process,
        values: vec!["steam.exe".into()],
        outbound: RuleOutbound::Direct,
        ..RoutingRule::default()
    });
    let config = xray::build_client(&reality(), &settings, &ConfigEnv::default(), true);
    let rules = config["routing"]["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| r["outboundTag"] == "block" && r["domain"][0] == "domain:example.org"));
    assert!(rules.iter().any(|r| r["ip"][0] == "geoip:ru"));
    assert!(!rules.iter().any(|r| r.to_string().contains("steam.exe")));
    assert_eq!(config["inbounds"][0]["sniffing"]["routeOnly"], true);

    let no_geo = ConfigEnv { geo_available: false, ..ConfigEnv::default() };
    let config = xray::build_client(&reality(), &settings, &no_geo, true);
    assert!(!config.to_string().contains("geoip:") && !config.to_string().contains("geosite:"));
}

#[test]
fn tun_mode_runs_singbox_in_front_of_xray_with_per_app() {
    let mut settings = Settings::default();
    settings.proxy.mode = crate::settings::ConnectionMode::Tun;
    settings.tun.app_mode = TunAppMode::Include;
    settings.tun.apps = vec!["chrome.exe".into(), "Telegram.exe".into()];
    let plan = plan(&reality(), &settings, &ConfigEnv::default());
    assert!(plan.xray.is_some());
    let sb = plan.singbox.unwrap();
    assert_eq!(sb["inbounds"][0]["type"], "tun");
    assert_eq!(sb["outbounds"][0]["type"], "socks");
    let rules = sb["route"]["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| r["process_name"][0] == "xray.exe" && r["outbound"] == "direct"));
    assert!(rules.iter().any(|r| r["process_name"][0] == "chrome.exe" && r["invert"] == true));
    assert_eq!(sb["route"]["find_process"], true);
}

#[test]
fn hysteria2_uses_singbox_natively() {
    let hy = parse_link("hy2://pw@hy.example.com:443/?sni=hy.example.com&obfs=salamander&obfs-password=x&mport=20000-30000#HY").unwrap();
    let plan = plan(&hy, &Settings::default(), &ConfigEnv::default());
    assert!(plan.xray.is_none());
    let sb = plan.singbox.unwrap();
    assert_eq!(sb["inbounds"][0]["type"], "mixed");
    let out = &sb["outbounds"][0];
    assert_eq!(out["type"], "hysteria2");
    assert_eq!(out["obfs"]["type"], "salamander");
    assert_eq!(out["server_ports"][0], "20000:30000");
}

#[test]
fn singbox_dns_server_specs() {
    assert_eq!(singbox::dns_server("https://1.1.1.1/dns-query", "r"), serde_json::json!({"type": "https", "tag": "r", "server": "1.1.1.1"}));
    assert_eq!(singbox::dns_server("8.8.8.8", "l"), serde_json::json!({"type": "udp", "tag": "l", "server": "8.8.8.8"}));
    let tls = singbox::dns_server("tls://dns.google:853", "t");
    assert_eq!(tls["server"], "dns.google");
    assert_eq!(tls["server_port"], 853);
    assert_eq!(tls["domain_resolver"], "local");
    assert_eq!(dns_host("[2606:4700::1111]:53"), "2606:4700::1111");
}

#[test]
fn batch_config_maps_inbounds_to_outbounds() {
    let a = reality();
    let b = parse_link("trojan://pw@t.com:443#T").unwrap();
    let config = xray::build_batch(&[(&a, 20000), (&b, 20001)], &Settings::default());
    assert_eq!(config["inbounds"][1]["port"], 20001);
    assert_eq!(config["outbounds"][1]["protocol"], "trojan");
    assert_eq!(config["routing"]["rules"][1]["outboundTag"], "out-1");
}

#[test]
fn override_is_merged() {
    let mut settings = Settings::default();
    settings.advanced.config_override = r#"{"log": {"loglevel": "debug"}}"#.into();
    let config = xray::build_client(&reality(), &settings, &ConfigEnv::default(), true);
    assert_eq!(config["log"]["loglevel"], "debug");
}
