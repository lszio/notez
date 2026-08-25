//! `composition` — the single composition root for notez runtimes.
//!
//! Every transport (CLI, MCP, web) obtains its [`ApplicationFacade`]
//! from here so that space selection, runtime config resolution,
//! legacy `sources.json` merging, database placement and format-parser
//! registration happen exactly once, identically for all surfaces
//! (architecture doc §4: Core provides the only domain use-case seam;
//! transports must not re-implement assembly).
//!
//! Library name: `notez_composition`.

use notez_core::application::federation::SourceInstancesCache;
use notez_core::application::{ApplicationFacade, SourceContext};
use notez_core::config::{
    ConfigError, ConfigPaths, SelectedSource, SourceSelector, resolve_source_runtime, select_source,
};
use notez_core::storage::{SqliteProjection, StorageError};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A fully wired space runtime: the bound facade plus the
/// [`SourceContext`] it was opened against.
pub struct SpaceHandle {
    pub ctx: SourceContext,
    pub facade: ApplicationFacade<SqliteProjection>,
    /// The resolved selection this runtime was opened from (the CLI
    /// `config migrate` surface still consumes its raw source config).
    pub selected: SelectedSource,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenSpaceError {
    /// Space selection or runtime config failed (CLI maps this to exit 2).
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    /// The projection database could not be opened (CLI exit 5).
    #[error("storage error: {0}")]
    Store(#[from] StorageError),
}

/// Discover configuration and open the space selected by `selector`.
///
/// `db_override` replaces the resolved database path (the CLI `--db`
/// flag). When relative it is interpreted against the process working
/// directory, matching the historical CLI behaviour.
pub fn open_space(
    selector: SourceSelector<'_>,
    db_override: Option<&Path>,
) -> Result<SpaceHandle, OpenSpaceError> {
    let env = collect_env();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = ConfigPaths::discover(&env, &cwd)?;
    let selected = select_source(&paths, selector)?;
    open_selected_with_paths(&paths, &env, &selected, db_override)
}

/// Open a runtime from an already-resolved selection. The web client
/// resolves spaces through its own registry lookup and only needs the
/// shared wiring half of this module.
pub fn open_selected(
    selected: &SelectedSource,
    db_override: Option<&Path>,
) -> Result<SpaceHandle, OpenSpaceError> {
    let env = collect_env();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = ConfigPaths::discover(&env, &cwd)?;
    open_selected_with_paths(&paths, &env, selected, db_override)
}

fn open_selected_with_paths(
    paths: &ConfigPaths,
    env: &BTreeMap<String, OsString>,
    selected: &SelectedSource,
    db_override: Option<&Path>,
) -> Result<SpaceHandle, OpenSpaceError> {
    let resolved = resolve_source_runtime(paths, selected, env, None)?;

    // Merge legacy `.notez/sources.json` (written by `notez source add`)
    // into the runtime so mutations see sources registered in prior
    // invocations. Sources declared in `notez.toml` win.
    let mut runtime = resolved.config.clone();
    let json_sources = SourceInstancesCache::load(&selected.root)
        .ok()
        .map(|c| c.sources)
        .unwrap_or_default();
    for src in json_sources {
        if !runtime.sources.iter().any(|s| s.id == src.id) {
            runtime.sources.push(src);
        }
    }

    let db_path: PathBuf = match db_override {
        Some(path) => path.to_path_buf(),
        None => runtime.source.database.clone(),
    };
    if let Some(parent) = db_path.parent()
        && !parent.exists()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    let store = SqliteProjection::open(&db_path)?;
    let space = SourceContext::new(runtime.source.name.clone(), selected.root.clone(), runtime);
    let mut facade = ApplicationFacade::with_source(store, space.clone());
    register_builtin_format_parsers(&mut facade);
    Ok(SpaceHandle {
        ctx: space,
        facade,
        selected: selected.clone(),
    })
}

/// Register the built-in Org / Markdown format parsers. Centralised so
/// every transport scans with the same parser set.
fn register_builtin_format_parsers(facade: &mut ApplicationFacade<SqliteProjection>) {
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
}

fn collect_env() -> BTreeMap<String, OsString> {
    std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().to_string(), v))
        .collect()
}
