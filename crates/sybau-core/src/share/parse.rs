//! Share link parsing: vless, vmess, trojan, ss, hysteria2/hy2, tuic.

use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

use crate::model::{Network, Profile, Protocol, Security};
use crate::util::{b64_decode_str, percent_decode, split_list};

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("unsupported link scheme")]
    UnsupportedScheme,
    #[error("malformed link: {0}")]
    Malformed(String),
}

type Result<T> = std::result::Result<T, ParseError>;

pub fn is_share_link(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    ["vless://", "vmess://", "trojan://", "ss://", "hysteria2://", "hy2://", "tuic://"].iter().any(|scheme| lower.starts_with(scheme))
}

pub fn parse_link(line: &str) -> Result<Profile> {
    let line = line.trim();
    let scheme = line.split_once("://").map(|(scheme, _)| scheme.to_ascii_lowercase()).ok_or(ParseError::UnsupportedScheme)?;
    match scheme.as_str() {
        "vless" => parse_standard(line, Protocol::Vless),
        "vmess" => parse_vmess(line),
        "trojan" => parse_standard(line, Protocol::Trojan),
        "ss" => parse_shadowsocks(line),
        "hysteria2" | "hy2" => parse_hysteria2(line),
        "tuic" => parse_tuic(line),
        _ => Err(ParseError::UnsupportedScheme),
    }
}

/// Parts of a `scheme://userinfo@host:port/path?query#fragment` link, decoded by hand so that
/// unescaped characters (spaces, emoji, `|` in names) that real-world links contain do not fail.
struct LinkParts {
    user: String,
    host: String,
    port: u16,
    query: HashMap<String, String>,
    name: String,
}

fn split_link(line: &str) -> Result<LinkParts> {
    let (_, rest) = line.split_once("://").ok_or_else(|| ParseError::Malformed("missing scheme".into()))?;
    let (rest, fragment) = match rest.split_once('#') {
        Some((rest, fragment)) => (rest, percent_decode(fragment)),
        None => (rest, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((rest, query)) => (rest, parse_query(query)),
        None => (rest, HashMap::new()),
    };
    // Passwords may contain `/`, so the authority ends at the first `/` after the last `@`.
    let host_start = rest.rfind('@').map_or(0, |index| index + 1);
    let authority = match rest[host_start..].find('/') {
        Some(index) => &rest[..host_start + index],
        None => rest,
    };
    let (user, host_port) = match authority.rfind('@') {
        Some(index) => (percent_decode(&authority[..index]), &authority[index + 1..]),
        None => (String::new(), authority),
    };
    let (host, port) = split_host_port(host_port)?;
    Ok(LinkParts { user, host, port, query, name: fragment.trim().to_string() })
}

fn split_host_port(value: &str) -> Result<(String, u16)> {
    let (host, port) = if let Some(stripped) = value.strip_prefix('[') {
        let (host, rest) = stripped.split_once(']').ok_or_else(|| ParseError::Malformed("bad IPv6 address".into()))?;
        (host.to_string(), rest.trim_start_matches(':'))
    } else {
        match value.rsplit_once(':') {
            Some((host, port)) => (host.to_string(), port),
            None => (value.to_string(), ""),
        }
    };
    if host.is_empty() {
        return Err(ParseError::Malformed("missing host".into()));
    }
    // Hysteria2 port hopping links may carry `443,20000-30000`; the first port is the main one.
    let first = port.split([',', '-']).next().unwrap_or("");
    let port =
        if first.is_empty() { 443 } else { first.parse::<u16>().map_err(|_| ParseError::Malformed(format!("bad port `{port}`")))? };
    Ok((percent_decode(&host), port))
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (percent_decode(key), percent_decode(&value.replace('+', "%20"))),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn get<'a>(query: &'a HashMap<String, String>, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|key| query.get(*key).filter(|v| !v.is_empty())).map(String::as_str).unwrap_or("")
}

fn truthy(value: &str) -> bool {
    matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
}

/// Applies the common transport/TLS query parameters used by vless, trojan and URI-style vmess.
fn apply_stream_query(profile: &mut Profile, query: &HashMap<String, String>) {
    let transport = &mut profile.transport;
    transport.network = Network::parse(get(query, &["type", "net"]));
    transport.header_type = get(query, &["headerType"]).to_string();
    transport.host = get(query, &["host", "authority"]).to_string();
    transport.path = get(query, &["path"]).to_string();
    transport.service_name = get(query, &["serviceName"]).to_string();
    transport.mode = get(query, &["mode"]).to_string();
    transport.extra = get(query, &["extra"]).to_string();
    transport.seed = get(query, &["seed"]).to_string();
    if transport.network == Network::Kcp && transport.path.is_empty() {
        transport.path = transport.seed.clone();
    }
    if transport.network == Network::Grpc && transport.service_name.is_empty() {
        transport.service_name = transport.path.clone();
    }

    let security = get(query, &["security"]);
    if !security.is_empty() {
        profile.tls.security = Security::parse(security);
    }
    let tls = &mut profile.tls;
    tls.sni = get(query, &["sni", "peer"]).to_string();
    tls.alpn = split_list(get(query, &["alpn"]));
    tls.fingerprint = get(query, &["fp"]).to_string();
    tls.public_key = get(query, &["pbk"]).to_string();
    tls.short_id = get(query, &["sid"]).to_string();
    tls.spider_x = get(query, &["spx"]).to_string();
    tls.allow_insecure = truthy(get(query, &["allowInsecure", "insecure"]));

    profile.flow = get(query, &["flow"]).to_string();
    profile.dns = get(query, &["dns"]).to_string();
}

fn parse_standard(line: &str, protocol: Protocol) -> Result<Profile> {
    let parts = split_link(line)?;
    if parts.user.is_empty() {
        return Err(ParseError::Malformed("missing user id".into()));
    }
    let mut profile = Profile { protocol, name: parts.name, address: parts.host, port: parts.port, ..Profile::default() };
    match protocol {
        Protocol::Trojan => {
            profile.password = parts.user;
            profile.tls.security = Security::Tls;
        }
        _ => {
            profile.uuid = parts.user;
            profile.method = get(&parts.query, &["encryption"]).to_string();
            if profile.method.is_empty() {
                profile.method = "none".into();
            }
        }
    }
    apply_stream_query(&mut profile, &parts.query);
    Ok(profile)
}

fn parse_vmess(line: &str) -> Result<Profile> {
    let payload = &line.trim()["vmess://".len()..];
    let payload = payload.split('#').next().unwrap_or("");
    let Some(json) = b64_decode_str(payload) else {
        // Some generators emit `vmess://uuid@host:port?...` instead of base64 JSON.
        let mut profile = parse_standard(line, Protocol::Vmess)?;
        profile.method = "auto".into();
        if let Some(query) = line.split_once('?').map(|(_, q)| q.split('#').next().unwrap_or("")) {
            let query = parse_query(query);
            let scy = get(&query, &["encryption", "scy"]);
            if !scy.is_empty() && scy != "none" {
                profile.method = scy.to_string();
            }
        }
        return Ok(profile);
    };
    let root: Value = serde_json::from_str(&json).map_err(|e| ParseError::Malformed(format!("vmess json: {e}")))?;
    let read = |key: &str| -> String {
        match root.get(key) {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => String::new(),
        }
    };
    let address = read("add");
    if address.is_empty() {
        return Err(ParseError::Malformed("vmess without address".into()));
    }
    let mut profile = Profile {
        protocol: Protocol::Vmess,
        name: read("ps"),
        address,
        port: read("port").parse().unwrap_or(443),
        uuid: read("id"),
        alter_id: read("aid").parse().unwrap_or(0),
        method: Some(read("scy")).filter(|s| !s.is_empty()).unwrap_or_else(|| "auto".into()),
        ..Profile::default()
    };
    let transport = &mut profile.transport;
    transport.network = Network::parse(&read("net"));
    transport.header_type = read("type");
    if transport.header_type == "none" {
        transport.header_type.clear();
    }
    transport.host = read("host");
    transport.path = read("path");
    match transport.network {
        Network::Grpc => {
            transport.service_name = transport.path.clone();
            transport.mode = transport.header_type.clone();
            transport.header_type.clear();
        }
        Network::Kcp => transport.seed = transport.path.clone(),
        Network::Xhttp => transport.mode = transport.header_type.clone(),
        _ => {}
    }
    profile.tls.security = Security::parse(&read("tls"));
    profile.tls.sni = read("sni");
    profile.tls.alpn = split_list(&read("alpn"));
    profile.tls.fingerprint = read("fp");
    profile.tls.allow_insecure = truthy(&read("allowInsecure")) || truthy(&read("insecure"));
    Ok(profile)
}

fn parse_shadowsocks(line: &str) -> Result<Profile> {
    let body = &line.trim()["ss://".len()..];
    let (body, name) = match body.split_once('#') {
        Some((body, name)) => (body, percent_decode(name).trim().to_string()),
        None => (body, String::new()),
    };
    let (body, query) = match body.split_once('?') {
        Some((body, query)) => (body.trim_end_matches('/'), parse_query(query)),
        None => (body.trim_end_matches('/'), HashMap::new()),
    };

    // SIP002 `userinfo@host:port`, where userinfo is base64(method:password) or percent-encoded.
    // Legacy: base64(method:password@host:port).
    let decoded_whole;
    let full = if body.contains('@') {
        body
    } else {
        decoded_whole = b64_decode_str(body).ok_or_else(|| ParseError::Malformed("bad ss base64".into()))?;
        decoded_whole.as_str()
    };
    let at = full.rfind('@').ok_or_else(|| ParseError::Malformed("ss without host".into()))?;
    let (userinfo, host_port) = (&full[..at], &full[at + 1..]);
    let userinfo = if userinfo.contains(':') {
        percent_decode(userinfo)
    } else {
        b64_decode_str(&percent_decode(userinfo)).ok_or_else(|| ParseError::Malformed("bad ss userinfo".into()))?
    };
    let (method, password) = userinfo.split_once(':').ok_or_else(|| ParseError::Malformed("ss userinfo without password".into()))?;
    let (host, port) = split_host_port(host_port)?;
    Ok(Profile {
        protocol: Protocol::Shadowsocks,
        name,
        address: host,
        port,
        method: method.to_string(),
        password: password.to_string(),
        plugin: get(&query, &["plugin"]).to_string(),
        ..Profile::default()
    })
}

fn parse_hysteria2(line: &str) -> Result<Profile> {
    let parts = split_link(line)?;
    let query = &parts.query;
    let mut profile = Profile {
        protocol: Protocol::Hysteria2,
        name: parts.name,
        address: parts.host,
        port: parts.port,
        password: parts.user,
        ..Profile::default()
    };
    profile.tls.security = Security::Tls;
    profile.tls.sni = get(query, &["sni", "peer"]).to_string();
    profile.tls.alpn = split_list(get(query, &["alpn"]));
    profile.tls.allow_insecure = truthy(get(query, &["insecure", "allowInsecure"]));
    profile.quic.obfs = get(query, &["obfs"]).to_string();
    profile.quic.obfs_password = get(query, &["obfs-password", "obfs_password"]).to_string();
    profile.quic.hop_ports = get(query, &["mport", "ports"]).to_string();
    profile.quic.up_mbps = get(query, &["upmbps", "up"]).parse().unwrap_or(0);
    profile.quic.down_mbps = get(query, &["downmbps", "down"]).parse().unwrap_or(0);
    Ok(profile)
}

fn parse_tuic(line: &str) -> Result<Profile> {
    let parts = split_link(line)?;
    let query = &parts.query;
    let (uuid, password) = match parts.user.split_once(':') {
        Some((uuid, password)) => (uuid.to_string(), password.to_string()),
        None => (parts.user.clone(), get(query, &["password"]).to_string()),
    };
    let mut profile =
        Profile { protocol: Protocol::Tuic, name: parts.name, address: parts.host, port: parts.port, uuid, password, ..Profile::default() };
    profile.tls.security = Security::Tls;
    profile.tls.sni = get(query, &["sni"]).to_string();
    profile.tls.alpn = split_list(get(query, &["alpn"]));
    profile.tls.allow_insecure = truthy(get(query, &["allow_insecure", "insecure", "allowInsecure"]));
    profile.quic.congestion_control = get(query, &["congestion_control", "congestion"]).to_string();
    profile.quic.udp_relay_mode = get(query, &["udp_relay_mode"]).to_string();
    Ok(profile)
}
