//! `EmbeddedBackend` — desktop-side implementation of [`ui::Backend`].
//!
//! Each call opens (and caches) an `Engine` via the composition
//! `Runtime` for the requested space root, then dispatches a typed
//! protocol request. The same Runtime is reused across calls so a
//! desktop session keeps one open database + watcher per space.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notez_composition::native::{OpenSpaceError, SpaceHandle};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::config::discovery::{SelectedSource, SourceSelector};
use notez_core::config::ConfigPaths;
use notez_protocol::request::{
    QueryResourcesRequest, Request, ScanNativeRequest,
};
use ui::{Backend, ResourceRow, SpaceRow};

type EngineHandle =
    Arc<Mutex<notez_core::application::Engine<notez_core::storage::SqliteProjection>>>;

fn discover_paths() -> Result<ConfigPaths, String> {
    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?;
    ConfigPaths::discover(&env, &cwd).map_err(|e| format!("config discovery failed: {e}"))
}

fn open_space(root: &PathBuf) -> Result<SpaceHandle, OpenSpaceError> {
    notez_composition::native::open_space(SourceSelector::Path(root.as_path()), None)
}

/// Backend that talks directly to a local `Engine`. In-process, no
/// HTTP, single-tenant.
#[derive(Clone)]
pub struct EmbeddedBackend {
    default_root: Option<PathBuf>,
}

impl EmbeddedBackend {
    pub fn new(default_root: Option<PathBuf>) -> Self {
        Self { default_root }
    }

    fn resolved_root<'a>(&'a self, root: &'a str) -> Result<PathBuf, String> {
        if root.is_empty() {
            return self
                .default_root
                .clone()
                .ok_or_else(|| "no space selected".to_string());
        }
        let path = PathBuf::from(root);
        if path.exists() {
            return Ok(path);
        }
        Err(format!("cannot resolve space '{root}'"))
    }

    fn dispatch_blocking(&self, root: &PathBuf, request: Request) -> Result<Response, String> {
        let handle = open_space(root).map_err(|e: OpenSpaceError| e.to_string())?;
        let engine: EngineHandle = Arc::new(Mutex::new(handle.engine));
        let mut guard = engine
            .lock()
            .map_err(|e| format!("engine lock poisoned: {e}"))?;
        ApplicationDispatcher::new(&mut guard)
            .dispatch(request)
            .map_err(|e| e.to_string())
    }

    fn list_spaces_blocking(&self) -> Result<Vec<SpaceRow>, String> {
        let paths = discover_paths()?;
        let mut out: Vec<SpaceRow> = Vec::new();
        if let Some(default) = self.default_root.as_ref() {
            out.push(SpaceRow {
                source_id: "<default>".to_string(),
                display_name: default
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("space")
                    .to_string(),
                root: default.clone(),
            });
        } else {
            // Fall back to the global default source if no NOTEZ_DEFAULT_SPACE.
            if let Some(global_default) = paths
                .global_config
                .as_ref()
                .and_then(|g| g.sources.iter().next().map(|(_, r)| r.path.clone()))
                .map(expand_tilde)
            {
                out.push(SpaceRow {
                    source_id: "<global-default>".to_string(),
                    display_name: global_default
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("space")
                        .to_string(),
                    root: global_default,
                });
            }
        }
        Ok(out)
    }
}

fn expand_tilde(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    p
}

impl Backend for EmbeddedBackend {
    async fn list_spaces(&self) -> Result<Vec<SpaceRow>, String> {
        let backend = self.clone();
        tokio::task::spawn_blocking(move || backend.list_spaces_blocking())
            .await
            .map_err(|e| format!("join error: {e}"))?
    }

    async fn scan_space(&self, root: &str) -> Result<u32, String> {
        let backend = self.clone();
        let root_text = root.to_string();
        tokio::task::spawn_blocking(move || -> Result<u32, String> {
            let root = backend.resolved_root(&root_text)?;
            let response = backend.dispatch_blocking(
                &root,
                Request::ScanNative(ScanNativeRequest {}),
            )?;
            match response {
                Response::Scan(report) => Ok(report.scanned_resources as u32),
                other => Err(format!("unexpected scan response: {other:?}")),
            }
        })
        .await
        .map_err(|e| format!("join error: {e}"))?
    }

    async fn query_resources(
        &self,
        root: &str,
        title_contains: Option<&str>,
        kind: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<ResourceRow>, String> {
        let backend = self.clone();
        let root_text = root.to_string();
        let title_contains = title_contains.map(|s| s.to_string());
        let kind = kind.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<Vec<ResourceRow>, String> {
            let root = backend.resolved_root(&root_text)?;
            let request = Request::QueryResources(QueryResourcesRequest {
                kind,
                title_contains,
                exact_ref: None,
                source_id: None,
                limit,
            });
            let response = backend.dispatch_blocking(&root, request)?;
            match response {
                Response::ResourcePage(page) => Ok(page
                    .items
                    .into_iter()
                    .map(|r| ResourceRow {
                        kind: format!("{:?}", r.kind),
                        id: r.ref_,
                        title: r.title,
                        locator: r.locator,
                    })
                    .collect()),
                other => Err(format!("unexpected query response: {other:?}")),
            }
        })
        .await
        .map_err(|e| format!("join error: {e}"))?
    }
}

// Suppress an unused warning for the imported discovery helper we
// keep available for future richer space listings.
#[allow(dead_code)]
fn _selected_source_root(s: &SelectedSource) -> &PathBuf {
    &s.root
}
