use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use super::{CoreEvent, CoreKind, EventSink};
use crate::logs::parse_core_line;

/// A running core whose stdout/stderr lines are forwarded to the event sink.
/// A supervisor task owns the child: it reports unexpected exits and performs kills.
pub struct CoreProcess {
    kind: CoreKind,
    alive: Arc<AtomicBool>,
    kill_tx: Option<oneshot::Sender<()>>,
    done_rx: Option<oneshot::Receiver<()>>,
}

pub fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

impl CoreProcess {
    pub fn spawn(kind: CoreKind, exe: &Path, args: &[String], workdir: &Path, sink: EventSink) -> Result<Self> {
        let mut command = Command::new(exe);
        command
            .args(args)
            .current_dir(workdir)
            .env("XRAY_LOCATION_ASSET", workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        hide_window(&mut command);
        let mut child = command.spawn().with_context(|| format!("start {}", exe.display()))?;
        if let Some(out) = child.stdout.take() {
            forward_lines(out, sink.clone());
        }
        if let Some(err) = child.stderr.take() {
            forward_lines(err, sink.clone());
        }
        tracing::info!("{} started (pid {:?})", kind.label(), child.id());

        let alive = Arc::new(AtomicBool::new(true));
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let task_alive = alive.clone();
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    task_alive.store(false, Ordering::SeqCst);
                    sink(CoreEvent::Exited { kind, code: status.ok().and_then(|s| s.code()) });
                }
                _ = kill_rx => {
                    let _ = child.kill().await;
                    task_alive.store(false, Ordering::SeqCst);
                }
            }
            let _ = done_tx.send(());
        });
        Ok(CoreProcess { kind, alive, kill_tx: Some(kill_tx), done_rx: Some(done_rx) })
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn kind(&self) -> CoreKind {
        self.kind
    }

    pub async fn kill(&mut self) {
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
        if let Some(rx) = self.done_rx.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), rx).await;
        }
        tracing::info!("{} stopped", self.kind.label());
    }
}

impl Drop for CoreProcess {
    fn drop(&mut self) {
        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
    }
}

fn forward_lines<R: AsyncRead + Unpin + Send + 'static>(reader: R, sink: EventSink) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(entry) = parse_core_line(&line) {
                sink(CoreEvent::Log(entry));
            }
        }
    });
}

/// Waits until something listens on `127.0.0.1:port`, giving up early when `alive` turns false.
pub async fn wait_for_port(port: u16, timeout: Duration, alive: impl Fn() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        if !alive() {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Reserves `count` currently free local TCP ports.
pub fn free_ports(count: usize) -> std::io::Result<Vec<u16>> {
    let listeners = (0..count).map(|_| std::net::TcpListener::bind("127.0.0.1:0")).collect::<std::io::Result<Vec<_>>>()?;
    listeners.iter().map(|l| l.local_addr().map(|a| a.port())).collect()
}
