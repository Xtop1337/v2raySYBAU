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

/// How long a core gets to shut down by itself before it is killed.
const GRACEFUL_STOP: Duration = Duration::from_secs(4);

/// Cores started by this process (anything else running from the cores folder is a leftover).
fn own_pids() -> &'static std::sync::Mutex<std::collections::HashSet<u32>> {
    static PIDS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<u32>>> = std::sync::OnceLock::new();
    PIDS.get_or_init(Default::default)
}

pub(crate) fn is_own_pid(pid: u32) -> bool {
    own_pids().lock().unwrap().contains(&pid)
}

/// Asks a core to exit (Xray and sing-box both shut down cleanly on SIGINT/SIGTERM).
#[cfg(windows)]
pub(crate) fn request_stop(pid: u32) -> bool {
    use windows::Win32::System::Console::{AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, CTRL_BREAK_EVENT};
    // The cores run in their own hidden console: attach to it for a moment and send CTRL_BREAK
    // to the core's process group. Attaching is process-wide, so one core at a time.
    static CONSOLE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    if pid == 0 {
        return false;
    }
    let _guard = CONSOLE.lock().unwrap();
    unsafe {
        let _ = FreeConsole();
        if AttachConsole(pid).is_err() {
            return false;
        }
        let _ = SetConsoleCtrlHandler(None, true);
        let sent = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid).is_ok();
        let _ = FreeConsole();
        let _ = SetConsoleCtrlHandler(None, false);
        sent
    }
}

#[cfg(unix)]
pub(crate) fn request_stop(pid: u32) -> bool {
    pid != 0 && unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0
}

#[cfg(not(any(windows, unix)))]
pub(crate) fn request_stop(_pid: u32) -> bool {
    false
}

/// Every core is put in a job object that kills it when the app exits for any reason (crash,
/// installer closing the app, restart as administrator), so no core is left running on its
/// ports with an old server.
#[cfg(windows)]
mod job {
    use std::sync::OnceLock;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    fn job() -> Option<HANDLE> {
        static JOB: OnceLock<Option<isize>> = OnceLock::new();
        let handle = JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(None, windows::core::PCWSTR::null()).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(job.0 as isize)
        });
        handle.map(|h| HANDLE(h as *mut core::ffi::c_void))
    }

    pub fn assign(process: std::os::windows::io::RawHandle) {
        if let Some(job) = job() {
            let _ = unsafe { AssignProcessToJobObject(job, HANDLE(process.cast())) };
        }
    }
}

/// A running core whose stdout/stderr lines are forwarded to the event sink.
/// A supervisor task owns the child: it reports unexpected exits and performs kills.
pub struct CoreProcess {
    kind: CoreKind,
    pid: u32,
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
        #[cfg(windows)]
        {
            // Own process group, so a CTRL_BREAK can ask exactly this core to shut down.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        let mut child = command.spawn().with_context(|| format!("start {}", exe.display()))?;
        let pid = child.id().unwrap_or(0);
        #[cfg(windows)]
        if let Some(handle) = child.raw_handle() {
            job::assign(handle);
        }
        own_pids().lock().unwrap().insert(pid);
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
                    // A graceful stop lets the core remove its TUN adapter, routes and firewall
                    // rules; killed outright it leaves Windows to clean them up much later.
                    if !(request_stop(pid) && tokio::time::timeout(GRACEFUL_STOP, child.wait()).await.is_ok()) {
                        let _ = child.kill().await;
                    }
                    task_alive.store(false, Ordering::SeqCst);
                }
            }
            own_pids().lock().unwrap().remove(&pid);
            let _ = done_tx.send(());
        });
        Ok(CoreProcess { kind, pid, alive, kill_tx: Some(kill_tx), done_rx: Some(done_rx) })
    }

    pub fn pid(&self) -> u32 {
        self.pid
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
            let _ = tokio::time::timeout(GRACEFUL_STOP + Duration::from_secs(3), rx).await;
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
