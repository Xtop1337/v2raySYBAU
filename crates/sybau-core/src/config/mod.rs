//! Generation of core configurations.
//!
//! * Proxy mode: Xray exposes SOCKS + HTTP inbounds (sing-box for Hysteria2/TUIC).
//! * TUN mode: sing-box owns the virtual adapter. For Xray protocols it forwards to the local
//!   Xray SOCKS inbound (like v2rayN); Hysteria2/TUIC are handled by sing-box natively.

pub mod singbox;
pub mod xray;

use serde_json::Value;

use crate::model::Profile;
use crate::settings::{ConnectionMode, Settings};

/// Facts about the environment that influence the generated configs.
#[derive(Debug, Clone)]
pub struct ConfigEnv {
    /// `geoip.dat` / `geosite.dat` are present next to xray.
    pub geo_available: bool,
    /// Executable names of the cores, excluded from the tunnel to avoid loops.
    pub core_process_names: Vec<String>,
    /// Port of the sing-box Clash API used for traffic statistics.
    pub clash_api_port: u16,
}

impl Default for ConfigEnv {
    fn default() -> Self {
        ConfigEnv {
            geo_available: true,
            core_process_names: vec!["xray.exe".into(), "sing-box.exe".into()],
            clash_api_port: 10814,
        }
    }
}

/// Which processes have to run, and with which configs, to connect to a profile.
#[derive(Debug, Clone, Default)]
pub struct LaunchPlan {
    pub xray: Option<Value>,
    pub singbox: Option<Value>,
}

pub fn plan(profile: &Profile, settings: &Settings, env: &ConfigEnv) -> LaunchPlan {
    let native_singbox = profile.protocol.needs_singbox();
    match (settings.proxy.mode, native_singbox) {
        (ConnectionMode::Proxy, false) => LaunchPlan {
            xray: Some(xray::build_client(profile, settings, env, true)),
            singbox: None,
        },
        (ConnectionMode::Proxy, true) => LaunchPlan {
            xray: None,
            singbox: Some(singbox::build(profile, settings, env, singbox::Mode::Proxy)),
        },
        (ConnectionMode::Tun, false) => LaunchPlan {
            xray: Some(xray::build_client(profile, settings, env, false)),
            singbox: Some(singbox::build(profile, settings, env, singbox::Mode::TunOverXray)),
        },
        (ConnectionMode::Tun, true) => LaunchPlan {
            xray: None,
            singbox: Some(singbox::build(profile, settings, env, singbox::Mode::TunNative)),
        },
    }
}

/// Private and reserved ranges that never go through the proxy.
pub const PRIVATE_CIDRS: &[&str] = &[
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "224.0.0.0/4",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

/// Russian TLDs used by the "bypass RU" preset even without geosite files.
pub const RU_DOMAIN_SUFFIXES: &[&str] = &["ru", "su", "xn--p1ai", "moscow", "xn--80adxhks"];

/// Host part of a DNS server spec such as `https://1.1.1.1/dns-query`, `tcp://8.8.8.8:53`, `8.8.8.8`.
pub fn dns_host(server: &str) -> String {
    let without_scheme = server.split_once("://").map_or(server, |(_, rest)| rest);
    let authority = without_scheme.split('/').next().unwrap_or("");
    if let Some(stripped) = authority.strip_prefix('[') {
        return stripped.split(']').next().unwrap_or("").to_string();
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host.to_string(),
        _ => authority.to_string(),
    }
}

pub fn is_ip(value: &str) -> bool {
    value.parse::<std::net::IpAddr>().is_ok()
}

/// DNS server that should resolve proxied domains: from the key when allowed, otherwise the setting.
pub fn remote_dns(profile: &Profile, settings: &Settings) -> String {
    if settings.dns.use_profile_dns && !profile.dns.trim().is_empty() {
        profile.dns.trim().to_string()
    } else {
        settings.dns.remote.trim().to_string()
    }
}

pub fn to_pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[cfg(test)]
mod tests;
