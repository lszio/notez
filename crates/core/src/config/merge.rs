use crate::config::defaults::resolve_path;
use crate::config::discovery::{ConfigPaths, SelectedSource};
use crate::config::model::{ConfigError, Preferences, SourceConfig};
use std::collections::BTreeMap;
use std::ffi::OsString;

/// Resolved runtime binding for a source: the on-disk `SourceConfig` plus
/// the merged `Preferences` (global + env + CLI). Lives here so the CLI can
/// compute it without depending on `application`.
pub struct ResolvedSourceRuntime {
    pub config: SourceConfig,
    pub preferences: Preferences,
}

pub fn resolve_source_runtime(
    paths: &ConfigPaths,
    sel: &SelectedSource,
    env: &BTreeMap<String, OsString>,
    cli_output: Option<String>,
) -> Result<ResolvedSourceRuntime, ConfigError> {
    let mut prefs = paths
        .global_config
        .as_ref()
        .map(|g| g.preferences.clone())
        .unwrap_or_default();

    if let Some(log) = env.get("NOTEZ_LOG_LEVEL") {
        if let Some(s) = log.to_str() {
            prefs.log_level = s.to_string();
        }
    }
    if let Some(out) = cli_output {
        prefs.output = out;
    }

    let mut source_config = sel.source_config.clone();

    if source_config.source.database.is_relative() {
        source_config.source.database = resolve_path(&sel.root, &source_config.source.database);
    }

    for s in source_config.sources.iter_mut() {
        if s.path.is_relative() {
            s.path = resolve_path(&sel.root, &s.path);
        }
        s.include_paths = std::mem::take(&mut s.include_paths)
            .into_iter()
            .map(|p| resolve_path(&sel.root, &p))
            .collect();
        s.exclude_paths = std::mem::take(&mut s.exclude_paths)
            .into_iter()
            .map(|p| resolve_path(&sel.root, &p))
            .collect();
    }

    Ok(ResolvedSourceRuntime {
        config: source_config,
        preferences: prefs,
    })
}
