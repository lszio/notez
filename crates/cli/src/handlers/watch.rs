//! Handler for `Commands::Watch`, including the SIGINT plumbing used
//! by the long-running start loop.

use crate::commands;

/// Run the `notez watch` subcommand.
pub fn run_watch(args: commands::WatchArgs, source_root: &std::path::Path) {
    use notez_core::application::{WatchError, WatchService};
    let svc = WatchService::new();
    match args.command {
        None | Some(commands::WatchCommands::Start) => {
            match svc.start(source_root) {
                Ok(_) => {}
                Err(WatchError::AlreadyActive(_)) => {
                    eprintln!(
                        "watch already active for {}; following existing events.",
                        source_root.display()
                    );
                }
                Err(e) => {
                    eprintln!("failed to start watch: {e}");
                    std::process::exit(5);
                }
            }
            let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            install_sigint_handler(running.clone());
            // Delta cursor: only print events with a sequence number
            // newer than the last one we printed, so ring-buffer
            // eviction never replays or swallows events.
            let mut last_seq: u64 = 0;
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let events = svc.events(source_root, 200);
                for ev in events.iter().filter(|e| e.seq > last_seq) {
                    let secs = ev
                        .at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    println!("[#{} {}] {:?} {}", ev.seq, secs, ev.kind, ev.path.display());
                }
                if let Some(max) = events.iter().map(|e| e.seq).max() {
                    last_seq = max;
                }
            }
            svc.stop(source_root);
            eprintln!("watch stopped.");
        }
        Some(commands::WatchCommands::Stop) => {
            if svc.stop(source_root) {
                println!("watch stopped.");
            } else {
                eprintln!("no watch was active for {}", source_root.display());
            }
        }
        Some(commands::WatchCommands::Status { limit }) => {
            let status = svc.status(source_root);
            let events = svc.events(source_root, limit);
            match status {
                None => println!("no watch is active for {}", source_root.display()),
                Some(s) => println!(
                    "watching {} since epoch+{}s ({} events buffered)",
                    s.root.display(),
                    s.started_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    s.event_count
                ),
            }
            for ev in events {
                let secs = ev
                    .at
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                println!("[{}] {:?} {}", secs, ev.kind, ev.path.display());
            }
        }
    }
}

#[cfg(unix)]
static mut SIGINT_FLAG: *mut std::sync::atomic::AtomicBool = std::ptr::null_mut();

/// Register a SIGINT handler that flips the running flag. Uses
/// `libc::signal` directly to keep the CLI lean. The flag is held
/// as a raw pointer; the caller is responsible for the
/// `Arc<AtomicBool>` outliving the process.
#[cfg(unix)]
fn install_sigint_handler(running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::Ordering;
    unsafe {
        SIGINT_FLAG = std::sync::Arc::into_raw(running) as *mut std::sync::atomic::AtomicBool;
        libc::signal(libc::SIGINT, sigint_trampoline as *const () as libc::sighandler_t);
    }
    // Suppress unused import warning when this fn is dead-code-eliminated
    // by feature gates.
    let _ = Ordering::SeqCst;
}

#[cfg(unix)]
extern "C" fn sigint_trampoline(_sig: libc::c_int) {
    unsafe {
        if !SIGINT_FLAG.is_null() {
            (*SIGINT_FLAG).store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(not(unix))]
fn install_sigint_handler(_running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    // No-op: Windows users should send Ctrl-Break.
}
