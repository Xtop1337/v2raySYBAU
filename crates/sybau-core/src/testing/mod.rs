//! Latency measurement: TCP connect, ICMP echo and v2rayNG-style "real delay" through the proxy.

mod batch;
mod icmp;
mod monitor;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::model::TestMethod;
pub use batch::{run_batch, BatchOptions};
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

pub fn to_ms(duration: Option<Duration>) -> i64 {
    duration.map_or(-1, |d| d.as_millis().max(1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

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
