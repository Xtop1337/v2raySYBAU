//! Application settings. Every section is `#[serde(default)]` so older settings files keep loading.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{RoutingRule, TestMethod};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub general: GeneralSettings,
    pub proxy: ProxySettings,
    pub tun: TunSettings,
    pub dns: DnsSettings,
    pub routing: RoutingSettings,
    pub mux: MuxSettings,
    pub fragment: FragmentSettings,
    pub analysis: AnalysisSettings,
    pub test: TestSettings,
    pub advanced: AdvancedSettings,
    pub last_profile_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GeneralSettings {
    /// `auto`, `ru` or `en`.
    pub language: String,
    pub theme: Theme,
    pub autostart: bool,
    pub auto_connect: bool,
    /// Run a latency test of all servers on launch.
    pub scan_on_start: bool,
    pub confirm_delete: bool,
    pub show_speed: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub update_subscriptions_on_start: bool,
    pub check_updates: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            language: "auto".into(),
            theme: Theme::System,
            autostart: false,
            auto_connect: false,
            scan_on_start: false,
            confirm_delete: true,
            show_speed: true,
            minimize_to_tray: true,
            start_minimized: false,
            update_subscriptions_on_start: true,
            check_updates: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    /// Local SOCKS/HTTP proxy, optionally set as the Windows system proxy.
    #[default]
    Proxy,
    /// Virtual adapter (sing-box TUN) capturing the whole system or selected apps.
    Tun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProxySettings {
    pub mode: ConnectionMode,
    pub socks_port: u16,
    pub http_port: u16,
    pub system_proxy: bool,
    /// Semicolon separated WinINET bypass list.
    pub bypass: String,
    pub allow_lan: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        ProxySettings {
            mode: ConnectionMode::Proxy,
            socks_port: 10808,
            http_port: 10809,
            system_proxy: true,
            bypass: "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.2*;172.30.*;172.31.*;192.168.*;<local>".into(),
            allow_lan: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunAppMode {
    /// Every application goes through the tunnel.
    #[default]
    All,
    /// Only the listed applications go through the tunnel ("proxy for selected apps").
    Include,
    /// Every application except the listed ones.
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TunSettings {
    pub app_mode: TunAppMode,
    /// Executable names such as `chrome.exe`, `Telegram.exe`.
    pub apps: Vec<String>,
    /// `system`, `gvisor` or `mixed`.
    pub stack: String,
    pub mtu: u32,
    pub strict_route: bool,
}

impl Default for TunSettings {
    fn default() -> Self {
        TunSettings {
            app_mode: TunAppMode::All,
            apps: Vec::new(),
            stack: "mixed".into(),
            mtu: 9000,
            strict_route: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DnsSettings {
    /// DNS used for proxied domains (resolved through the tunnel).
    pub remote: String,
    /// DNS used for direct domains.
    pub direct: String,
    /// Prefer the DNS server provided by the key/subscription.
    pub use_profile_dns: bool,
    /// Expose a local DNS server on `local_port` (v2rayNG "local DNS").
    pub local_dns: bool,
    pub local_port: u16,
    /// FakeDNS: answer with fake IPs and route by domain.
    pub fake_dns: bool,
    /// Xray `dns.queryStrategy`: `UseIP`, `UseIPv4`, `UseIPv6`.
    pub query_strategy: String,
    /// Extra static hosts, one `domain ip` pair per line.
    pub hosts: String,
}

impl Default for DnsSettings {
    fn default() -> Self {
        DnsSettings {
            remote: "https://1.1.1.1/dns-query".into(),
            direct: "8.8.8.8".into(),
            use_profile_dns: true,
            local_dns: false,
            local_port: 10853,
            fake_dns: false,
            query_strategy: "UseIPv4".into(),
            hosts: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoutingPreset {
    /// Everything through the proxy.
    Global,
    /// Private networks direct, everything else proxied.
    #[default]
    BypassLan,
    /// Private networks and Russian sites direct.
    BypassLanRu,
    /// Only the custom rules decide; unmatched traffic goes direct.
    DirectByDefault,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoutingSettings {
    pub preset: RoutingPreset,
    /// Xray `routing.domainStrategy`: `AsIs`, `IPIfNonMatch`, `IPOnDemand`.
    pub domain_strategy: String,
    pub sniffing: bool,
    /// Sniffed domain is used only for routing, the connection keeps its original destination.
    pub route_only: bool,
    pub block_ads: bool,
    /// Custom rules, evaluated in order before the preset rules.
    pub rules: Vec<RoutingRule>,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        RoutingSettings {
            preset: RoutingPreset::BypassLan,
            domain_strategy: "IPIfNonMatch".into(),
            sniffing: true,
            route_only: false,
            block_ads: false,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MuxSettings {
    pub enabled: bool,
    pub concurrency: i32,
    pub xudp_concurrency: i32,
    /// `reject`, `allow` or `skip`.
    pub xudp_proxy_udp443: String,
}

impl Default for MuxSettings {
    fn default() -> Self {
        MuxSettings {
            enabled: false,
            concurrency: 8,
            xudp_concurrency: 16,
            xudp_proxy_udp443: "reject".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FragmentSettings {
    pub enabled: bool,
    /// `tlshello` or a range such as `1-3`.
    pub packets: String,
    pub length: String,
    pub interval: String,
}

impl Default for FragmentSettings {
    fn default() -> Self {
        FragmentSettings {
            enabled: false,
            packets: "tlshello".into(),
            length: "100-200".into(),
            interval: "10-20".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MonitorScope {
    /// Only the connected server.
    #[default]
    Active,
    /// Every server of the selected group.
    Group,
    /// Every server.
    All,
}

/// Per-node packet analysis: periodic probes giving latency, jitter and loss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AnalysisSettings {
    pub enabled: bool,
    pub interval_secs: u32,
    /// Number of samples in the sliding window.
    pub window: u32,
    pub method: TestMethod,
    pub scope: MonitorScope,
}

impl Default for AnalysisSettings {
    fn default() -> Self {
        AnalysisSettings {
            enabled: true,
            interval_secs: 5,
            window: 30,
            method: TestMethod::Tcp,
            scope: MonitorScope::Active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TestSettings {
    pub url: String,
    pub timeout_ms: u64,
    pub concurrency: usize,
    pub method: TestMethod,
}

impl Default for TestSettings {
    fn default() -> Self {
        TestSettings {
            url: "https://www.gstatic.com/generate_204".into(),
            timeout_ms: 5000,
            concurrency: 16,
            method: TestMethod::Real,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AdvancedSettings {
    /// Xray log level: `debug`, `info`, `warning`, `error`, `none`.
    pub log_level: String,
    /// Write the access log (every request) and show it in the journal.
    pub access_log: bool,
    pub allow_insecure: bool,
    /// User-Agent used to download subscriptions.
    pub subscription_user_agent: String,
    /// JSON object deep-merged into the generated Xray config.
    pub config_override: String,
    pub update_prerelease: bool,
    /// Port of the Xray stats API.
    pub api_port: u16,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        AdvancedSettings {
            log_level: "warning".into(),
            access_log: true,
            allow_insecure: false,
            // Panels (Marzban, Remnawave, 3x-ui) serve plain share links to v2rayN-compatible clients.
            subscription_user_agent: "v2rayN/7.10.0".into(),
            config_override: String::new(),
            update_prerelease: false,
            api_port: 10813,
        }
    }
}
