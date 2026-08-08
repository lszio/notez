//! Space resolution helpers for the v0.1 web reader.
//!
//! The web reader lets users pick and switch spaces dynamically instead of
//! pinning a single `NOTEZ_SPACE_ROOT` at startup. This module wraps
//! [`ConfigPaths::discover`] + [`select_space`] behind a small, web-friendly
//! surface and a dedicated [`WebSpaceError`] type that the HTTP layer can
//! translate into a structured response (path missing, path is not a space,
//! etc.) without exposing TOML internals.
//!
//! The helpers are pure: callers pass an environment map and a `cwd` so
//! unit tests can drive them without touching the real process state.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::discovery::{select_space, ConfigPaths, SelectedSpace, SpaceSelector};
use crate::config::model::{ConfigError, GlobalConfig, SpaceRegistration};

/// Whether a space entry came from the explicit global registration or
/// from the on-disk auto-discovery scan. The picker uses this so
/// discovered (but un-registered) entries can be visually marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpaceSource {
    Registered,
    Discovered,
}

/// Lightweight description of a space entry, used to populate the
/// picker's dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSpace {
    pub name: String,
    pub path: PathBuf,
    pub source: SpaceSource,
}

/// Failure modes the web reader surfaces to the UI. Distinct from
/// [`ConfigError`] so callers do not need to pattern-match TOML details.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WebSpaceError {
    /// The supplied path does not exist on disk.
    #[error("space path does not exist: {0}")]
    NotFound(String),
    /// The path exists but contains neither `notez.toml` nor a
    /// `.notez/` directory, so it does not look like a Notez space.
    #[error("path is not a notez space (missing notez.toml and .notez/): {0}")]
    NotASpace(String),
    /// Some other discovery / IO failure (TOML parse error, unreadable
    /// file, missing global config, …).
    #[error("{0}")]
    Other(String),
}

impl From<ConfigError> for WebSpaceError {
    fn from(err: ConfigError) -> Self {
        // We deliberately do not leak TOML details to the HTTP layer.
        // `ConfigError` already produces user-readable messages so this
        // is a 1:1 string forwarding; the typed variant lets us layer
        // on more specific NotFound / NotASpace detection below.
        WebSpaceError::Other(err.to_string())
    }
}

/// Discover every space the picker should show, in priority order:
///
/// 1. Spaces explicitly registered in the global XDG config.
/// 2. Auto-discovered candidates under `$HOME` (one level deep) and
///    upward from `cwd` (up to [`MAX_UPWARD_DEPTH`] ancestor levels).
///    A discovered entry whose path matches a registered one is
///    coalesced into the registered entry — the explicit registration
///    wins on the conflict so the user keeps the name they chose.
///
/// Discovery can be turned off with `NOTEZ_DISABLE_AUTO_DISCOVER=1`,
/// which restricts the result to the XDG registrations. Missing or
/// unreadable global config is not an error: discovery still runs, the
/// picker just shows nothing from registration.
pub fn list_spaces(env: &BTreeMap<String, OsString>, cwd: &Path) -> Vec<RegisteredSpace> {
    let paths = ConfigPaths::discover(env, cwd).ok();
    let registered = paths
        .as_ref()
        .and_then(|p| p.global_config.as_ref())
        .map(|gc| {
            gc.spaces
                .iter()
                .map(|(name, reg)| RegisteredSpace {
                    name: name.clone(),
                    path: expand_tilde(&reg.path),
                    source: SpaceSource::Registered,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Track which absolute paths are already represented by a
    // registration so auto-discovery does not duplicate them.
    let registered_paths: std::collections::HashSet<PathBuf> = registered
        .iter()
        .map(|r| canonicalize_lossy(&r.path))
        .collect();

    let mut out = registered;
    if !auto_discover_enabled(env) {
        return dedup_preserve_order(out);
    }

    for (name, path) in discover_home_children(env) {
        let canon = canonicalize_lossy(&path);
        if registered_paths.contains(&canon) {
            continue;
        }
        out.push(RegisteredSpace {
            name,
            path,
            source: SpaceSource::Discovered,
        });
    }
    for (name, path) in discover_upward(cwd) {
        let canon = canonicalize_lossy(&path);
        if registered_paths.contains(&canon) {
            continue;
        }
        out.push(RegisteredSpace {
            name,
            path,
            source: SpaceSource::Discovered,
        });
    }
    dedup_preserve_order(out)
}

/// Cap on how many ancestor levels `discover_upward` will walk so a
/// discovery run from `/` cannot walk all the way to `/` again.
const MAX_UPWARD_DEPTH: usize = 5;

/// Cap on the number of children inside `$HOME` we'll inspect so a
/// huge home directory cannot block the picker.
const MAX_HOME_CHILDREN: usize = 200;

/// Whether the auto-discovery pass is enabled. Default on; set
/// `NOTEZ_DISABLE_AUTO_DISCOVER=1` to restrict the picker to explicit
/// global registrations.
fn auto_discover_enabled(env: &BTreeMap<String, OsString>) -> bool {
    match env.get("NOTEZ_DISABLE_AUTO_DISCOVER").map(|v| v.to_string_lossy().into_owned()) {
        Some(v) => !matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => true,
    }
}

/// Scan one level of `$HOME`'s children for notez-looking directories.
///
/// Hidden entries (those whose name starts with `.`) are skipped, as
/// are any that we lack read permission for. The result is sorted by
/// path so two runs on the same `$HOME` produce the same order.
fn discover_home_children(env: &BTreeMap<String, OsString>) -> Vec<(String, PathBuf)> {
    let Some(home) = env.get("HOME").map(PathBuf::from).or_else(|| {
        // Fall back to $XDG_CONFIG_HOME's parent when HOME is unset
        // (some container environments drop HOME but keep XDG).
        env.get("XDG_CONFIG_HOME")
            .map(|p| PathBuf::from(p).parent().map(|p| p.to_path_buf()))
            .flatten()
    }) else {
        return Vec::new();
    };
    if !home.is_dir() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&home) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_HOME_CHILDREN {
            break;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if !path.is_dir() || !looks_like_space_dir(&path) {
            continue;
        }
        out.push((name_for_space(&path, name), path));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Walk up to [`MAX_UPWARD_DEPTH`] ancestor levels from `cwd` and
/// collect every directory that already contains a `notez.toml` or a
/// `.notez/` child. Closer ancestors come first.
fn discover_upward(cwd: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut current = Some(cwd.to_path_buf());
    for _ in 0..MAX_UPWARD_DEPTH {
        let Some(dir) = current.take() else { break };
        if dir.is_dir() && looks_like_space_dir(&dir) {
            let fallback = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((name_for_space(&dir, &fallback), dir.clone()));
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    out
}

/// Best-effort canonical path for deduping. Falls back to the input
/// when canonicalization fails (non-existent path, permission error,
/// etc.) so we still have a stable key.
fn canonicalize_lossy(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Drop exact-duplicate `(name, path, source)` triples while keeping
/// the first occurrence (registered entries come before discovered).
fn dedup_preserve_order(mut v: Vec<RegisteredSpace>) -> Vec<RegisteredSpace> {
    let mut seen = std::collections::HashSet::new();
    v.retain(|r| {
        let key = (
            r.name.clone(),
            canonicalize_lossy(&r.path),
            r.source,
        );
        seen.insert(key)
    });
    v
}

/// Pick a friendly name for a discovered space.
///
/// Prefers `[space].name` from a sibling `notez.toml`; falls back to
/// the directory's leaf name when the toml is missing or malformed.
fn name_for_space(dir: &Path, fallback: &str) -> String {
    let toml_path = dir.join("notez.toml");
    if let Ok(text) = std::fs::read_to_string(&toml_path) {
        if let Ok(cfg) = crate::config::model::SpaceConfig::parse(&text) {
            return cfg.space.name;
        }
    }
    fallback.to_string()
}

/// Resolve a user-supplied path to a [`SelectedSpace`].
///
/// The path is checked in this order:
/// 1. Expand a leading `~` to `$HOME`.
/// 2. The path must exist on disk; otherwise [`WebSpaceError::NotFound`].
/// 3. If it is a directory, require `notez.toml` *or* a `.notez/` child
///    directory; otherwise [`WebSpaceError::NotASpace`]. Pointing at a
///    bare `notez.toml` file is also accepted (its parent is the root).
pub fn resolve_space(path: &Path) -> Result<SelectedSpace, WebSpaceError> {
    let expanded = expand_tilde(path);

    if !expanded.exists() {
        return Err(WebSpaceError::NotFound(expanded.display().to_string()));
    }

    // Distinguish "not a space" from generic discovery errors so the UI
    // can show a targeted hint ("this is just a folder, run `notez init`").
    if expanded.is_dir() && !looks_like_space_dir(&expanded) {
        return Err(WebSpaceError::NotASpace(expanded.display().to_string()));
    }

    // Delegate the actual `SelectedSpace` assembly to the existing
    // `select_space` machinery; it already handles toml/file/dir cases.
    // We need a ConfigPaths for the Default selector path but not for
    // explicit Path, so build a minimal one.
    let cwd = expanded
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = ConfigPaths::discover(&BTreeMap::new(), &cwd).unwrap_or_else(|_| ConfigPaths {
        global: PathBuf::new(),
        cwd,
        global_config: None,
        space_config: None,
    });

    select_space(&paths, SpaceSelector::Path(&expanded))
        .map_err(|e| match e {
            // select_space returns ConfigError::Invalid("space", "invalid space path …")
            // when the path does not exist or is not a toml/dir. Our
            // pre-check above already covered those cases for the
            // typical "user typed a path" flow, so anything reaching
            // here is a deeper discovery issue.
            ConfigError::Invalid(field, msg) if field == "space" => {
                WebSpaceError::NotASpace(msg)
            }
            other => WebSpaceError::Other(other.to_string()),
        })
}

fn looks_like_space_dir(dir: &Path) -> bool {
    dir.join("notez.toml").is_file() || dir.join(".notez").exists()
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if s == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    path.to_path_buf()
}

/// Re-export of [`SpaceConfig`] for downstream code that wants to inspect
/// the resolved space without taking a hard dependency on the model
/// module layout.
pub use crate::config::model::SpaceConfig as ResolvedSpaceConfig;
