//! Builds share links back from profiles ("copy link", QR, subscription export).

use serde_json::json;

use crate::model::{Network, Profile, Protocol, Security};
use crate::util::{b64_encode, b64_encode_url, percent_encode};

pub fn build_link(profile: &Profile) -> String {
    match profile.protocol {
        Protocol::Vless => standard_link("vless", &profile.uuid, profile, true),
        Protocol::Trojan => standard_link("trojan", &profile.password, profile, false),
        Protocol::Vmess => vmess_link(profile),
        Protocol::Shadowsocks => shadowsocks_link(profile),
        Protocol::Hysteria2 => hysteria2_link(profile),
        Protocol::Tuic => tuic_link(profile),
    }
}

fn host_port(profile: &Profile) -> String {
    if profile.address.contains(':') {
        format!("[{}]:{}", profile.address, profile.port)
    } else {
        format!("{}:{}", profile.address, profile.port)
    }
}

fn fragment(profile: &Profile) -> String {
    if profile.name.is_empty() {
        String::new()
    } else {
        format!("#{}", percent_encode(&profile.name))
    }
}

fn query_string(pairs: &[(&str, String)]) -> String {
    let joined = pairs
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{key}={}", percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    if joined.is_empty() {
        String::new()
    } else {
        format!("?{joined}")
    }
}

fn standard_link(scheme: &str, user: &str, profile: &Profile, include_encryption: bool) -> String {
    let t = &profile.transport;
    let tls = &profile.tls;
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if include_encryption {
        pairs.push(("encryption", if profile.method.is_empty() { "none".into() } else { profile.method.clone() }));
    }
    pairs.extend([
        ("type", t.network.as_str().to_string()),
        ("security", tls.security.as_str().to_string()),
        ("flow", profile.flow.clone()),
        ("sni", tls.sni.clone()),
        ("alpn", tls.alpn.join(",")),
        ("fp", tls.fingerprint.clone()),
        ("pbk", tls.public_key.clone()),
        ("sid", tls.short_id.clone()),
        ("spx", tls.spider_x.clone()),
        ("headerType", t.header_type.clone()),
        ("host", t.host.clone()),
        ("path", t.path.clone()),
        ("serviceName", t.service_name.clone()),
        ("mode", t.mode.clone()),
        ("extra", t.extra.clone()),
        ("seed", t.seed.clone()),
        ("pqv", tls.mldsa65_verify.clone()),
        ("pcs", tls.pinned_sha256.clone()),
        ("vcn", tls.verify_names.clone()),
        ("ech", tls.ech_config.clone()),
        ("allowInsecure", if tls.allow_insecure { "1".into() } else { String::new() }),
    ]);
    format!("{scheme}://{}@{}{}{}", percent_encode(user), host_port(profile), query_string(&pairs), fragment(profile))
}

fn vmess_link(profile: &Profile) -> String {
    let t = &profile.transport;
    let (header_type, path) = match t.network {
        Network::Grpc => (t.mode.clone(), t.service_name.clone()),
        Network::Kcp => (t.header_type.clone(), t.seed.clone()),
        Network::Xhttp => (t.mode.clone(), t.path.clone()),
        _ => (t.header_type.clone(), t.path.clone()),
    };
    let value = json!({
        "v": "2",
        "ps": profile.name,
        "add": profile.address,
        "port": profile.port.to_string(),
        "id": profile.uuid,
        "aid": profile.alter_id.to_string(),
        "scy": if profile.method.is_empty() { "auto" } else { profile.method.as_str() },
        "net": t.network.as_str(),
        "type": if header_type.is_empty() { "none".to_string() } else { header_type },
        "host": t.host,
        "path": path,
        "tls": if profile.tls.security == Security::Tls { "tls" } else { "" },
        "sni": profile.tls.sni,
        "alpn": profile.tls.alpn.join(","),
        "fp": profile.tls.fingerprint,
        "pcs": profile.tls.pinned_sha256,
        "vcn": profile.tls.verify_names,
        "ech": profile.tls.ech_config,
    });
    format!("vmess://{}", b64_encode(value.to_string().as_bytes()))
}

fn shadowsocks_link(profile: &Profile) -> String {
    let userinfo = b64_encode_url(format!("{}:{}", profile.method, profile.password).as_bytes());
    let plugin = query_string(&[("plugin", profile.plugin.clone())]);
    let plugin = if plugin.is_empty() { plugin } else { format!("/{plugin}") };
    format!("ss://{userinfo}@{}{plugin}{}", host_port(profile), fragment(profile))
}

fn hysteria2_link(profile: &Profile) -> String {
    let q = &profile.quic;
    let pairs = [
        ("sni", profile.tls.sni.clone()),
        ("alpn", profile.tls.alpn.join(",")),
        ("insecure", if profile.tls.allow_insecure { "1".into() } else { String::new() }),
        ("pinSHA256", profile.tls.pinned_sha256.clone()),
        ("obfs", q.obfs.clone()),
        ("obfs-password", q.obfs_password.clone()),
        ("mport", q.hop_ports.clone()),
    ];
    format!("hysteria2://{}@{}/{}{}", percent_encode(&profile.password), host_port(profile), query_string(&pairs), fragment(profile))
}

fn tuic_link(profile: &Profile) -> String {
    let q = &profile.quic;
    let pairs = [
        ("sni", profile.tls.sni.clone()),
        ("alpn", profile.tls.alpn.join(",")),
        ("allow_insecure", if profile.tls.allow_insecure { "1".into() } else { String::new() }),
        ("congestion_control", q.congestion_control.clone()),
        ("udp_relay_mode", q.udp_relay_mode.clone()),
    ];
    format!(
        "tuic://{}:{}@{}{}{}",
        percent_encode(&profile.uuid),
        percent_encode(&profile.password),
        host_port(profile),
        query_string(&pairs),
        fragment(profile)
    )
}
