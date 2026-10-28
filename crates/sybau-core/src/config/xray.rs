//! Xray client configuration.

use serde_json::{json, Map, Value};

use super::{dns_host, is_ip, remote_dns, ConfigEnv, PRIVATE_CIDRS, RU_DOMAIN_SUFFIXES};
use crate::model::{Network, Profile, Protocol, RuleKind, Security};
use crate::settings::{RoutingPreset, Settings};
use crate::util::merge_json;

pub const PROXY_TAG: &str = "proxy";
const FAKEDNS_POOL: &str = "198.18.0.0/15";

/// Builds the config used to connect. `proxy_mode` is false when sing-box TUN sits in front of Xray.
pub fn build_client(profile: &Profile, settings: &Settings, env: &ConfigEnv, proxy_mode: bool) -> Value {
    let listen = if settings.proxy.allow_lan { "0.0.0.0" } else { "127.0.0.1" };
    let fake_dns = settings.dns.fake_dns && proxy_mode;
    let sniffing = sniffing(settings, fake_dns);

    let mut inbounds = vec![
        json!({
            "tag": "socks", "protocol": "socks", "listen": listen, "port": settings.proxy.socks_port,
            "settings": {"auth": "noauth", "udp": true}, "sniffing": sniffing
        }),
        json!({
            "tag": "http", "protocol": "http", "listen": listen, "port": settings.proxy.http_port,
            "settings": {}, "sniffing": sniffing
        }),
    ];
    let remote = remote_dns(profile, settings);
    if settings.dns.local_dns {
        let target = dns_host(&remote);
        inbounds.push(json!({
            "tag": "dns-in", "protocol": "dokodemo-door", "listen": "127.0.0.1",
            "port": settings.dns.local_port,
            "settings": {"address": if is_ip(&target) { target } else { "1.1.1.1".to_string() }, "port": 53, "network": "tcp,udp"}
        }));
    }

    let mut proxy = outbound(profile, settings);
    proxy["tag"] = json!(PROXY_TAG);
    let mut outbounds = vec![proxy];
    // `sockopt.domainStrategy` replaces the deprecated `freedom.domainStrategy` (Xray 26.10+).
    outbounds.push(json!({
        "tag": "direct", "protocol": "freedom", "settings": {},
        "streamSettings": {"sockopt": {"domainStrategy": "UseIP"}}
    }));
    outbounds.push(json!({"tag": "block", "protocol": "blackhole", "settings": {"response": {"type": "http"}}}));
    outbounds.push(json!({"tag": "dns-out", "protocol": "dns"}));
    if settings.fragment.enabled {
        outbounds.push(json!({
            "tag": "fragment", "protocol": "freedom",
            "settings": {"fragment": {
                "packets": settings.fragment.packets,
                "length": settings.fragment.length,
                "interval": settings.fragment.interval
            }},
            "streamSettings": {"sockopt": {"tcpNoDelay": true, "domainStrategy": "UseIP"}}
        }));
    }

    let mut config = json!({
        "log": log(settings),
        "dns": dns(profile, settings, env, fake_dns),
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": routing(settings, env),
        "api": {"tag": "api", "listen": format!("127.0.0.1:{}", settings.advanced.api_port), "services": ["StatsService"]},
        "stats": {},
        "policy": {"system": {"statsOutboundUplink": true, "statsOutboundDownlink": true}}
    });
    if fake_dns {
        config["fakedns"] = json!([{"ipPool": FAKEDNS_POOL, "poolSize": 65535}]);
    }
    apply_override(&mut config, &settings.advanced.config_override);
    config
}

/// One SOCKS inbound per profile on consecutive ports, each routed to its own outbound.
/// Used for batch "real delay" tests, like v2rayNG/v2rayN do.
pub fn build_batch(entries: &[(&Profile, u16)], settings: &Settings) -> Value {
    let mut inbounds = Vec::new();
    let mut outbounds = Vec::new();
    let mut rules = Vec::new();
    for (index, (profile, port)) in entries.iter().enumerate() {
        inbounds.push(json!({
            "tag": format!("in-{index}"), "protocol": "socks", "listen": "127.0.0.1", "port": port,
            "settings": {"auth": "noauth", "udp": false}
        }));
        let mut out = outbound(profile, settings);
        out["tag"] = json!(format!("out-{index}"));
        outbounds.push(out);
        rules.push(json!({"type": "field", "inboundTag": [format!("in-{index}")], "outboundTag": format!("out-{index}")}));
    }
    if settings.fragment.enabled {
        outbounds.push(json!({
            "tag": "fragment", "protocol": "freedom",
            "settings": {"fragment": {"packets": settings.fragment.packets, "length": settings.fragment.length, "interval": settings.fragment.interval}}
        }));
    }
    json!({
        "log": {"loglevel": "none"},
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": {"domainStrategy": "AsIs", "rules": rules}
    })
}

fn log(settings: &Settings) -> Value {
    // Empty paths make Xray print both logs to stdout, which the core manager captures and parses.
    json!({
        "loglevel": settings.advanced.log_level,
        "access": if settings.advanced.access_log { "" } else { "none" },
        "error": "",
        "dnsLog": false
    })
}

fn sniffing(settings: &Settings, fake_dns: bool) -> Value {
    let mut dest = vec!["http", "tls", "quic"];
    if fake_dns {
        dest.push("fakedns");
    }
    json!({
        "enabled": settings.routing.sniffing || fake_dns,
        "destOverride": dest,
        "routeOnly": settings.routing.route_only
    })
}

fn dns(profile: &Profile, settings: &Settings, env: &ConfigEnv, fake_dns: bool) -> Value {
    let remote = remote_dns(profile, settings);
    let direct = settings.dns.direct.trim();
    let mut direct_domains = Vec::new();
    if !is_ip(&profile.address) {
        // The proxy server itself must be resolved without the proxy.
        direct_domains.push(format!("full:{}", profile.address));
    }
    if settings.routing.preset == RoutingPreset::BypassLanRu {
        direct_domains.extend(RU_DOMAIN_SUFFIXES.iter().map(|s| format!("domain:{s}")));
        if env.geo_available {
            direct_domains.push("geosite:category-ru".into());
        }
    }

    let mut servers = Vec::new();
    if fake_dns {
        servers.push(json!("fakedns"));
    }
    servers.push(json!(remote));
    if !direct.is_empty() {
        servers.push(json!({"address": direct, "domains": direct_domains, "skipFallback": true}));
    }

    let mut hosts = Map::new();
    for line in settings.dns.hosts.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(domain), Some(ip)) = (parts.next(), parts.next()) {
            hosts.insert(domain.to_string(), json!(ip));
        }
    }
    json!({
        "hosts": hosts,
        "servers": servers,
        "queryStrategy": settings.dns.query_strategy,
        "tag": "dns-module"
    })
}

fn routing(settings: &Settings, env: &ConfigEnv) -> Value {
    let mut rules = Vec::new();
    if settings.dns.local_dns {
        rules.push(json!({"type": "field", "inboundTag": ["dns-in"], "outboundTag": "dns-out"}));
    }
    // Plain-DNS direct server must not depend on the proxy.
    let direct_dns = dns_host(&settings.dns.direct);
    if is_ip(&direct_dns) {
        rules.push(json!({"type": "field", "ip": [direct_dns], "port": "53", "outboundTag": "direct"}));
    }

    for rule in settings.routing.rules.iter().filter(|r| r.enabled && !r.values.is_empty()) {
        let values: Vec<String> = rule
            .values
            .iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .filter(|v| env.geo_available || !(v.starts_with("geosite:") || v.starts_with("geoip:")))
            .collect();
        if values.is_empty() {
            continue;
        }
        let mut entry = json!({"type": "field", "outboundTag": rule.outbound.tag()});
        match rule.kind {
            RuleKind::Domain => entry["domain"] = json!(values),
            RuleKind::Ip => entry["ip"] = json!(values),
            RuleKind::Port => entry["port"] = json!(values.join(",")),
            RuleKind::Protocol => entry["protocol"] = json!(values),
            RuleKind::Network => entry["network"] = json!(values.join(",")),
            // Process rules are applied by sing-box in TUN mode.
            RuleKind::Process => continue,
        }
        rules.push(entry);
    }

    if settings.routing.block_ads && env.geo_available {
        rules.push(json!({"type": "field", "domain": ["geosite:category-ads-all"], "outboundTag": "block"}));
    }
    let preset = settings.routing.preset;
    if preset != RoutingPreset::Global {
        rules.push(json!({"type": "field", "ip": PRIVATE_CIDRS, "outboundTag": "direct"}));
        if env.geo_available {
            rules.push(json!({"type": "field", "domain": ["geosite:private"], "outboundTag": "direct"}));
        }
    }
    if preset == RoutingPreset::BypassLanRu {
        let mut domains: Vec<String> = RU_DOMAIN_SUFFIXES.iter().map(|s| format!("domain:{s}")).collect();
        if env.geo_available {
            domains.push("geosite:category-ru".into());
        }
        rules.push(json!({"type": "field", "domain": domains, "outboundTag": "direct"}));
        if env.geo_available {
            rules.push(json!({"type": "field", "ip": ["geoip:ru"], "outboundTag": "direct"}));
        }
    }
    if preset == RoutingPreset::DirectByDefault {
        rules.push(json!({"type": "field", "network": "tcp,udp", "outboundTag": "direct"}));
    }
    json!({"domainStrategy": settings.routing.domain_strategy, "rules": rules})
}

/// Xray outbound for a single profile (without tag).
pub fn outbound(profile: &Profile, settings: &Settings) -> Value {
    let settings_value = match profile.protocol {
        Protocol::Vless => json!({"vnext": [{
            "address": profile.address, "port": profile.port,
            "users": [{"id": profile.uuid, "encryption": non_empty(&profile.method, "none"), "flow": profile.flow}]
        }]}),
        Protocol::Vmess => json!({"vnext": [{
            "address": profile.address, "port": profile.port,
            "users": [{"id": profile.uuid, "alterId": profile.alter_id, "security": non_empty(&profile.method, "auto")}]
        }]}),
        Protocol::Trojan => json!({"servers": [{
            "address": profile.address, "port": profile.port, "password": profile.password
        }]}),
        Protocol::Shadowsocks => json!({"servers": [{
            "address": profile.address, "port": profile.port, "method": profile.method, "password": profile.password
        }]}),
        // Not supported by Xray; callers route these profiles to sing-box.
        Protocol::Hysteria2 | Protocol::Tuic => json!({}),
    };
    let mut out = json!({
        "protocol": profile.protocol.as_str(),
        "settings": settings_value,
        "streamSettings": stream_settings(profile, settings)
    });
    let mux_allowed = profile.flow.is_empty() && profile.transport.network != Network::Xhttp;
    if settings.mux.enabled && mux_allowed {
        out["mux"] = json!({
            "enabled": true,
            "concurrency": settings.mux.concurrency,
            "xudpConcurrency": settings.mux.xudp_concurrency,
            "xudpProxyUDP443": settings.mux.xudp_proxy_udp443
        });
    }
    out
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn stream_settings(profile: &Profile, settings: &Settings) -> Value {
    let t = &profile.transport;
    let network = match t.network {
        Network::Tcp => "raw",
        Network::H2 => "xhttp",
        Network::Quic => "raw",
        other => other.as_str(),
    };
    let mut stream = json!({"network": network});
    let host_or_sni = || if t.host.is_empty() { profile.tls.sni.clone() } else { t.host.clone() };
    match t.network {
        Network::Tcp | Network::Quic if t.header_type == "http" => {
            stream["rawSettings"] = json!({"header": {"type": "http", "request": {
                "path": [non_empty(&t.path, "/")],
                "headers": {"Host": t.host.split(',').map(str::trim).filter(|h| !h.is_empty()).collect::<Vec<_>>()}
            }}});
        }
        Network::Ws => stream["wsSettings"] = json!({"path": non_empty(&t.path, "/"), "host": host_or_sni()}),
        Network::HttpUpgrade => stream["httpupgradeSettings"] = json!({"path": non_empty(&t.path, "/"), "host": host_or_sni()}),
        Network::Grpc => {
            stream["grpcSettings"] = json!({
                "serviceName": t.service_name, "multiMode": t.mode == "multi", "authority": t.host
            })
        }
        Network::Xhttp | Network::H2 => {
            let mut xhttp = json!({"path": non_empty(&t.path, "/"), "host": t.host, "mode": non_empty(&t.mode, "auto")});
            if let Ok(extra @ Value::Object(_)) = serde_json::from_str::<Value>(&t.extra) {
                xhttp["extra"] = extra;
            }
            stream["xhttpSettings"] = xhttp;
        }
        Network::Kcp => {
            stream["kcpSettings"] = json!({"header": {"type": non_empty(&t.header_type, "none")}, "seed": non_empty(&t.seed, &t.path)})
        }
        _ => {}
    }

    let tls = &profile.tls;
    let server_name = if !tls.sni.is_empty() {
        tls.sni.clone()
    } else if !t.host.is_empty() && !is_ip(&t.host) {
        t.host.split(',').next().unwrap_or("").trim().to_string()
    } else {
        profile.address.clone()
    };
    match tls.security {
        Security::Tls => {
            stream["security"] = json!("tls");
            let mut tls_settings = json!({
                "serverName": server_name,
                "allowInsecure": tls.allow_insecure || settings.advanced.allow_insecure,
                "fingerprint": non_empty(&tls.fingerprint, "chrome")
            });
            if !tls.alpn.is_empty() {
                tls_settings["alpn"] = json!(tls.alpn);
            }
            stream["tlsSettings"] = tls_settings;
        }
        Security::Reality => {
            stream["security"] = json!("reality");
            stream["realitySettings"] = json!({
                "serverName": server_name,
                "fingerprint": non_empty(&tls.fingerprint, "chrome"),
                "publicKey": tls.public_key,
                "shortId": tls.short_id,
                "spiderX": non_empty(&tls.spider_x, "/")
            });
        }
        Security::None => {}
    }
    if settings.fragment.enabled && tls.security != Security::None {
        stream["sockopt"] = json!({"dialerProxy": "fragment"});
    }
    stream
}

fn apply_override(config: &mut Value, override_json: &str) {
    if override_json.trim().is_empty() {
        return;
    }
    match serde_json::from_str::<Value>(override_json) {
        Ok(patch @ Value::Object(_)) => merge_json(config, &patch),
        Ok(_) => tracing::warn!("config override is not a JSON object, ignored"),
        Err(e) => tracing::warn!("invalid config override: {e}"),
    }
}
