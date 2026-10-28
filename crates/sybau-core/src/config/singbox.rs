//! sing-box configuration (format of sing-box 1.12+): TUN mode, per-app proxy and the
//! Hysteria2/TUIC protocols that Xray does not provide.

use serde_json::{json, Value};

use super::{dns_host, is_ip, remote_dns, ConfigEnv, RU_DOMAIN_SUFFIXES};
use crate::model::{Profile, Protocol, RoutingRule, RuleKind, RuleOutbound};
use crate::settings::{RoutingPreset, Settings, TunAppMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Local mixed (SOCKS+HTTP) inbounds with a native Hysteria2/TUIC outbound.
    Proxy,
    /// TUN inbound forwarding to the local Xray SOCKS inbound.
    TunOverXray,
    /// TUN inbound with a native Hysteria2/TUIC outbound.
    TunNative,
}

pub const TUN_INTERFACE: &str = "SybauTun";

pub fn build(profile: &Profile, settings: &Settings, env: &ConfigEnv, mode: Mode) -> Value {
    let tun = mode != Mode::Proxy;
    let fake_dns = settings.dns.fake_dns && tun;

    let mut inbounds = Vec::new();
    if tun {
        let t = &settings.tun;
        inbounds.push(json!({
            "type": "tun", "tag": "tun-in", "interface_name": TUN_INTERFACE,
            "address": ["172.19.0.1/30", "fdfe:dcba:9876::1/126"],
            "mtu": t.mtu, "auto_route": true, "strict_route": t.strict_route, "stack": t.stack
        }));
    } else {
        let listen = if settings.proxy.allow_lan { "0.0.0.0" } else { "127.0.0.1" };
        inbounds.push(json!({"type": "mixed", "tag": "socks", "listen": listen, "listen_port": settings.proxy.socks_port}));
        inbounds.push(json!({"type": "mixed", "tag": "http", "listen": listen, "listen_port": settings.proxy.http_port}));
    }
    if settings.dns.local_dns {
        inbounds.push(json!({"type": "direct", "tag": "dns-in", "listen": "127.0.0.1", "listen_port": settings.dns.local_port}));
    }

    let mut proxy = match mode {
        Mode::TunOverXray => json!({
            "type": "socks", "server": "127.0.0.1", "server_port": settings.proxy.socks_port, "version": "5"
        }),
        _ => outbound(profile, settings),
    };
    proxy["tag"] = json!("proxy");

    let mut config = json!({
        "log": log(settings),
        "dns": dns(profile, settings, fake_dns),
        "inbounds": inbounds,
        "outbounds": [proxy, {"type": "direct", "tag": "direct"}],
        "route": route(profile, settings, env, mode),
        "experimental": {"clash_api": {"external_controller": format!("127.0.0.1:{}", env.clash_api_port)}}
    });
    if fake_dns {
        config["experimental"]["cache_file"] = json!({"enabled": true, "store_fakeip": true});
    }
    config
}

/// Batch latency test config for profiles that need sing-box.
pub fn build_batch(entries: &[(&Profile, u16)], settings: &Settings) -> Value {
    let mut inbounds = Vec::new();
    let mut outbounds = Vec::new();
    let mut rules = Vec::new();
    for (index, (profile, port)) in entries.iter().enumerate() {
        inbounds.push(json!({"type": "socks", "tag": format!("in-{index}"), "listen": "127.0.0.1", "listen_port": port}));
        let mut out = outbound(profile, settings);
        out["tag"] = json!(format!("out-{index}"));
        outbounds.push(out);
        rules.push(json!({"inbound": [format!("in-{index}")], "outbound": format!("out-{index}")}));
    }
    outbounds.push(json!({"type": "direct", "tag": "direct"}));
    json!({
        "log": {"disabled": true},
        "dns": {"servers": [{"type": "local", "tag": "local"}]},
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {"rules": rules, "final": "direct", "default_domain_resolver": "local"}
    })
}

fn log(settings: &Settings) -> Value {
    match settings.advanced.log_level.as_str() {
        "none" => json!({"disabled": true}),
        "warning" => json!({"level": "warn", "timestamp": true}),
        level => json!({"level": level, "timestamp": true}),
    }
}

/// Converts `https://1.1.1.1/dns-query`, `tls://dns.google`, `8.8.8.8` ... to a sing-box DNS server.
pub fn dns_server(spec: &str, tag: &str) -> Value {
    let spec = spec.trim();
    let (scheme, rest) = spec.split_once("://").unwrap_or(("udp", spec));
    let host = dns_host(spec);
    let after_host = rest.split_once('/').map(|(_, path)| format!("/{path}"));
    let port = rest
        .split('/')
        .next()
        .and_then(|authority| authority.rsplit_once(':'))
        .and_then(|(_, port)| port.parse::<u16>().ok());
    let kind = match scheme.to_ascii_lowercase().as_str() {
        "https" => "https",
        "h3" => "h3",
        "tls" => "tls",
        "quic" => "quic",
        "tcp" => "tcp",
        _ => "udp",
    };
    if spec.eq_ignore_ascii_case("localhost") || spec.eq_ignore_ascii_case("local") {
        return json!({"type": "local", "tag": tag});
    }
    let mut server = json!({"type": kind, "tag": tag, "server": host});
    if let Some(port) = port {
        server["server_port"] = json!(port);
    }
    if matches!(kind, "https" | "h3") {
        if let Some(path) = after_host.filter(|p| p != "/dns-query") {
            server["path"] = json!(path);
        }
    }
    if !is_ip(&host) {
        server["domain_resolver"] = json!("local");
    }
    server
}

fn dns(profile: &Profile, settings: &Settings, fake_dns: bool) -> Value {
    let mut remote = dns_server(&remote_dns(profile, settings), "remote");
    remote["detour"] = json!("proxy");
    let direct = if settings.dns.direct.trim().is_empty() { "local" } else { settings.dns.direct.as_str() };
    let mut local = dns_server(direct, "local");
    if local.get("domain_resolver").is_some() {
        // The direct resolver cannot depend on itself.
        local = json!({"type": "local", "tag": "local"});
    }
    let mut servers = vec![remote, local];
    let mut rules = Vec::new();
    if !is_ip(&profile.address) {
        rules.push(json!({"domain": [profile.address], "server": "local"}));
    }
    if settings.routing.preset == RoutingPreset::BypassLanRu {
        rules.push(json!({"domain_suffix": RU_DOMAIN_SUFFIXES, "server": "local"}));
    }
    if fake_dns {
        servers.push(json!({"type": "fakeip", "tag": "fakeip", "inet4_range": "198.18.0.0/15", "inet6_range": "fc00::/18"}));
        rules.push(json!({"query_type": ["A", "AAAA"], "server": "fakeip"}));
    }
    let strategy = match settings.dns.query_strategy.as_str() {
        "UseIPv4" => "ipv4_only",
        "UseIPv6" => "ipv6_only",
        _ => "prefer_ipv4",
    };
    json!({"servers": servers, "rules": rules, "final": "remote", "strategy": strategy})
}

fn route(profile: &Profile, settings: &Settings, env: &ConfigEnv, mode: Mode) -> Value {
    let tun = mode != Mode::Proxy;
    let mut rules = vec![json!({"action": "sniff"}), json!({"protocol": "dns", "action": "hijack-dns"})];
    if settings.dns.local_dns {
        rules.push(json!({"inbound": ["dns-in"], "action": "hijack-dns"}));
    }
    if tun {
        // Core processes reach the proxy server directly; otherwise their traffic loops into the tunnel.
        rules.push(json!({"process_name": env.core_process_names, "outbound": "direct"}));
    }
    if is_ip(&profile.address) {
        rules.push(json!({"ip_cidr": [profile.address], "outbound": "direct"}));
    } else {
        rules.push(json!({"domain": [profile.address], "outbound": "direct"}));
    }
    if settings.routing.preset != RoutingPreset::Global {
        rules.push(json!({"ip_is_private": true, "outbound": "direct"}));
    }

    let apps: Vec<&String> = settings.tun.apps.iter().filter(|a| !a.trim().is_empty()).collect();
    let mut uses_process = tun;
    if tun && !apps.is_empty() {
        match settings.tun.app_mode {
            TunAppMode::Include => rules.push(json!({"process_name": apps, "invert": true, "outbound": "direct"})),
            TunAppMode::Exclude => rules.push(json!({"process_name": apps, "outbound": "direct"})),
            TunAppMode::All => {}
        }
    }

    for rule in settings.routing.rules.iter().filter(|r| r.enabled) {
        if let Some(converted) = convert_rule(rule) {
            uses_process |= rule.kind == RuleKind::Process;
            rules.push(converted);
        }
    }
    if settings.routing.preset == RoutingPreset::BypassLanRu {
        rules.push(json!({"domain_suffix": RU_DOMAIN_SUFFIXES, "outbound": "direct"}));
    }
    let final_outbound = if settings.routing.preset == RoutingPreset::DirectByDefault { "direct" } else { "proxy" };
    let mut route = json!({
        "rules": rules,
        "final": final_outbound,
        "auto_detect_interface": true,
        "default_domain_resolver": "local"
    });
    if uses_process {
        route["find_process"] = json!(true);
    }
    route
}

/// Converts an Xray-syntax rule to sing-box. Values that need geo databases are skipped.
fn convert_rule(rule: &RoutingRule) -> Option<Value> {
    let mut entry = serde_json::Map::new();
    let push = |entry: &mut serde_json::Map<String, Value>, key: &str, value: Value| {
        entry.entry(key).or_insert_with(|| json!([])).as_array_mut().unwrap().push(value);
    };
    for value in rule.values.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        match rule.kind {
            RuleKind::Domain => {
                if let Some(v) = value.strip_prefix("full:") {
                    push(&mut entry, "domain", json!(v));
                } else if let Some(v) = value.strip_prefix("domain:") {
                    push(&mut entry, "domain_suffix", json!(v));
                } else if let Some(v) = value.strip_prefix("keyword:") {
                    push(&mut entry, "domain_keyword", json!(v));
                } else if let Some(v) = value.strip_prefix("regexp:") {
                    push(&mut entry, "domain_regex", json!(v));
                } else if value.starts_with("geosite:") || value.starts_with("ext:") {
                    continue;
                } else {
                    push(&mut entry, "domain_keyword", json!(value));
                }
            }
            RuleKind::Ip => {
                if value == "geoip:private" {
                    entry.insert("ip_is_private".into(), json!(true));
                } else if !value.starts_with("geoip:") {
                    push(&mut entry, "ip_cidr", json!(value));
                }
            }
            RuleKind::Port => {
                for part in value.split(',').map(str::trim) {
                    if let Ok(port) = part.parse::<u16>() {
                        push(&mut entry, "port", json!(port));
                    } else if part.contains('-') {
                        push(&mut entry, "port_range", json!(part.replace('-', ":")));
                    }
                }
            }
            RuleKind::Process => push(&mut entry, "process_name", json!(value)),
            RuleKind::Protocol => push(&mut entry, "protocol", json!(value)),
            RuleKind::Network => {
                for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                    push(&mut entry, "network", json!(part));
                }
            }
        }
    }
    if entry.is_empty() {
        return None;
    }
    match rule.outbound {
        RuleOutbound::Block => entry.insert("action".into(), json!("reject")),
        other => entry.insert("outbound".into(), json!(other.tag())),
    };
    Some(Value::Object(entry))
}

/// Native sing-box outbound (without tag). Used for Hysteria2/TUIC and batch tests.
pub fn outbound(profile: &Profile, settings: &Settings) -> Value {
    let tls = &profile.tls;
    let mut tls_value = json!({
        "enabled": true,
        "server_name": if tls.sni.is_empty() { profile.address.clone() } else { tls.sni.clone() },
        "insecure": tls.allow_insecure || settings.advanced.allow_insecure,
        "alpn": if tls.alpn.is_empty() { vec!["h3".to_string()] } else { tls.alpn.clone() }
    });
    match profile.protocol {
        Protocol::Hysteria2 => {
            let q = &profile.quic;
            let mut out = json!({
                "type": "hysteria2", "server": profile.address, "server_port": profile.port,
                "password": profile.password, "tls": tls_value
            });
            if q.up_mbps > 0 {
                out["up_mbps"] = json!(q.up_mbps);
            }
            if q.down_mbps > 0 {
                out["down_mbps"] = json!(q.down_mbps);
            }
            if !q.obfs.is_empty() {
                out["obfs"] = json!({"type": q.obfs, "password": q.obfs_password});
            }
            let hops: Vec<String> = q
                .hop_ports
                .split(',')
                .map(str::trim)
                .filter(|p| p.contains('-'))
                .map(|p| p.replace('-', ":"))
                .collect();
            if !hops.is_empty() {
                out["server_ports"] = json!(hops);
            }
            out
        }
        Protocol::Tuic => {
            let q = &profile.quic;
            json!({
                "type": "tuic", "server": profile.address, "server_port": profile.port,
                "uuid": profile.uuid, "password": profile.password,
                "congestion_control": if q.congestion_control.is_empty() { "bbr" } else { q.congestion_control.as_str() },
                "udp_relay_mode": if q.udp_relay_mode.is_empty() { "native" } else { q.udp_relay_mode.as_str() },
                "tls": tls_value
            })
        }
        Protocol::Shadowsocks => json!({
            "type": "shadowsocks", "server": profile.address, "server_port": profile.port,
            "method": profile.method, "password": profile.password
        }),
        Protocol::Trojan | Protocol::Vless | Protocol::Vmess => {
            // Only used as a fallback; Xray normally handles these protocols.
            tls_value["alpn"] = json!(tls.alpn);
            let mut out = json!({"type": profile.protocol.as_str(), "server": profile.address, "server_port": profile.port});
            match profile.protocol {
                Protocol::Trojan => out["password"] = json!(profile.password),
                Protocol::Vless => {
                    out["uuid"] = json!(profile.uuid);
                    if !profile.flow.is_empty() {
                        out["flow"] = json!(profile.flow);
                    }
                }
                _ => {
                    out["uuid"] = json!(profile.uuid);
                    out["security"] = json!(if profile.method.is_empty() { "auto" } else { profile.method.as_str() });
                    out["alter_id"] = json!(profile.alter_id);
                }
            }
            if tls.security != crate::model::Security::None {
                if tls.security == crate::model::Security::Reality {
                    tls_value["reality"] = json!({"enabled": true, "public_key": tls.public_key, "short_id": tls.short_id});
                }
                tls_value["utls"] = json!({"enabled": true, "fingerprint": if tls.fingerprint.is_empty() { "chrome" } else { tls.fingerprint.as_str() }});
                out["tls"] = tls_value;
            }
            out
        }
    }
}
