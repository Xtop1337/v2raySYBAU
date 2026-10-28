//! Per-second traffic statistics: Xray stats API (via `xray api statsquery`) or sing-box Clash API.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::process::hide_window;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSample {
    /// Bytes per second through the proxy.
    pub up: u64,
    pub down: u64,
    /// Bytes per second sent directly (bypassed by routing).
    pub direct_up: u64,
    pub direct_down: u64,
    /// Proxy totals since connect.
    pub total_up: u64,
    pub total_down: u64,
}

pub enum TrafficMonitor {
    Xray { exe: PathBuf, api_port: u16 },
    SingBox { clash_port: u16 },
}

impl TrafficMonitor {
    /// Emits one sample per second until `cancel` fires.
    pub fn spawn(self, cancel: CancellationToken, sink: Arc<dyn Fn(TrafficSample) + Send + Sync>) {
        tokio::spawn(async move {
            match self {
                TrafficMonitor::Xray { exe, api_port } => run_xray(exe, api_port, cancel, sink).await,
                TrafficMonitor::SingBox { clash_port } => run_singbox(clash_port, cancel, sink).await,
            }
        });
    }
}

async fn run_xray(exe: PathBuf, api_port: u16, cancel: CancellationToken, sink: Arc<dyn Fn(TrafficSample) + Send + Sync>) {
    let mut totals = TrafficSample::default();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = interval.tick() => {}
        }
        let mut command = tokio::process::Command::new(&exe);
        command
            .args(["api", "statsquery", &format!("--server=127.0.0.1:{api_port}"), "-pattern", "outbound>>>", "-reset"])
            .kill_on_drop(true);
        hide_window(&mut command);
        let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(2), command.output()).await else {
            continue;
        };
        if let Ok(value) = serde_json::from_slice::<Value>(&output.stdout) {
            let mut sample = parse_xray_stats(&value);
            totals.total_up += sample.up;
            totals.total_down += sample.down;
            sample.total_up = totals.total_up;
            sample.total_down = totals.total_down;
            sink(sample);
        }
    }
}

/// Parses the `statsquery -reset` output (deltas since the previous call, values may be strings).
pub fn parse_xray_stats(value: &Value) -> TrafficSample {
    let mut sample = TrafficSample::default();
    for stat in value.get("stat").and_then(Value::as_array).into_iter().flatten() {
        let name = stat.get("name").and_then(Value::as_str).unwrap_or("");
        let amount = match stat.get("value") {
            Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
            Some(Value::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        };
        let parts: Vec<&str> = name.split(">>>").collect();
        if let [_, tag, _, direction] = parts.as_slice() {
            match (*tag, *direction) {
                ("proxy", "uplink") => sample.up += amount,
                ("proxy", "downlink") => sample.down += amount,
                ("direct", "uplink") => sample.direct_up += amount,
                ("direct", "downlink") => sample.direct_down += amount,
                _ => {}
            }
        }
    }
    sample
}

async fn run_singbox(port: u16, cancel: CancellationToken, sink: Arc<dyn Fn(TrafficSample) + Send + Sync>) {
    let client = reqwest::Client::builder().no_proxy().build().expect("http client");
    let mut totals = TrafficSample::default();
    while !cancel.is_cancelled() {
        let response = client.get(format!("http://127.0.0.1:{port}/traffic")).send().await;
        let Ok(response) = response else {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(1)) => continue,
            }
        };
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => return,
                chunk = stream.next() => chunk,
            };
            let Some(Ok(chunk)) = chunk else { break };
            buffer.extend_from_slice(&chunk);
            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=pos).collect();
                if let Ok(value) = serde_json::from_slice::<Value>(&line) {
                    let up = value.get("up").and_then(Value::as_u64).unwrap_or(0);
                    let down = value.get("down").and_then(Value::as_u64).unwrap_or(0);
                    totals.total_up += up;
                    totals.total_down += down;
                    sink(TrafficSample { up, down, total_up: totals.total_up, total_down: totals.total_down, ..Default::default() });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stats() {
        let value = serde_json::json!({"stat": [
            {"name": "outbound>>>proxy>>>traffic>>>uplink", "value": "100"},
            {"name": "outbound>>>proxy>>>traffic>>>downlink", "value": 2000},
            {"name": "outbound>>>direct>>>traffic>>>downlink", "value": "5"},
            {"name": "outbound>>>block>>>traffic>>>downlink"}
        ]});
        let s = parse_xray_stats(&value);
        assert_eq!((s.up, s.down, s.direct_down), (100, 2000, 5));
    }
}
