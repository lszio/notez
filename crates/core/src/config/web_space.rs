//! Source resolution helpers for the v0.1 web reader.
//!
//! The web reader lets users pick and switch sources dynamically instead of
//! pinning a single `NOTEZ_SOURCE_ROOT` at startup. This module wraps the
//! canonical discovery/selection machinery so the route handlers stay thin.

use std::path::{Path, PathBuf};

use crate::config::discovery::{ConfigPaths, SelectedSource, SourceSelector, select_source};
use crate::config::model::{ConfigError, SourceRegistration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSourceError {
    /// The requested source path does not exist.
    NotFound(String),
    /// The path exists but is neither a directory nor a notez.toml.
    NotASource(String),
    /// The requested source path is malformed.
    InvalidPath(String),
    /// The global config is malformed.
    GlobalConfig(String),
    /// The source resolved but the notez.toml is invalid or absent.
    Source(String),
}

impl std::fmt::Display for WebSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSourceError::NotFound(p) => write!(f, "source not found: {p}"),
            WebSourceError::NotASource(p) => write!(f, "not a source: {p}"),
            WebSourceError::InvalidPath(s) => write!(f, "invalid source path: {s}"),
            WebSourceError::GlobalConfig(s) => write!(f, "global config error: {s}"),
            WebSourceError::Source(s) => write!(f, "source error: {s}"),
        }
    }
}

impl std::error::Error for WebSourceError {}

/// Whether a source entry came from the explicit global registration or from
/// the on-disk auto-discovery scan. The picker uses this so the URL surface
/// distinguishes "registered" sources from "ephemeral" ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    Registered(SourceRegistration),
    Discovered,
    Ephemeral,
}

/// List all sources the picker can present: the global registrations first,
/// then any on-disk sources discovered upward from `cwd`.
///
/// The web reader displays them as one flat list ordered alphabetically by
/// display name so users can scan quickly.
pub fn list_sources(
    env: &std::collections::BTreeMap<String, std::ffi::OsString>,
    cwd: &Path,
) -> Result<Vec<ListedSource>, WebSourceError> {
    let paths =
        ConfigPaths::discover(env, cwd).map_err(|e| WebSourceError::GlobalConfig(e.to_string()))?;
    let mut out: Vec<ListedSource> = Vec::new();
    if let Some(global) = &paths.global_config {
        for (name, reg) in &global.sources {
            out.push(ListedSource {
                name: name.clone(),
                root: reg.path.clone(),
                origin: SourceOrigin::Registered(reg.clone()),
            });
        }
    }
    if let Some((config_path, _cfg)) = &paths.source_config {
        if let Some(name) = source_name_from_toml(config_path) {
            if !out.iter().any(|s| s.name == name) {
                out.push(ListedSource {
                    name,
                    root: config_path.parent().unwrap().to_path_buf(),
                    origin: SourceOrigin::Discovered,
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
fn source_name_from_toml(toml_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(toml_path).ok()?;
    let cfg = crate::config::model::SourceConfig::parse(&text).ok()?;
    Some(cfg.source.name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSource {
    pub name: String,
    pub root: PathBuf,
    pub origin: SourceOrigin,
}

/// Validate a user-typed path and resolve it to a `SelectedSource`.
///
/// The UI calls this when the user types a path in the manual input, before
/// navigating to `/source/<encoded>/list`. The encoded path is base64
/// (URL-safe) so a filesystem path with slashes survives round-tripping
/// through the URL.
pub fn resolve_source(path: &Path) -> Result<SelectedSource, WebSourceError> {
    let expanded = expand_tilde(path);

    if !expanded.exists() {
        return Err(WebSourceError::NotFound(expanded.display().to_string()));
    }

    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let paths = ConfigPaths::discover(&env, &expanded)
        .map_err(|e| WebSourceError::GlobalConfig(e.to_string()))?;

    // Delegate the actual `SelectedSource` assembly to the existing
    // `select_source` machinery; it already handles toml/file/dir cases.
    // We need a ConfigPaths for the Default selector path but not for
    // explicit Path, so build a minimal one.
    let mut paths_for_select = paths;
    paths_for_select.cwd = expanded.clone();

    select_source(&paths_for_select, SourceSelector::Path(&expanded)).map_err(|e| match e {
        ConfigError::Invalid(_, msg) if msg.starts_with("invalid source path") => {
            WebSourceError::NotASource(msg)
        }
        ConfigError::Invalid(_, msg) => WebSourceError::Source(msg),
        ConfigError::TomlField { message } => WebSourceError::Source(message),
        other => WebSourceError::Source(other.to_string()),
    })
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = std::env::var_os("HOME") {
            return if stripped.as_os_str().is_empty() {
                PathBuf::from(home)
            } else {
                PathBuf::from(home).join(stripped)
            };
        }
    }
    path.to_path_buf()
}

/// Re-export of [`SourceConfig`] for downstream code that wants to inspect the
/// resolved source without taking a hard dependency on the model module
/// layout.
pub use crate::config::model::SourceConfig as ResolvedSourceConfig;
