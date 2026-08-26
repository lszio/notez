//! WatchService — in-process filesystem watcher for space roots.
//!
//! The watcher is a thin wrapper around the `notify` crate. It tracks
//! one or more space roots; for each, it runs a background OS-level
//! file-system event channel and appends events to a bounded ring
//! buffer. The application facade is **not** invoked from the watcher
//! thread — the watcher records events only. Re-scanning after a
//! change is the caller's job (so a `notez scan` after `notez watch
//! start` is the intended workflow).
//!
//! Why no auto-scan? `rusqlite::Connection` is `!Send`, and the
//! facade wraps a `SqliteProjection` that holds a `Connection` with
//! a thread-local handle. Spawning a background scanner would force
//! us to lock the facade across `.await` points, which the existing
//! application code avoids by design. Keeping the watcher
//! read-only makes it cheap and safe to run for long stretches.
//!
//! # Lifecycle
//!
//! ```ignore
//! let svc = Arc::new(WatchService::new());
//! svc.start("/path/to/space")?;
//! // ... user mutates files ...
//! let recent = svc.events("/path/to/space", 50);
//! svc.stop("/path/to/space");
//! ```
//!
//! `WatchService` is intended to be wrapped in `Arc<WatchService>`
//! and shared from the HTTP layer. The `start` method takes
//! `&Arc<Self>` so the background `notify` callback thread can hold
//! a stable reference.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of events retained per watch. The UI shows the most
/// recent slice; anything older is discarded.
pub const EVENT_BUFFER: usize = 200;

/// Failure modes for starting / operating a watch.
#[derive(Debug, Error)]
pub enum WatchError {
    /// The path does not exist or is not a directory.
    #[error("watch path is not a directory: {0}")]
    NotADirectory(String),
    /// `notify` failed to attach to the path (permissions, inotify
    /// limit, etc.).
    #[error("failed to attach filesystem watcher: {0}")]
    Attach(String),
    /// A watch is already running for this path.
    #[error("watch already active for {0}")]
    AlreadyActive(String),
}

/// Coarse-grained event kind. We collapse `notify`'s
/// `CreateKind`/`ModifyKind`/`RemoveKind` variants into four buckets
/// the UI can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    Create,
    Modify,
    Remove,
    Rename,
}

impl WatchKind {
    fn from_notify(kind: &EventKind) -> Option<Self> {
        match kind {
            EventKind::Create(CreateKind::Any)
            | EventKind::Create(CreateKind::File)
            | EventKind::Create(CreateKind::Folder)
            | EventKind::Create(CreateKind::Other) => Some(Self::Create),
            EventKind::Modify(ModifyKind::Any)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Metadata(_))
            | EventKind::Modify(ModifyKind::Name(RenameMode::To))
            | EventKind::Modify(ModifyKind::Other) => Some(Self::Modify),
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Some(Self::Rename),
            EventKind::Remove(RemoveKind::Any)
            | EventKind::Remove(RemoveKind::File)
            | EventKind::Remove(RemoveKind::Folder)
            | EventKind::Remove(RemoveKind::Other) => Some(Self::Remove),
            _ => None,
        }
    }
}

/// One filesystem event observed by the watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    /// Monotonically increasing sequence number assigned when the
    /// event was recorded. Consumers use it to take deltas across
    /// polls without replaying the whole buffer.
    pub seq: u64,
    /// Wall-clock timestamp of when the event was observed.
    pub at: SystemTime,
    pub kind: WatchKind,
    /// Path that triggered the event. Absolute when `notify` reports
    /// an absolute path; relative to the watched root otherwise.
    pub path: PathBuf,
}

/// Snapshot of a watch's state — returned by `status()` and exposed
/// to the UI for the "watching / not watching" badge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchStatus {
    pub root: PathBuf,
    pub started_at: SystemTime,
    pub event_count: usize,
}

/// Internal handle for a single watch. Held in the `WatchService`
/// map; dropping it drops the `notify::RecommendedWatcher` which
/// detaches from the kernel.
struct WatchHandle {
    root: PathBuf,
    started_at: SystemTime,
    /// `notify` watcher; keep alive for as long as the watch is
    /// active. Dropping it stops the OS-level watch.
    _watcher: RecommendedWatcher,
    events: VecDeque<WatchEvent>,
}

impl WatchHandle {
    fn record(&mut self, ev: WatchEvent) {
        if self.events.len() >= EVENT_BUFFER {
            self.events.pop_front();
        }
        self.events.push_back(ev);
    }
}

/// The shared watcher. Cheap to construct, safe to share via `Arc`.
pub struct WatchService {
    handles: Mutex<HashMap<PathBuf, WatchHandle>>,
    /// Source of monotonic `WatchEvent::seq` values.
    next_seq: std::sync::atomic::AtomicU64,
}

impl WatchService {
    /// Construct an empty `WatchService`.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            handles: Mutex::new(HashMap::new()),
            next_seq: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Allocate the next event sequence number.
    fn take_seq(&self) -> u64 {
        self.next_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Start watching `source_root` recursively.
    ///
    /// Returns the wall-clock start time of the watch. Errors when
    /// the path is missing, not a directory, or a watch is already
    /// running for that path.
    pub fn start(self: &Arc<Self>, source_root: &Path) -> Result<SystemTime, WatchError> {
        let canonical = match std::fs::canonicalize(source_root) {
            Ok(p) => p,
            Err(_) => {
                return Err(WatchError::NotADirectory(source_root.display().to_string()));
            }
        };
        if !canonical.is_dir() {
            return Err(WatchError::NotADirectory(canonical.display().to_string()));
        }

        let mut handles = self.handles.lock().expect("watch handles poisoned");
        if handles.contains_key(&canonical) {
            return Err(WatchError::AlreadyActive(canonical.display().to_string()));
        }

        let started_at = SystemTime::now();
        let mut handle = WatchHandle {
            root: canonical.clone(),
            started_at,
            _watcher: build_watcher(self.clone())?,
            events: VecDeque::with_capacity(EVENT_BUFFER.min(64)),
        };
        handle
            ._watcher
            .watch(&canonical, RecursiveMode::Recursive)
            .map_err(|e| WatchError::Attach(e.to_string()))?;
        handles.insert(canonical.clone(), handle);

        Ok(started_at)
    }

    /// Stop watching a space. Returns `true` if a watch was running.
    pub fn stop(&self, source_root: &Path) -> bool {
        let key = match std::fs::canonicalize(source_root) {
            Ok(p) => p,
            Err(_) => source_root.to_path_buf(),
        };
        self.handles
            .lock()
            .expect("watch handles poisoned")
            .remove(&key)
            .is_some()
    }

    /// Status snapshot for a space. `None` if no watch is running.
    pub fn status(&self, source_root: &Path) -> Option<WatchStatus> {
        let key = self.lookup_key(source_root)?;
        let handles = self.handles.lock().expect("watch handles poisoned");
        let h = handles.get(&key)?;
        Some(WatchStatus {
            root: h.root.clone(),
            started_at: h.started_at,
            event_count: h.events.len(),
        })
    }

    /// The most recent `limit` events, oldest first. Returns an
    /// empty vec when no watch is running.
    pub fn events(&self, source_root: &Path, limit: usize) -> Vec<WatchEvent> {
        let Some(key) = self.lookup_key(source_root) else {
            return Vec::new();
        };
        let handles = self.handles.lock().expect("watch handles poisoned");
        let Some(h) = handles.get(&key) else {
            return Vec::new();
        };
        let n = h.events.len();
        let start = n.saturating_sub(limit);
        h.events.iter().skip(start).cloned().collect()
    }

    /// Number of active watches (used by CLI / MCP / web status).
    pub fn active_count(&self) -> usize {
        self.handles.lock().expect("watch handles poisoned").len()
    }

    /// Snapshot of every active watch.
    pub fn list(&self) -> Vec<WatchStatus> {
        self.handles
            .lock()
            .expect("watch handles poisoned")
            .values()
            .map(|h| WatchStatus {
                root: h.root.clone(),
                started_at: h.started_at,
                event_count: h.events.len(),
            })
            .collect()
    }

    /// Map a filesystem path back to the canonical key of an active
    /// watch. Walks up the path looking for a registered key.
    fn lookup_key(&self, path: &Path) -> Option<PathBuf> {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let handles = self.handles.lock().ok()?;
        for ancestor in canonical.ancestors() {
            if handles.contains_key(ancestor) {
                return Some(ancestor.to_path_buf());
            }
        }
        if handles.contains_key(&canonical) {
            return Some(canonical);
        }
        None
    }
}

/// `notify`'s callback handler signature is `FnMut(Event) + Send +
/// 'static`. We funnel events through a channel into the
/// `WatchService` rather than touching the mutex from the callback
/// thread; that keeps the lock duration short and decouples the
/// `notify` thread from the application runtime.
fn build_watcher(svc: Arc<WatchService>) -> Result<RecommendedWatcher, WatchError> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| WatchError::Attach(e.to_string()))?;

    // Drain events into the service.
    std::thread::Builder::new()
        .name("notez-watch".into())
        .spawn(move || {
            while let Ok(res) = rx.recv() {
                let Ok(event) = res else { continue };
                let Some(kind) = WatchKind::from_notify(&event.kind) else {
                    continue;
                };
                for p in event.paths {
                    let Some(canonical_key) = svc.lookup_key(&p) else {
                        continue;
                    };
                    let mut handles = match svc.handles.lock() {
                        Ok(h) => h,
                        Err(_) => return,
                    };
                    if let Some(h) = handles.get_mut(&canonical_key) {
                        h.record(WatchEvent {
                            seq: svc.take_seq(),
                            at: SystemTime::now(),
                            kind,
                            path: p,
                        });
                    }
                }
            }
        })
        .map_err(|e| WatchError::Attach(e.to_string()))?;

    Ok(watcher)
}

// ---- tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn start_rejects_nonexistent_path() {
        let svc = WatchService::new();
        let err = svc.start(Path::new("/nope/nope/nope")).unwrap_err();
        assert!(matches!(err, WatchError::NotADirectory(_)));
    }

    #[test]
    fn start_rejects_plain_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("not-a-dir.txt");
        fs::write(&file, "x").unwrap();
        let svc = WatchService::new();
        let err = svc.start(&file).unwrap_err();
        assert!(matches!(err, WatchError::NotADirectory(_)));
    }

    #[test]
    fn start_then_status_then_stop_roundtrip() {
        let dir = tempdir().unwrap();
        let svc = WatchService::new();
        let t0 = svc.start(dir.path()).expect("start");
        let status = svc.status(dir.path()).expect("status");
        assert_eq!(status.root, fs::canonicalize(dir.path()).unwrap());
        assert_eq!(status.started_at, t0);
        assert_eq!(status.event_count, 0);
        assert!(svc.stop(dir.path()));
        assert!(svc.status(dir.path()).is_none());
    }

    #[test]
    fn double_start_is_rejected() {
        let dir = tempdir().unwrap();
        let svc = WatchService::new();
        svc.start(dir.path()).unwrap();
        let err = svc.start(dir.path()).unwrap_err();
        assert!(matches!(err, WatchError::AlreadyActive(_)));
        svc.stop(dir.path());
    }

    #[test]
    fn stop_returns_false_for_unknown_path() {
        let svc = WatchService::new();
        assert!(!svc.stop(Path::new("/never/started")));
    }

    #[test]
    fn events_empty_before_any_mutation() {
        let dir = tempdir().unwrap();
        let svc = WatchService::new();
        svc.start(dir.path()).expect("start");
        let recent = svc.events(dir.path(), 1000);
        assert!(recent.is_empty());
        svc.stop(dir.path());
    }

    #[test]
    fn real_filesystem_events_get_recorded() {
        let dir = tempdir().unwrap();
        let svc = WatchService::new();
        svc.start(dir.path()).expect("start");

        // Give the kernel watcher a moment to attach.
        std::thread::sleep(Duration::from_millis(50));

        // Mutate the directory.
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        std::thread::sleep(Duration::from_millis(50));
        fs::remove_file(dir.path().join("a.txt")).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        // Give the background thread a moment to drain.
        std::thread::sleep(Duration::from_millis(150));
        let recent = svc.events(dir.path(), 50);
        // notify's event timing is OS-dependent; the test asserts
        // non-empty rather than exact contents so it doesn't flake
        // under heavy CI load.
        assert!(
            !recent.is_empty(),
            "expected at least one event from create+remove"
        );
        svc.stop(dir.path());
    }
}
