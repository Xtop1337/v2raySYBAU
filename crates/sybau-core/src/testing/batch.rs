//! Batch latency testing of many servers, as in v2rayNG "Test all real delay".

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::stream::{self, StreamExt};
use tokio_util::sync::CancellationToken;

use super::{http_delay, icmp_ping, tcp_ping, to_ms, DelayResult};
use crate::config::{singbox, xray};
use crate::model::{Profile, TestMethod};
use crate::runtime::{free_ports, wait_for_port, write_config, CoreKind, CorePaths, CoreProcess, EventSink};
use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub method: TestMethod,
    pub url: String,
    pub timeout: Duration,
    pub concurrency: usize,
    /// Profiles per temporary core instance.
    pub chunk: usize,
}

impl BatchOptions {
    pub fn from_settings(settings: &Settings, method: TestMethod) -> Self {
        BatchOptions {
            method,
            url: settings.test.url.clone(),
            timeout: Duration::from_millis(settings.test.timeout_ms.max(500)),
            concurrency: settings.test.concurrency.clamp(1, 128),
            chunk: 64,
        }
    }
}

pub type ResultSink = Arc<dyn Fn(DelayResult) + Send + Sync>;

pub async fn run_batch(
    profiles: Vec<Profile>,
    settings: &Settings,
    paths: &CorePaths,
    options: &BatchOptions,
    cancel: CancellationToken,
    sink: ResultSink,
) -> Result<()> {
    match options.method {
        TestMethod::Tcp | TestMethod::Icmp => {
            let method = options.method;
            let timeout = options.timeout;
            stream::iter(profiles)
                .map(|p| {
                    let cancel = cancel.clone();
                    async move {
                        if cancel.is_cancelled() {
                            return None;
                        }
                        let rtt = match method {
                            TestMethod::Icmp => icmp_ping(&p.address, timeout).await,
                            _ => tcp_ping(&p.address, p.port, timeout).await,
                        };
                        Some(DelayResult { profile_id: p.id, ms: to_ms(rtt), method })
                    }
                })
                .buffer_unordered(options.concurrency)
                .for_each(|r| {
                    if let Some(r) = r {
                        sink(r);
                    }
                    async {}
                })
                .await;
            Ok(())
        }
        TestMethod::Real => {
            let (native, xray_profiles): (Vec<_>, Vec<_>) = profiles.into_iter().partition(|p| p.protocol.needs_singbox());
            for (kind, list) in [(CoreKind::Xray, xray_profiles), (CoreKind::SingBox, native)] {
                if list.is_empty() {
                    continue;
                }
                paths.ensure(kind)?;
                for chunk in list.chunks(options.chunk.max(1)) {
                    if cancel.is_cancelled() {
                        return Ok(());
                    }
                    test_chunk(kind, chunk.to_vec(), settings.clone(), paths.clone(), options.clone(), cancel.clone(), sink.clone()).await;
                }
            }
            Ok(())
        }
    }
}

/// Starts one temporary core for the chunk. When the core rejects the config (one broken
/// profile breaks the whole file) the chunk is bisected until the bad profiles are isolated.
fn test_chunk(
    kind: CoreKind,
    chunk: Vec<Profile>,
    settings: Settings,
    paths: CorePaths,
    options: BatchOptions,
    cancel: CancellationToken,
    sink: ResultSink,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let Ok(ports) = free_ports(chunk.len()) else {
            report_failed(&chunk, &sink);
            return;
        };
        let entries: Vec<(&Profile, u16)> = chunk.iter().zip(ports.iter().copied()).collect();
        let config = match kind {
            CoreKind::Xray => xray::build_batch(&entries, &settings),
            CoreKind::SingBox => singbox::build_batch(&entries, &settings),
        };
        let file = paths.config_dir.join(format!("batch-{}-{}.json", kind.label(), uuid::Uuid::new_v4().simple()));
        if write_config(&file, &config).is_err() {
            report_failed(&chunk, &sink);
            return;
        }
        let quiet: EventSink = Arc::new(|_| {});
        let args = vec!["run".to_string(), "-c".to_string(), file.display().to_string()];
        let process = CoreProcess::spawn(kind, &paths.exe(kind), &args, &paths.dir, quiet);
        let Ok(mut process) = process else {
            let _ = std::fs::remove_file(&file);
            report_failed(&chunk, &sink);
            return;
        };
        let last_port = *ports.last().unwrap();
        let ready = wait_for_port(last_port, Duration::from_secs(6), || process.is_alive()).await;
        if !ready {
            process.kill().await;
            let _ = std::fs::remove_file(&file);
            if chunk.len() == 1 {
                report_failed(&chunk, &sink);
            } else {
                let (left, right) = chunk.split_at(chunk.len() / 2);
                test_chunk(kind, left.to_vec(), settings.clone(), paths.clone(), options.clone(), cancel.clone(), sink.clone()).await;
                test_chunk(kind, right.to_vec(), settings, paths, options, cancel, sink).await;
            }
            return;
        }

        let targets: Vec<(uuid::Uuid, u16)> = chunk.iter().map(|p| p.id).zip(ports.iter().copied()).collect();
        probe_ports(targets, options.url.clone(), options.timeout, options.concurrency, cancel.clone(), sink.clone()).await;
        process.kill().await;
        let _ = std::fs::remove_file(&file);
    })
}

/// Measures real delay through each local port; owned inputs keep the future `Send + 'static`.
async fn probe_ports(
    targets: Vec<(uuid::Uuid, u16)>,
    url: String,
    timeout: Duration,
    concurrency: usize,
    cancel: CancellationToken,
    sink: ResultSink,
) {
    let mut stream = stream::iter(targets)
        .map(move |(id, port)| {
            let url = url.clone();
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return None;
                }
                let rtt = http_delay(port, &url, timeout).await;
                Some(DelayResult { profile_id: id, ms: to_ms(rtt), method: TestMethod::Real })
            }
        })
        .buffer_unordered(concurrency);
    while let Some(result) = stream.next().await {
        if let Some(result) = result {
            sink(result);
        }
    }
}

fn report_failed(chunk: &[Profile], sink: &ResultSink) {
    for p in chunk {
        sink(DelayResult { profile_id: p.id, ms: -1, method: TestMethod::Real });
    }
}
