//! `composition` — the single composition root for notez runtimes.
//!
//! Every transport obtains the shared `Engine` runtime from this module.
//! Source selection, configuration, storage, and parser registration happen
//! exactly once so surfaces cannot diverge.
//! (architecture doc §4: Core provides the only domain use-case seam;
//! transports must not re-implement assembly).
//!
//! Library name: `notez_composition`.

#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    use notez_core::application::federation::SourceInstancesCache;
    use notez_core::application::{Engine, NativeJanetExecutor, SourceContext, WatchService};
    use notez_core::config::{
        ConfigError, ConfigPaths, SelectedSource, SourceSelector, resolve_source_runtime,
        select_source,
    };
    use notez_core::storage::{SqliteProjection, StorageError};
    use std::collections::{BTreeMap, HashMap};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    
    /// A fully wired space runtime: the bound engine plus the
    /// [`SourceContext`] it was opened against.
    pub struct SpaceHandle {
        pub ctx: SourceContext,
        pub engine: Engine<SqliteProjection>,
        /// The resolved selection this runtime was opened from.
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
        // M3 event spine: journal and audit share the projection database so
        // writes are recorded alongside the projection they mutate. A second
        // connection is opened to keep the public `ProjectionStore` interface
        // journal and audit use the same database while remaining separate ports.
        let journal = notez_core::storage::SqliteEventJournal::new(
            notez_core::storage::SqliteProjection::open_for_adapter(&db_path)
                .map_err(OpenSpaceError::Store)?,
        );
        let audit = notez_core::storage::SqliteAuditLog::new(
            notez_core::storage::SqliteProjection::open_for_adapter(&db_path)
                .map_err(OpenSpaceError::Store)?,
        );
        let space = SourceContext::new(runtime.source.name.clone(), selected.root.clone(), runtime);
        let mut engine = Engine::with_source(store, space.clone());
        engine.attach_journal(journal);
        engine.attach_audit(audit);
        engine.attach_janet_executor(NativeJanetExecutor);
        register_builtin_format_parsers(&mut engine);
        Ok(SpaceHandle {
            ctx: space,
            engine,
            selected: selected.clone(),
        })
    }
    
    /// Register the built-in Org / Markdown format parsers. Centralised so
    /// every transport scans with the same parser set.
    fn register_builtin_format_parsers(engine: &mut Engine<SqliteProjection>) {
        engine.register_format_parser(Box::new(orgmode::OrgParser::new()));
        engine.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    }
    
    fn collect_env() -> BTreeMap<String, OsString> {
        std::env::vars_os()
            .map(|(k, v)| (k.to_string_lossy().to_string(), v))
            .collect()
    }
    /// Shared server runtime: an engine cache keyed by canonical source
    /// root, plus the watch service. Every transport in one process (web
    /// SSR, HTTP API, MCP host, embedded desktop) opens engines through
    /// the same `Runtime`, so a canonical root maps to exactly one engine
    /// and one watcher — no surface keeps its own second cache.
    #[derive(Clone)]
    pub struct Runtime {
        engines: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<Engine<SqliteProjection>>>>>>,
        watch: Arc<WatchService>,
    }

    impl Default for Runtime {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Runtime {
        /// New runtime owning its own watch service.
        pub fn new() -> Self {
            Self {
                engines: Arc::new(Mutex::new(HashMap::new())),
                watch: WatchService::new(),
            }
        }

        /// New runtime sharing an externally owned watch service. The web
        /// host passes its process-global watcher so every `WebState` /
        /// router copy observes the same live-reindex state.
        pub fn with_watch(watch: Arc<WatchService>) -> Self {
            Self {
                engines: Arc::new(Mutex::new(HashMap::new())),
                watch,
            }
        }

        /// The watch service this runtime starts watchers on.
        pub fn watch(&self) -> Arc<WatchService> {
            self.watch.clone()
        }

        /// Open (or reuse) the engine for an already-resolved selection.
        /// Starting the source watcher is part of opening: a surface that
        /// opens an engine gets live reindexing without extra ceremony.
        pub fn open(
            &self,
            selected: &SelectedSource,
        ) -> Result<Arc<Mutex<Engine<SqliteProjection>>>, OpenSpaceError> {
            let canonical = std::fs::canonicalize(&selected.root)
                .unwrap_or_else(|_| selected.root.clone());
            {
                let cache = self.engines.lock().expect("runtime engine cache poisoned");
                if let Some(engine) = cache.get(&canonical) {
                    return Ok(engine.clone());
                }
            }
            let handle = open_selected(selected, None)?;
            let engine = Arc::new(Mutex::new(handle.engine));
            self.engines
                .lock()
                .expect("runtime engine cache poisoned")
                                .insert(canonical.clone(), engine.clone());
            let _ = self.watch.start(&canonical);
            Ok(engine)
        }
    }

}

/// On wasm32 the composition root is unavailable: the client talks
/// to the HTTP/SSR surface instead of opening databases directly.
#[cfg(target_arch = "wasm32")]
pub mod native {}
