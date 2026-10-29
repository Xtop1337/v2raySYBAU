//! Latency measurement: TCP connect, ICMP echo and v2rayNG-style "real delay" through the proxy.

mod batch;
mod certpin;
mod icmp;
mod monitor;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::model::TestMethod;
pub use batch::{run_batch, BatchOptions};
pub use certpin::{leaf_sha256, needs_pin, peer_cert, pin_insecure, PeerCert};
pub use icmp::icmp_ping;
pub use monitor::{NodeStats, NodeWindow};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayResult {
    pub profile_id: Uuid,
    /// Milliseconds, `-1` when the server did not answer.
    pub ms: i64,
    pub method: TestMethod,
}

async fn resolve(host: &str, port: u16) -> Option<SocketAddr> {
    let mut addrs = tokio::net::lookup_host((host, port)).await.ok()?;
    let all: Vec<SocketAddr> = addrs.by_ref().collect();
    all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
}

/// Time of a TCP handshake with `host:port` (DNS resolution not included), like `tcping`.
pub async fn tcp_ping(host: &str, port: u16, timeout: Duration) -> Option<Duration> {
    let addr = tokio::time::timeout(timeout, resolve(host, port)).await.ok()??;
    let start = Instant::now();
    tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await.ok()?.ok()?;
    Some(start.elapsed())
}

/// HTTP GET of `url` through the local SOCKS5 port: the full path client → proxy → site, the
/// same "real delay" v2rayNG shows. Any HTTP status counts as a response (generate_204 gives 204).
pub async fn http_delay(socks_port: u16, url: &str, timeout: Duration) -> Option<Duration> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks_port}")).ok()?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .connect_timeout(timeout)
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let start = Instant::now();
    let response = client.get(url).header("User-Agent", "Mozilla/5.0").send().await.ok()?;
    let elapsed = start.elapsed();
    let _ = response.bytes().await;
    Some(elapsed)
}

/// Public address the internet sees (and its country), as reported by an IP echo service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitInfo {
    pub ip: String,
    /// ISO 3166 alpha-2 code, empty when unknown.
    pub country: String,
}

const TRACE_URLS: [&str; 2] = ["https://www.cloudflare.com/cdn-cgi/trace", "https://1.1.1.1/cdn-cgi/trace"];

/// Asks Cloudflare which address the request came from. `proxy` is a proxy URL
/// (`socks5h://...`, `http://...`), or `None` for a request that takes the system route
/// (in TUN mode that is the tunnel). System proxy settings are never used implicitly.
pub async fn exit_info(proxy: Option<&str>, timeout: Duration) -> Option<ExitInfo> {
    let mut builder = reqwest::Client::builder().timeout(timeout).connect_timeout(timeout).pool_max_idle_per_host(0).no_proxy();
    if let Some(proxy) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy).ok()?);
    }
    let client = builder.build().ok()?;
    // SYBAU_TRACE_URL points the check at another trace service (used by the UI tests).
    let custom = std::env::var("SYBAU_TRACE_URL").ok();
    let urls: Vec<&str> = match &custom {
        Some(url) => vec![url.as_str()],
        None => TRACE_URLS.to_vec(),
    };
    for url in urls {
        let Ok(response) = client.get(url).header("User-Agent", "Mozilla/5.0").send().await else {
            continue;
        };
        if let Some(info) = response.text().await.ok().as_deref().and_then(parse_trace) {
            return Some(info);
        }
    }
    None
}

fn parse_trace(body: &str) -> Option<ExitInfo> {
    let field = |name: &str| body.lines().find_map(|l| l.strip_prefix(name).and_then(|v| v.strip_prefix('='))).map(str::trim);
    let ip = field("ip").filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())?;
    let country = field("loc").filter(|c| c.len() == 2 && *c != "XX").unwrap_or("");
    Some(ExitInfo { ip: ip.to_string(), country: country.to_ascii_uppercase() })
}

pub fn to_ms(duration: Option<Duration>) -> i64 {
    duration.map_or(-1, |d| d.as_millis().max(1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cloudflare_trace() {
        let body = "fl=12f\nh=www.cloudflare.com\nip=203.0.113.7\nts=1.2\nloc=NL\ntls=TLSv1.3\n";
        assert_eq!(parse_trace(body), Some(ExitInfo { ip: "203.0.113.7".into(), country: "NL".into() }));
        assert_eq!(parse_trace("ip=2001:db8::1\nloc=XX").unwrap().country, "");
        assert_eq!(parse_trace("<html>blocked</html>"), None);
    }

    #[tokio::test]
    async fn tcp_ping_local_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { while listener.accept().await.is_ok() {} });
        assert!(tcp_ping("127.0.0.1", port, Duration::from_secs(1)).await.is_some());
        let closed = crate::runtime::free_ports(1).unwrap()[0];
        assert!(tcp_ping("127.0.0.1", closed, Duration::from_millis(500)).await.is_none());
    }
}
