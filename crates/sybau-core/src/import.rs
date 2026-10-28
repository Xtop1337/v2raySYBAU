//! Detects what the user pasted / dropped (share links, base64 blob, subscription URL,
//! Xray or sing-box JSON, our own export or backup) and turns it into profiles.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Network, Profile, Protocol, RoutingRule, Security, Subscription};
use crate::settings::Settings;
use crate::share::{is_share_link, parse_link};
use crate::util::b64_decode_str;

/// Full backup written by "Export everything" and accepted by import.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Backup {
    pub app: String,
    pub version: u32,
    pub profiles: Vec<Profile>,
    pub subscriptions: Vec<Subscription>,
    pub rules: Option<Vec<RoutingRule>>,
    pub settings: Option<Settings>,
}

pub const BACKUP_APP: &str = "v2raysybau";

#[derive(Debug, Default)]
pub struct ImportResult {
    pub profiles: Vec<Profile>,
    pub subscription_urls: Vec<String>,
    pub backup: Option<Backup>,
    pub errors: Vec<String>,
}

pub fn import_text(text: &str) -> ImportResult {
    let mut result = ImportResult::default();
    import_into(text.trim().trim_start_matches('\u{feff}'), &mut result, 0);
    result
}

fn import_into(text: &str, result: &mut ImportResult, depth: u8) {
    if text.is_empty() {
        return;
    }
    if text.starts_with('{') || text.starts_with('[') {
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            import_json(&value, result);
            return;
        }
    }

    let mut matched = false;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if is_share_link(line) {
            matched = true;
            match parse_link(line) {
                Ok(profile) => result.profiles.push(profile),
                Err(e) => result.errors.push(format!("{e}: {}", truncate(line))),
            }
        } else if line.starts_with("http://") || line.starts_with("https://") {
            matched = true;
            result.subscription_urls.push(line.to_string());
        } else if line.starts_with("sybau://") || line.starts_with("happ://add/") {
            // Deep links that wrap a subscription URL.
            matched = true;
            let url = line.trim_start_matches("sybau://import/").trim_start_matches("happ://add/");
            result.subscription_urls.push(url.to_string());
        }
    }
    if !matched && depth == 0 {
        if let Some(decoded) = b64_decode_str(text) {
            import_into(decoded.trim(), result, depth + 1);
        }
    }
}

fn truncate(line: &str) -> String {
    line.chars().take(60).collect()
}

fn import_json(value: &Value, result: &mut ImportResult) {
    if let Value::Array(items) = value {
        for item in items {
            if item.get("protocol").is_some() || item.get("address").is_some() {
                if let Ok(profile) = serde_json::from_value::<Profile>(item.clone()) {
                    result.profiles.push(profile);
                    continue;
                }
            }
            import_json(item, result);
        }
        return;
    }
    if value.get("app").and_then(Value::as_str) == Some(BACKUP_APP) {
        match serde_json::from_value::<Backup>(value.clone()) {
            Ok(backup) => result.backup = Some(backup),
            Err(e) => result.errors.push(format!("backup: {e}")),
        }
        return;
    }
    let Some(outbounds) = value.get("outbounds").and_then(Value::as_array) else {
        result.errors.push("JSON without outbounds".into());
        return;
    };
    let config_name = value.get("remarks").and_then(Value::as_str).unwrap_or_default();
    let dns = first_dns_server(value);
    for outbound in outbounds {
        let profile = if outbound.get("type").is_some() {
            singbox_outbound(outbound)
        } else {
            xray_outbound(outbound)
        };
        if let Some(mut profile) = profile {
            if profile.name.is_empty() {
                profile.name = config_name.to_string();
            }
            if profile.dns.is_empty() {
                profile.dns = dns.clone();
            }
            result.profiles.push(profile);
        }
    }
}

fn first_dns_server(config: &Value) -> String {
    let servers = config.pointer("/dns/servers").and_then(Value::as_array);
    servers
        .and_then(|servers| servers.first())
        .and_then(|server| match server {
            Value::String(s) => Some(s.clone()),
            other => other
                .get("address")
                .or_else(|| other.get("server"))
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .unwrap_or_default()
}

fn s(value: &Value, pointer: &str) -> String {
    match value.pointer(pointer) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn xray_outbound(outbound: &Value) -> Option<Profile> {
    let protocol = match outbound.get("protocol")?.as_str()? {
        "vless" => Protocol::Vless,
        "vmess" => Protocol::Vmess,
        "trojan" => Protocol::Trojan,
        "shadowsocks" => Protocol::Shadowsocks,
        _ => return None,
    };
    let mut p = Profile {
        protocol,
        name: s(outbound, "/tag"),
        ..Profile::default()
    };
    if p.name == "proxy" {
        p.name.clear();
    }
    let settings = outbound.get("settings")?;
    // Both the classic `vnext`/`servers` layout and the flat Xray 25+ layout are accepted.
    let server = settings
        .pointer("/vnext/0")
        .or_else(|| settings.pointer("/servers/0"))
        .unwrap_or(settings);
    p.address = s(server, "/address");
    p.port = s(server, "/port").parse().ok()?;
    let user = server.pointer("/users/0").unwrap_or(server);
    match protocol {
        Protocol::Vless | Protocol::Vmess => {
            p.uuid = s(user, "/id");
            p.flow = s(user, "/flow");
            p.alter_id = s(user, "/alterId").parse().unwrap_or(0);
            p.method = s(user, if protocol == Protocol::Vless { "/encryption" } else { "/security" });
        }
        _ => {
            p.password = s(user, "/password");
            p.method = s(user, "/method");
        }
    }
    if p.address.is_empty() {
        return None;
    }
    let stream = outbound.get("streamSettings").cloned().unwrap_or(Value::Null);
    p.transport.network = Network::parse(&s(&stream, "/network"));
    p.tls.security = Security::parse(&s(&stream, "/security"));
    let tls = stream
        .get("realitySettings")
        .or_else(|| stream.get("tlsSettings"))
        .cloned()
        .unwrap_or(Value::Null);
    p.tls.sni = s(&tls, "/serverName");
    p.tls.fingerprint = s(&tls, "/fingerprint");
    p.tls.public_key = s(&tls, "/publicKey");
    p.tls.short_id = s(&tls, "/shortId");
    p.tls.spider_x = s(&tls, "/spiderX");
    p.tls.allow_insecure = tls.get("allowInsecure").and_then(Value::as_bool).unwrap_or(false);
    p.tls.alpn = tls
        .get("alpn")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let t = &mut p.transport;
    match t.network {
        Network::Ws => {
            t.path = s(&stream, "/wsSettings/path");
            t.host = s(&stream, "/wsSettings/host");
            if t.host.is_empty() {
                t.host = s(&stream, "/wsSettings/headers/Host");
            }
        }
        Network::Grpc => {
            t.service_name = s(&stream, "/grpcSettings/serviceName");
            t.mode = if stream.pointer("/grpcSettings/multiMode").and_then(Value::as_bool) == Some(true) {
                "multi".into()
            } else {
                String::new()
            };
        }
        Network::Xhttp => {
            t.path = s(&stream, "/xhttpSettings/path");
            t.host = s(&stream, "/xhttpSettings/host");
            t.mode = s(&stream, "/xhttpSettings/mode");
            if let Some(extra) = stream.pointer("/xhttpSettings/extra") {
                t.extra = extra.to_string();
            }
        }
        Network::HttpUpgrade => {
            t.path = s(&stream, "/httpupgradeSettings/path");
            t.host = s(&stream, "/httpupgradeSettings/host");
        }
        Network::Tcp => t.header_type = s(&stream, "/tcpSettings/header/type"),
        _ => {}
    }
    Some(p)
}

fn singbox_outbound(outbound: &Value) -> Option<Profile> {
    let protocol = match outbound.get("type")?.as_str()? {
        "vless" => Protocol::Vless,
        "vmess" => Protocol::Vmess,
        "trojan" => Protocol::Trojan,
        "shadowsocks" => Protocol::Shadowsocks,
        "hysteria2" => Protocol::Hysteria2,
        "tuic" => Protocol::Tuic,
        _ => return None,
    };
    let mut p = Profile {
        protocol,
        name: s(outbound, "/tag"),
        address: s(outbound, "/server"),
        port: s(outbound, "/server_port").parse().ok()?,
        uuid: s(outbound, "/uuid"),
        password: s(outbound, "/password"),
        method: s(outbound, "/method"),
        flow: s(outbound, "/flow"),
        ..Profile::default()
    };
    if protocol == Protocol::Vmess {
        p.method = s(outbound, "/security");
        p.alter_id = s(outbound, "/alter_id").parse().unwrap_or(0);
    }
    if let Some(tls) = outbound.get("tls").filter(|t| t.get("enabled").and_then(Value::as_bool) == Some(true)) {
        let reality = tls.pointer("/reality/enabled").and_then(Value::as_bool) == Some(true);
        p.tls.security = if reality { Security::Reality } else { Security::Tls };
        p.tls.sni = s(tls, "/server_name");
        p.tls.allow_insecure = tls.get("insecure").and_then(Value::as_bool).unwrap_or(false);
        p.tls.fingerprint = s(tls, "/utls/fingerprint");
        p.tls.public_key = s(tls, "/reality/public_key");
        p.tls.short_id = s(tls, "/reality/short_id");
        p.tls.alpn = tls
            .get("alpn")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
    }
    if let Some(transport) = outbound.get("transport") {
        p.transport.network = Network::parse(&s(transport, "/type"));
        p.transport.path = s(transport, "/path");
        p.transport.service_name = s(transport, "/service_name");
        p.transport.host = s(transport, "/headers/Host");
        if p.transport.host.is_empty() {
            p.transport.host = s(transport, "/host");
        }
    }
    p.quic.obfs = s(outbound, "/obfs/type");
    p.quic.obfs_password = s(outbound, "/obfs/password");
    p.quic.congestion_control = s(outbound, "/congestion_control");
    p.quic.udp_relay_mode = s(outbound, "/udp_relay_mode");
    p.quic.up_mbps = s(outbound, "/up_mbps").parse().unwrap_or(0);
    p.quic.down_mbps = s(outbound, "/down_mbps").parse().unwrap_or(0);
    (!p.address.is_empty()).then_some(p)
}

/// Removes profiles that duplicate an earlier one (same endpoint and credentials).
pub fn dedupe(profiles: &mut Vec<Profile>) -> usize {
    let before = profiles.len();
    let mut seen = std::collections::HashSet::new();
    profiles.retain(|p| seen.insert(p.identity_key()));
    before - profiles.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::b64_encode;

    #[test]
    fn imports_mixed_text_and_base64_blob() {
        let text = "vless://id@a.com:443?security=tls#A\n\nhttps://sub.example.com/abc\ntrojan://pw@b.com:443#B\nrandom";
        let result = import_text(text);
        assert_eq!(result.profiles.len(), 2);
        assert_eq!(result.subscription_urls, vec!["https://sub.example.com/abc"]);

        let blob = b64_encode(b"vless://id@a.com:443#A\r\nss://YWVzLTI1Ni1nY206cGFzcw@s.com:8388#S\r\n");
        let result = import_text(&blob);
        assert_eq!(result.profiles.len(), 2);
        assert_eq!(result.profiles[1].protocol, Protocol::Shadowsocks);
    }

    #[test]
    fn imports_xray_json_config_with_dns() {
        let config = r#"{
          "remarks": "Germany",
          "dns": {"servers": ["https://9.9.9.9/dns-query", "8.8.8.8"]},
          "outbounds": [
            {"tag": "proxy", "protocol": "vless",
             "settings": {"vnext": [{"address": "de.example.com", "port": 443,
                "users": [{"id": "uuid-1", "encryption": "none", "flow": "xtls-rprx-vision"}]}]},
             "streamSettings": {"network": "tcp", "security": "reality",
                "realitySettings": {"serverName": "www.google.com", "publicKey": "PBK", "shortId": "ab", "fingerprint": "chrome"}}},
            {"tag": "direct", "protocol": "freedom"}
          ]}"#;
        let result = import_text(config);
        assert_eq!(result.profiles.len(), 1);
        let p = &result.profiles[0];
        assert_eq!(p.name, "Germany");
        assert_eq!(p.address, "de.example.com");
        assert_eq!(p.flow, "xtls-rprx-vision");
        assert_eq!(p.tls.security, Security::Reality);
        assert_eq!(p.tls.public_key, "PBK");
        assert_eq!(p.dns, "https://9.9.9.9/dns-query");
    }

    #[test]
    fn imports_singbox_json() {
        let config = r#"{"outbounds": [
          {"type": "hysteria2", "tag": "HY", "server": "hy.com", "server_port": 443, "password": "pw",
           "tls": {"enabled": true, "server_name": "hy.com"}},
          {"type": "selector", "tag": "select", "outbounds": ["HY"]}
        ]}"#;
        let result = import_text(config);
        assert_eq!(result.profiles.len(), 1);
        assert_eq!(result.profiles[0].protocol, Protocol::Hysteria2);
        assert_eq!(result.profiles[0].tls.sni, "hy.com");
    }

    #[test]
    fn imports_own_export_and_backup() {
        let profile = parse_link("vless://id@a.com:443#A").unwrap();
        let exported = serde_json::to_string(&vec![profile.clone()]).unwrap();
        assert_eq!(import_text(&exported).profiles, vec![profile.clone()]);

        let backup = Backup { app: BACKUP_APP.into(), version: 1, profiles: vec![profile], ..Default::default() };
        let result = import_text(&serde_json::to_string(&backup).unwrap());
        assert_eq!(result.backup.unwrap().profiles.len(), 1);
    }

    #[test]
    fn dedupes_profiles() {
        let mut list = import_text("vless://id@a.com:443#A\nvless://id@a.com:443#A copy\nvless://id@b.com:443#B").profiles;
        assert_eq!(dedupe(&mut list), 1);
        assert_eq!(list.len(), 2);
    }
}
