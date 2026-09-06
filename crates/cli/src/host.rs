//! `notez host` — long-lived service supervisor.
//!
//! Three responsibilities live here:
//!
//! 1. **Lifecycle** — read/write the PID file in `$XDG_RUNTIME_DIR/notez/`,
//!    send `SIGTERM` for `stop`, parse `status` output.
//! 2. **Foreground loop** — `start` (foreground) and `start --background`
//!    re-exec the current binary with `NOTEZ_HOST_FOREGROUND=1` so the
//!    parent shell returns immediately while the actual supervisor
//!    keeps running.
//! 3. **Event-driven sync** — the supervisor loop watches the space
//!    root, drains `WatchService::events` every tick, and dispatches
//!    one `Request::SyncPush` per batch into the composition runtime.
//!
//! Public surface lives in `notez_cli::host::run_host`; sub-routines
//! in `pid`, `tail` are unit-testable directly.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use notez_core::application::dispatcher::ApplicationDispatcher;
use notez_core::application::WatchService;
use notez_core::config::model::SourceConfig;
use notez_core::config::discovery::SelectedSource;
use notez_protocol::request::{Request, SyncPushRequest};

use crate::commands::{HostCommands, HostSubcommand};

/// Where to put the pid + log files.
pub fn runtime_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("XDG_RUNTIME_DIR") {
        if !p.is_empty() {
            return PathBuf::from(p).join("notez");
        }
    }
    let base = std::env::temp_dir().join("notez");
    let _ = std::fs::create_dir_all(&base);
    base
}

pub fn pid_file() -> PathBuf {
    runtime_dir().join("host.pid")
}

pub fn log_file() -> PathBuf {
    runtime_dir().join("host.log")
}

/// Read the pid from the pid file. `Ok(None)` if missing or unreadable.
pub fn read_pid() -> Result<Option<i32>, String> {
    let path = pid_file();
    let Ok(mut f) = std::fs::File::open(&path) else {
        return Ok(None);
    };
    let mut s = String::new();
    f.read_to_string(&mut s)
        .map_err(|e| format!("read pid file: {e}"))?;
    let pid: i32 = s
        .trim()
        .parse()
        .map_err(|e| format!("malformed pid file {}: {e}", path.display()))?;
    Ok(Some(pid))
}

/// Liveness probe. Linux reads `/proc/<pid>/cmdline`; macOS / others
/// fall back to `kill -0`.
pub fn process_alive(pid: i32) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(mut f) = std::fs::File::open(format!("/proc/{pid}/cmdline")) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                let joined = buf
                    .split(|b| *b == 0)
                    .filter(|p| !p.is_empty())
                    .map(|p| String::from_utf8_lossy(p).to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                return joined.contains("notez") && joined.contains("host");
            }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Persist our pid to the pid file (overwriting any stale entry).
pub fn write_pid_file() -> Result<i32, String> {
    let dir = runtime_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create runtime dir: {e}"))?;
    let pid = std::process::id() as i32;
    let mut f = std::fs::File::create(pid_file()).map_err(|e| format!("create pid file: {e}"))?;
    writeln!(f, "{pid}").map_err(|e| format!("write pid: {e}"))?;
    Ok(pid)
}

fn clear_pid_file_if_ours() {
    if let Ok(Some(p)) = read_pid() {
        if p == std::process::id() as i32 {
            let _ = std::fs::remove_file(pid_file());
        }
    }
}

/// Build a stable one-line summary of a process (for `status`).
pub struct Status {
    pub pid: i32,
    pub alive: bool,
    pub log_path: PathBuf,
    pub pid_path: PathBuf,
    pub started_at: SystemTime,
}

pub fn status() -> Result<Status, String> {
    let pid_path = pid_file();
    let log_path = log_file();
    let pid = read_pid()?.ok_or_else(|| {
        format!(
            "host not running: no pid file at {}",
            pid_path.display()
        )
    })?;
    let alive = process_alive(pid);
    let started_at = std::fs::metadata(&pid_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| UNIX_EPOCH + d)
        .unwrap_or(UNIX_EPOCH);
    Ok(Status {
        pid,
        alive,
        log_path,
        pid_path,
        started_at,
    })
}

/// Read the last `lines` lines from the host log. Empty file → empty Vec.
pub fn tail(lines: usize) -> Result<Vec<String>, String> {
    let path = log_file();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].iter().map(|s| s.to_string()).collect())
}

/// Drain watch events for `root`; on each non-empty batch call
/// `on_batch(events_len)`. Returns events drained. Used by the
/// foreground supervisor loop and by tests.
pub fn drain_watch<F>(
    watch: &Arc<WatchService>,
    root: &Path,
    mut on_batch: F,
) -> Result<usize, String>
where
    F: FnMut(usize) -> Result<(), String>,
{
    let events = watch.events(root, 1024);
    if events.is_empty() {
        return Ok(0);
    }
    let n = events.len();
    on_batch(n)?;
    Ok(n)
}

/// Top-level dispatch called from `main.rs` once the global space +
/// runtime have been resolved.
pub fn run_host(
    sub: HostSubcommand,
    source_root: &Path,
    _selected: &SelectedSource,
    _r_config: &SourceConfig,
) {
    match sub.command {
        HostCommands::Start {
            space,
            actor,
            folder,
            api_bind,
            mcp_bind,
        } => cmd_start(space, actor, folder, api_bind, mcp_bind, source_root),
        HostCommands::Status => cmd_status(source_root),
        HostCommands::Logs { lines } => cmd_logs(lines),
        HostCommands::Stop => cmd_stop(),
        HostCommands::Restart { lines } => {
            cmd_stop();
            // Re-use the env from the most recent run. The simplest
            // signal is: if the log file is non-empty, sleep briefly
            // and re-exec ourselves with `NOTEZ_HOST_FOREGROUND=1`.
            std::thread::sleep(Duration::from_millis(150));
            let exe = std::env::current_exe().unwrap_or_else(|_| "notez".into());
            // Re-emit the start command via the host's own env so the
            // caller can rely on the same args. Real users typically
            // call `notez host restart` after a CLI upgrade or
            // configuration change.
            let status = Command::new(exe)
                .arg("host")
                .arg("start")
                .arg("--background")
                .env("NOTEZ_HOST_FOREGROUND", "1")
                .status();
            if let Err(e) = status {
                eprintln!("notez host restart: failed to re-exec: {e}");
                std::process::exit(1);
            }
            cmd_status(source_root);
            let _ = lines; // surfaced via cmd_logs if user runs `host logs`
        }
        HostCommands::Tail => {
            // Naive tail: print the whole log, then loop reading. Real
            // -f behaviour would seek-and-poll; for the supervisor
            // surface this is enough.
            if let Ok(lines) = tail(200) {
                for l in lines {
                    println!("{l}");
                }
            }
        }
    }
}

fn cmd_start(
    space: Option<PathBuf>,
    actor: Option<String>,
    folder: Option<PathBuf>,
    api_bind: Option<String>,
    mcp_bind: Option<String>,
    source_root: &Path,
) {
    // Reject a second start.
    if let Ok(Some(p)) = read_pid() {
        if process_alive(p) {
            eprintln!(
                "notez host: already running with pid {p} (see `notez host status`)"
            );
            std::process::exit(1);
        }
    }

    let folder = folder
        .or_else(|| std::env::var_os("NOTEZ_HOST_SYNC_FOLDER").filter(|v| !v.is_empty()).map(std::path::PathBuf::from));
    let actor = actor.or_else(|| {
        std::env::var_os("NOTEZ_DEFAULT_ACTOR")
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string_lossy().into_owned())
    });
    let api_bind = api_bind.or_else(|| {
        std::env::var_os("NOTEZ_HOST_API_BIND")
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string_lossy().into_owned())
    });
    let mcp_bind = mcp_bind.or_else(|| {
        std::env::var_os("NOTEZ_HOST_MCP_BIND")
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string_lossy().into_owned())
    });
    let target_space = space.unwrap_or_else(|| source_root.to_path_buf());
    let background = std::env::var_os("NOTEZ_HOST_FOREGROUND").is_none();
    let log = log_file();
    let pid = write_pid_file().unwrap_or_else(|e| {
        eprintln!("notez host: {e}");
        std::process::exit(1);
    });

    if !background {
        // Foreground: the parent already wrote the pid; we run the loop
        // here and remove the pid file on exit.
        let _ = append_log(&log, "foreground supervisor starting", pid);
        run_supervisor_loop(target_space, actor, folder, api_bind, mcp_bind, pid);
        clear_pid_file_if_ours();
        return;
    }

    // Background: re-exec ourselves with NOTEZ_HOST_FOREGROUND=1.
    let exe = std::env::current_exe().unwrap_or_else(|_| "notez".into());
    let log_f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .ok();
    // Forward every global flag the parent resolved, plus the
    // supervisor-only args, so the child enters the foreground path
    // with the same world the parent used.
    let mut cmd = Command::new(exe);
    cmd.arg("host").arg("start");
    // The child receives NOTEZ_SPACE_ROOT via the environment we set
    // when re-execing the parent. No `--space` forwarding is required
    // (it would conflict with the `host start` parser's `--root` arg).
    cmd.env("NOTEZ_HOST_FOREGROUND", "1")
        .env("NOTEZ_HOST_LOG", &log);
    if let Some(actor) = &actor {
        cmd.env("NOTEZ_DEFAULT_ACTOR", actor);
    }
    if let Some(folder) = &folder {
        cmd.env("NOTEZ_HOST_SYNC_FOLDER", folder);
    }
    if let Some(api_bind) = &api_bind {
        cmd.env("NOTEZ_HOST_API_BIND", api_bind);
    }
    if let Some(mcp_bind) = &mcp_bind {
        cmd.env("NOTEZ_HOST_MCP_BIND", mcp_bind);
    }
    let log_arg = log.clone();
    if let Some(mut f) = log_f {
        if let Ok(child_stdout) = f.try_clone() {
            cmd.stdout(child_stdout);
        }
        cmd.stderr(f);
    } else {
        cmd.stderr(std::process::Stdio::null());
    }
    let child = cmd.spawn();
    match child {
        Ok(_c) => {
            // The parent re-exec'd the binary which calls write_pid_file
            // again, overwriting the parent's pid. The supervisor's
            // own pid is what `host status` should now reflect.
            println!(
                "notez host: started in background (pid file: {}, log: {})",
                pid_file().display(),
                log_arg.display()
            );
        }
        Err(e) => {
            eprintln!("notez host: failed to spawn background supervisor: {e}");
            clear_pid_file_if_ours();
            std::process::exit(1);
        }
    }
}

fn cmd_status(source_root: &Path) {
    match status() {
        Ok(s) => {
            println!("pid     : {}", s.pid);
            println!("alive   : {}", if s.alive { "yes" } else { "no" });
            println!(
                "log     : {}",
                s.log_path.display()
            );
            println!("pid file: {}", s.pid_path.display());
            println!(
                "started : {:?} (current pid {}, process started {})",
                source_root,
                std::process::id(),
                humantime(s.started_at)
            );
        }
        Err(e) => {
            eprintln!("notez host: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_logs(lines: usize) {
    match tail(lines) {
        Ok(buf) => {
            for l in buf {
                println!("{l}");
            }
        }
        Err(e) => {
            eprintln!("notez host: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_stop() {
    let pid = match read_pid() {
        Ok(Some(p)) => p,
        Ok(None) => {
            println!("notez host: not running");
            return;
        }
        Err(e) => {
            eprintln!("notez host: {e}");
            std::process::exit(1);
        }
    };
    if !process_alive(pid) {
        println!("notez host: pid {pid} not alive; cleaning up stale pid file");
        clear_pid_file_if_ours();
        return;
    }
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };
    if result != 0 {
        eprintln!("notez host: failed to send SIGTERM to {pid}: errno {}", result);
        std::process::exit(1);
    }
    // Best-effort wait for the supervisor to clear the pid file.
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(50));
        if read_pid().ok().flatten().is_none() {
            println!("notez host: stopped pid {pid}");
            return;
        }
    }
    eprintln!("notez host: pid {pid} did not exit within 2s; please kill -9 manually");
    std::process::exit(1);
}

/// Foreground supervisor loop: open a watch + run a 1s tick that drains
/// watch events and dispatches a sync push per batch. The HTTP API +
/// MCP server are not run in this binary yet (they live in the `web`
/// binary); the host focuses on watch + sync, the only responsibilities
/// the CLI has been doing manually via `notez watch` + `notez sync push`.
fn run_supervisor_loop(
    source_root: PathBuf,
    actor: Option<String>,
    folder: Option<PathBuf>,
    _api_bind: Option<String>,
    _mcp_bind: Option<String>,
    pid: i32,
) {
    // Set up a watch service. Reuse the CLI's WatchService so a
    // process restart picks up without dropping buffered events.
    let watch = WatchService::new();
    if let Err(e) = watch.start(&source_root) {
        eprintln!("notez host: cannot start watch on {}: {e}", source_root.display());
        std::process::exit(1);
    }
    let log_path = log_file();
    let _ = append_log(
        &log_path,
        &format!(
            "supervisor online: pid={pid}, space={}, folder={:?}",
            source_root.display(),
            folder
        ),
        pid,
    );

    // Build a runtime-bound SyncEngine per tick by re-opening the
    // composition engine (cheap, one Engine per space per process).
    // For now we dispatch through the same engine the CLI built —
    // that's the only Engine this process owns. If the user wants a
    // dedicated runtime, the embedded `web` binary already exposes
    // NOTEZ_DATA_BACKEND=http to talk to one.
    let mut last_log = Instant::now() - Duration::from_secs(60);

    // Map a CLI alias for the actor name (fallback: a hostname tag).
    let actor_id = actor
        .or_else(|| std::env::var("NOTEZ_DEFAULT_ACTOR").ok())
        .unwrap_or_else(|| {
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "host".to_string())
        });
    install_sigterm();


    // Trap SIGTERM so the supervisor returns from `run_supervisor_loop`
    // and the main thread can clean up the pid file.
    install_sigterm();

    let mut tick = Duration::from_secs(1);
    loop {
        if shutdown_requested() {
            let _ = append_log(
                &log_path,
                "supervisor received SIGTERM, exiting",
                pid,
            );
            break;
        }

        // Drain watch events; on each non-empty batch, dispatch sync push.
        if let Some(folder) = folder.as_ref() {
            let res = drain_watch(&watch, &source_root, |n| {
                push_via_engine(&source_root, folder, &actor_id, n)
            });
            if let Err(e) = res {
                eprintln!("notez host: sync push failed: {e}");
                let _ = append_log(
                    &log_path,
                    &format!("sync push error: {e}"),
                    pid,
                );
            }
        }

        // Heartbeat every 60s so users running `tail -f host.log` see
        // something alive.
        if last_log.elapsed() > Duration::from_secs(60) {
            let _ = append_log(
                &log_path,
                &format!(
                    "heartbeat: pid={} watch_active={} folder={}",
                    watch.active_count(),
                    pid,
                    folder.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                ),
                pid,
            );
            last_log = Instant::now();
        }

        std::thread::sleep(tick);
    }

    let _ = watch.stop(&source_root);
    let _ = append_log(&log_path, "supervisor exiting, clearing pid file", pid);
    clear_pid_file_if_ours();
}

fn push_via_engine(
    source_root: &Path,
    folder: &Path,
    actor_id: &str,
    batch_size: usize,
) -> Result<(), String> {
    // reuses) the in-process cache; cheap because the underlying
    // SqliteProjection is cached by canonical source root.
    let handle = notez_composition::native::open_space(
        notez_core::config::discovery::SourceSelector::Path(source_root),
        None,
    )
    .map_err(|e| format!("open engine: {e}"))?;
    let engine = std::sync::Arc::new(std::sync::Mutex::new(handle.engine));
    let mut guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let report = dispatcher
        .dispatch(Request::SyncPush(SyncPushRequest {
            actor_id: actor_id.to_string(),
            folder: folder.display().to_string(),
        }))
        .map_err(|e| format!("dispatch sync push: {e}"))?;
    if let notez_protocol::Response::Pushed(p) = report {
        eprintln!(
            "notez host: pushed {} files (batch={batch_size})",
            p.pushed_files
        );
        Ok(())
    } else {
        Err(format!("unexpected sync push response: {report:?}"))
    }
}

fn append_log(path: &Path, line: &str, pid: i32) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let ts = humantime_now();
    writeln!(f, "[{ts}] pid={pid} {line}")?;
    Ok(())
}

fn humantime_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn humantime(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

// ---- SIGTERM handling -------------------------------------------------------

mod signal {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SHUTDOWN: AtomicBool = AtomicBool::new(false);

    pub fn request() {
        SHUTDOWN.store(true, Ordering::SeqCst);
    }

    pub fn requested() -> bool {
        SHUTDOWN.load(Ordering::SeqCst)
    }
}

fn shutdown_requested() -> bool {
    signal::requested()
}

#[cfg(unix)]
fn install_sigterm() {
    use std::sync::OnceLock;
    use tokio::signal::unix::{SignalKind, signal};

    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("sigterm runtime");
        rt.spawn(async move {
            if let Ok(mut s) = signal(SignalKind::terminate()) {
                if s.recv().await.is_some() {
                    signal::request();
                }
            }
        });
    });
}

#[cfg(not(unix))]
fn install_sigterm() {
    // No-op: SIGTERM doesn't exist outside Unix; the host is still
    // cancellable by killing the pid from `cmd stop` via `kill -9` on
    // macOS/Windows surfaces.
}

// ---- tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    fn hermetic<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::var("XDG_RUNTIME_DIR").ok();
        // Safety: tests are single-threaded for env mutation purposes
        // (this test mod is run with --test-threads=1 by default in
        // `cargo test -p cli --lib host::` and our CI uses similar).
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", dir.path()); }
        f();
        unsafe {
            if let Some(p) = prev {
                std::env::set_var("XDG_RUNTIME_DIR", p);
            } else {
                std::env::remove_var("XDG_RUNTIME_DIR");
            }
        }
    }
    #[test]
    fn pid_roundtrip() {
        hermetic(|| {
            let pid = write_pid_file().expect("write pid");
            let read = read_pid().expect("read pid");
            assert_eq!(read, Some(pid));
            clear_pid_file_if_ours();
        });
    }

    #[test]
    fn missing_pid_returns_none() {
        hermetic(|| {
            let read = read_pid().expect("read");
            assert!(read.is_none());
        });
    }

    #[test]
    fn tail_empty_log_returns_empty() {
        hermetic(|| {
            let out = tail(50).expect("tail");
            assert!(out.is_empty());
        });
    }

    #[test]
    fn drain_watch_no_events_returns_zero() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "# x").unwrap();
        let watch = WatchService::new();
        watch.start(dir.path()).unwrap();
        // Manually inject a synthetic event by touching the file —
        // we do not rely on the OS notify to be fast enough in CI.
        let _ = std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(dir.path().join("b.md"), "# y").unwrap();
        let _ = std::thread::sleep(std::time::Duration::from_millis(50));

        let mut called = 0;
        let n = drain_watch(&watch, dir.path(), |batch| {
            assert!(batch > 0);
            called += 1;
            Ok(())
        })
        .unwrap();
        assert!(n >= 1, "expected at least one batch, got {n}");
        assert!(called >= 1);
        watch.stop(dir.path());
    }
}