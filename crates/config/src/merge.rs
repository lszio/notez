use crate::discovery::{ConfigPaths, SelectedSpace};
use crate::defaults::resolve_path;
use crate::model::{ConfigError, RuntimeConfig};
use std::collections::BTreeMap;
use std::ffi::OsString;

pub fn load_runtime_config(
    paths: &ConfigPaths,
    sel: &SelectedSpace,
    env: &BTreeMap<String, OsString>,
    cli_output: Option<String>,
) -> Result<RuntimeConfig, ConfigError> {
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

    let space_config = sel.space_config.clone();

    let database = resolve_path(&sel.space_root, &space_config.space.database);

    let mut sources = Vec::new();
    for s in space_config.sources {
        let mut source_config = s.into_source_config();
        source_config.path = resolve_path(&sel.space_root, &source_config.path);
        // Include/exclude paths are also resolved relative to space root.
        source_config.include_paths = source_config.include_paths.into_iter().map(|p| resolve_path(&sel.space_root, &p)).collect();
        source_config.exclude_paths = source_config.exclude_paths.into_iter().map(|p| resolve_path(&sel.space_root, &p)).collect();
        sources.push(source_config);
    }

    Ok(RuntimeConfig {
        space_root: sel.space_root.clone(),
        space_name: sel.space_name.clone(),
        database,
        workflow: space_config.workflow,
        sources,
        preferences: prefs,
    })
}
