//! Data model shared by the core library, the Tauri shell and the UI (serialized as camelCase JSON).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Hysteria2,
    Tuic,
}

impl Protocol {
    /// Protocols that Xray cannot run and that therefore need sing-box as the main core.
    pub fn needs_singbox(self) -> bool {
        matches!(self, Protocol::Hysteria2 | Protocol::Tuic)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Vless => "vless",
            Protocol::Vmess => "vmess",
            Protocol::Trojan => "trojan",
            Protocol::Shadowsocks => "shadowsocks",
            Protocol::Hysteria2 => "hysteria2",
            Protocol::Tuic => "tuic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    #[default]
    Tcp,
    Ws,
    Grpc,
    Xhttp,
    #[serde(rename = "httpupgrade")]
    HttpUpgrade,
    H2,
    Kcp,
    Quic,
}

impl Network {
    pub fn parse(value: &str) -> Network {
        match value.to_ascii_lowercase().as_str() {
            "ws" | "websocket" => Network::Ws,
            "grpc" | "gun" => Network::Grpc,
            "xhttp" | "splithttp" => Network::Xhttp,
            "httpupgrade" => Network::HttpUpgrade,
            "h2" | "http" => Network::H2,
            "kcp" | "mkcp" => Network::Kcp,
            "quic" => Network::Quic,
            _ => Network::Tcp,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Network::Tcp => "tcp",
            Network::Ws => "ws",
            Network::Grpc => "grpc",
            Network::Xhttp => "xhttp",
            Network::HttpUpgrade => "httpupgrade",
            Network::H2 => "h2",
            Network::Kcp => "kcp",
            Network::Quic => "quic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    #[default]
    None,
    Tls,
    Reality,
}

impl Security {
    pub fn parse(value: &str) -> Security {
        match value.to_ascii_lowercase().as_str() {
            "tls" | "xtls" => Security::Tls,
            "reality" => Security::Reality,
            _ => Security::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Security::None => "none",
            Security::Tls => "tls",
            Security::Reality => "reality",
        }
    }
}

/// Stream transport options (Xray `streamSettings` minus security).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Transport {
    pub network: Network,
    /// `headerType` for tcp/kcp/quic (`none`, `http`, ...).
    pub header_type: String,
    /// Host header / authority.
    pub host: String,
    pub path: String,
    pub service_name: String,
    /// gRPC `gun`/`multi`, xHTTP `auto`/`packet-up`/`stream-up`/`stream-one`.
    pub mode: String,
    /// xHTTP `extra` JSON as a raw string.
    pub extra: String,
    /// mKCP seed.
    pub seed: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TlsOptions {
    pub security: Security,
    pub sni: String,
    pub alpn: Vec<String>,
    /// uTLS fingerprint (`chrome`, `firefox`, ...).
    pub fingerprint: String,
    pub allow_insecure: bool,
    /// Reality public key (`pbk`).
    pub public_key: String,
    /// Reality short id (`sid`).
    pub short_id: String,
    /// Reality spider path (`spx`).
    pub spider_x: String,
    /// Reality post-quantum ML-DSA-65 verify key (`pqv`).
    pub mldsa65_verify: String,
    /// Pinned SHA-256 of the server (or CA) certificate, hex, comma separated (`pcs`).
    pub pinned_sha256: String,
    /// Accept the certificate when it is valid for one of these names (`vcn`).
    pub verify_names: String,
    /// Encrypted Client Hello config list (`ech`).
    pub ech_config: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuicOptions {
    /// Hysteria2 obfuscation type (`salamander`).
    pub obfs: String,
    pub obfs_password: String,
    pub up_mbps: u32,
    pub down_mbps: u32,
    /// Hysteria2 port hopping range, e.g. `20000-30000`.
    pub hop_ports: String,
    /// TUIC congestion control (`bbr`, `cubic`, `new_reno`).
    pub congestion_control: String,
    /// TUIC UDP relay mode (`native`, `quic`).
    pub udp_relay_mode: String,
}

/// Result of the last latency measurement of a profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DelayInfo {
    /// Milliseconds, `-1` means timeout / unreachable, `None` means not tested.
    pub ms: Option<i64>,
    pub method: Option<TestMethod>,
    pub tested_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestMethod {
    /// HTTP GET through the proxy (v2rayNG "real delay").
    #[default]
    Real,
    Tcp,
    Icmp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    pub group: String,
    pub subscription_id: Option<Uuid>,
    pub protocol: Protocol,
    pub address: String,
    pub port: u16,
    /// UUID for VLESS/VMess/TUIC.
    pub uuid: String,
    /// Password for Trojan/Shadowsocks/Hysteria2/TUIC.
    pub password: String,
    pub alter_id: u32,
    /// VMess security / Shadowsocks method / VLESS encryption.
    pub method: String,
    /// VLESS flow (`xtls-rprx-vision`).
    pub flow: String,
    /// Shadowsocks SIP003 plugin string (kept for round-trips).
    pub plugin: String,
    pub transport: Transport,
    pub tls: TlsOptions,
    pub quic: QuicOptions,
    /// DNS server provided by the key/subscription, used when `dns.useProfileDns` is on.
    pub dns: String,
    pub delay: DelayInfo,
    pub created_at: DateTime<Utc>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            id: Uuid::new_v4(),
            name: String::new(),
            group: DEFAULT_GROUP.to_string(),
            subscription_id: None,
            protocol: Protocol::Vless,
            address: String::new(),
            port: 443,
            uuid: String::new(),
            password: String::new(),
            alter_id: 0,
            method: String::new(),
            flow: String::new(),
            plugin: String::new(),
            transport: Transport::default(),
            tls: TlsOptions::default(),
            quic: QuicOptions::default(),
            dns: String::new(),
            delay: DelayInfo::default(),
            created_at: Utc::now(),
        }
    }
}

pub const DEFAULT_GROUP: &str = "Default";

impl Profile {
    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            format!("{}:{}", self.address, self.port)
        } else {
            self.name.clone()
        }
    }

    /// Key used for de-duplication: same endpoint and credentials means same server.
    pub fn identity_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.protocol.as_str(),
            self.address.to_ascii_lowercase(),
            self.port,
            self.uuid,
            self.password,
            self.transport.network.as_str(),
            self.transport.path
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Subscription {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    /// Auto update interval in hours, 0 disables auto update.
    pub auto_update_hours: u32,
    /// Overrides the global subscription User-Agent when not empty.
    pub user_agent: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    /// Values from the `subscription-userinfo` header.
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<i64>,
}

impl Default for Subscription {
    fn default() -> Self {
        Subscription {
            id: Uuid::new_v4(),
            name: String::new(),
            url: String::new(),
            enabled: true,
            auto_update_hours: 12,
            user_agent: String::new(),
            updated_at: None,
            last_error: None,
            upload: None,
            download: None,
            total: None,
            expire: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    #[default]
    Domain,
    Ip,
    Port,
    /// Executable name, only effective in TUN mode (sing-box).
    Process,
    /// Sniffed protocol: `http`, `tls`, `quic`, `bittorrent`.
    Protocol,
    /// `tcp` / `udp`.
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleOutbound {
    #[default]
    Proxy,
    Direct,
    Block,
}

impl RuleOutbound {
    pub fn tag(self) -> &'static str {
        match self {
            RuleOutbound::Proxy => "proxy",
            RuleOutbound::Direct => "direct",
            RuleOutbound::Block => "block",
        }
    }
}

/// A user-defined routing rule. Values use Xray syntax (`geosite:google`, `domain:x.com`, `geoip:ru`, `1000-2000`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoutingRule {
    pub id: Uuid,
    pub enabled: bool,
    pub remark: String,
    pub kind: RuleKind,
    pub values: Vec<String>,
    pub outbound: RuleOutbound,
}

impl Default for RoutingRule {
    fn default() -> Self {
        RoutingRule {
            id: Uuid::new_v4(),
            enabled: true,
            remark: String::new(),
            kind: RuleKind::Domain,
            values: Vec::new(),
            outbound: RuleOutbound::Proxy,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: DateTime<Utc>,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    /// Parsed access-log fields, present for `LogSource::Access`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    App,
    Core,
    Access,
}

/// One proxied request taken from the Xray access log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRecord {
    pub source: String,
    pub destination: String,
    pub network: String,
    pub inbound: String,
    pub outbound: String,
    pub accepted: bool,
}
