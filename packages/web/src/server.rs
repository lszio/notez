//! Web server functions for the v0.2 reader.
//!
//! Each `#[server]` function takes the per-request `space_root` (an
//! absolute path the user picked in the UI) and a `ResourceRef` when
//! relevant. The `space_root` is opened into a `SqliteProjection`
//! lazily per call so a single server can serve many spaces.
//!
//! There is no `NOTEZ_SPACE_ROOT` env var anymore: spaces are chosen
//! dynamically by the user, so the only thing the server process needs
//! at startup is the bind address (`PORT` / `IP`, handled by the
//! Dioxus fullstack runtime).
//!
//! v0.2 adds four derived views over the projection:
//! `list_space_tree`, `list_source_files`, `search_palette`,
//! `resolve_index`. See the `tree` module for details.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use notez_core::application::{ApplicationFacade, Graph, GraphEdge, GraphNode};
use notez_core::config::{
    web_space::{
        list_spaces as core_list_spaces, resolve_space as core_resolve_space,
        RegisteredSpace, SpaceSource, WebSpaceError,
    },
    SelectedSpace,
};
use serde::{Deserialize, Serialize};

use crate::body::render_body;
use crate::model::ResourceRow;
use crate::router::Route;
use crate::tree::{
    self, IndexEntryDto, KindCounts, SearchHit, SourceFileRow, TreeNode,
};
use notez_core::domain::{Resource, ResourceRef, ResourceKind, Selector};
use notez_core::storage::SqliteProjection;

// ---- DTO surface -----------------------------------------------------------

/// Lightweight snapshot of a registered space, returned to the UI for
/// the picker's dropdown. Mirrors `RegisteredSpace` but with
/// `String` paths so the wire shape stays simple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredSpaceDto {
    pub name: String,
    pub path: String,
    /// `"registered"` (came from the global XDG config) or
    /// `"discovered"` (came from the auto-discovery scan). The
    /// picker UI uses this to render a small badge so the user
    /// can tell which entries are sticky vs ad-hoc.
    pub source: String,
}

impl From<RegisteredSpace> for RegisteredSpaceDto {
    fn from(r: RegisteredSpace) -> Self {
        let source = match r.source {
            SpaceSource::Registered => "registered",
            SpaceSource::Discovered => "discovered",
        };
        Self {
            name: r.name,
            path: r.path.to_string_lossy().into_owned(),
            source: source.to_string(),
        }
    }
}

/// Snapshot of a resolved space, returned alongside successful calls.
/// The web client uses this to display the friendly space name in the
/// header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedSpaceDto {
    pub name: String,
    pub path: String,
    pub db_path: String,
}

impl From<SelectedSpace> for SelectedSpaceDto {
    fn from(s: SelectedSpace) -> Self {
        let db_path = s
            .space_config
            .space
            .database
            .to_string_lossy()
            .into_owned();
        Self {
            name: s.space_name,
            path: s.space_root.to_string_lossy().into_owned(),
            db_path,
        }
    }
}

/// Error variants the UI can render directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WebServerError {
    /// The supplied path does not exist on disk.
    NotFound { path: String, message: String },
    /// The path exists but is not a notez space.
    NotASpace { path: String, message: String },
    /// The supplied resource ref is malformed.
    InvalidRef { raw: String, message: String },
    /// Anything else (DB open failure, IO error, etc.).
    Internal { message: String },
}

impl WebServerError {
    fn not_found(path: &str, msg: impl Into<String>) -> Self {
        Self::NotFound {
            path: path.to_string(),
            message: msg.into(),
        }
    }
    fn not_a_space(path: &str, msg: impl Into<String>) -> Self {
        Self::NotASpace {
            path: path.to_string(),
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self::Internal { message: msg.into() }
    }
}

impl std::fmt::Display for WebServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { message, .. } => write!(f, "{message}"),
            Self::NotASpace { message, .. } => write!(f, "{message}"),
            Self::InvalidRef { message, .. } => write!(f, "{message}"),
            Self::Internal { message } => write!(f, "{message}"),
        }
    }
}

impl From<WebSpaceError> for WebServerError {
    fn from(err: WebSpaceError) -> Self {
        let msg = err.to_string();
        match err {
            WebSpaceError::NotFound(p) => Self::NotFound { path: p, message: msg },
            WebSpaceError::NotASpace(p) => Self::NotASpace { path: p, message: msg },
            other => Self::Internal {
                message: format!("{other:?}: {msg}"),
            },
        }
    }
}

// ---- server functions ------------------------------------------------------

/// List every space registered in the global XDG config.
///
/// Returns an empty vec when the global config is missing or unreadable;
/// the picker is allowed to be empty, never crashes the server.
#[server]
pub async fn list_registered_spaces() -> Result<Vec<RegisteredSpaceDto>, ServerFnError> {
    let env: BTreeMap<String, OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(core_list_spaces(&env, &cwd)
        .into_iter()
        .map(RegisteredSpaceDto::from)
        .collect())
}

/// Validate a user-typed path and resolve it to a `SelectedSpace`.
///
/// The UI calls this when the user types a path in the manual input,
/// before navigating to `/space/<encoded>/list`. The encoded path
/// embeds an absolute filesystem path.
#[server]
pub async fn resolve_space_path(path: String) -> Result<SelectedSpaceDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&path))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    Ok(SelectedSpaceDto::from(sel))
}

/// Return the friendly space info for a given path. Used by the layout
/// to render the current space name in the header without forcing the
/// page to re-derive it.
#[server]
pub async fn selected_space(space_root: String) -> Result<SelectedSpaceDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    Ok(SelectedSpaceDto::from(sel))
}

#[server]
pub async fn list_resources(space_root: String) -> Result<Vec<ResourceRow>, ServerFnError> {
    list_resources_impl(Path::new(&space_root))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_resource(
    space_root: String,
    ref_str: String,
) -> Result<Option<ResourceRow>, ServerFnError> {
    get_resource_impl(Path::new(&space_root), &ref_str)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_graph(space_root: String) -> Result<Graph, ServerFnError> {
    list_graph_impl(Path::new(&space_root))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn neighbor_graph(
    space_root: String,
    ref_str: String,
) -> Result<Graph, ServerFnError> {
    neighbor_graph_impl(Path::new(&space_root), &ref_str)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// ---- v0.2 derived views -----------------------------------------------------

/// Folder hierarchy derived from every resource's `locator`.
#[server]
pub async fn list_space_tree(space_root: String) -> Result<TreeNode, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_tree_with_disk(&facade, &sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))
}

/// Flat list of every org/md/attachment file in the space.
#[server]
pub async fn list_source_files(space_root: String) -> Result<Vec<SourceFileRow>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_source_files(&facade, &sel.space_root)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Aggregate counts of every resource kind in the space.
#[server]
pub async fn list_kind_counts(space_root: String) -> Result<KindCounts, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_kind_counts(&facade).map_err(|e| ServerFnError::new(e.to_string()))
}

/// Find the `index.org`/`index.md`/`README.*` document the home page
/// falls back to. `None` means the space has no index document and the
/// welcome page should fall through to a directory listing.
#[server]
pub async fn resolve_index(space_root: String) -> Result<Option<IndexEntryDto>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_index_entry(&facade).map_err(|e| ServerFnError::new(e.to_string()))
}
/// Render a file at `locator` (relative to the space) as the
/// preview HTML the detail page uses. Works for both indexed
/// resources (delegates to `body::render_body`) and loose files
/// on disk (delegates to `body::render_path`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub title: String,
    pub display_path: String,
    pub size: u64,
    pub kind: String,
    pub body_html: String,
}

#[server]
pub async fn render_preview(
    space_root: String,
    locator: String,
) -> Result<Option<Preview>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let row_opt = {
        let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
        if let Ok(r_ref) = notez_core::domain::ResourceRef::parse(&locator) {
            facade.read(&r_ref).ok().flatten()
        } else {
            None
        }
    };
    if let Some(r) = row_opt {
        let mut row = ResourceRow::from(r);
        row.body_html = crate::body::render_body(&row, &sel.space_root);
        let size = sel.space_root.join(&row.locator).metadata().map(|m| m.len()).unwrap_or(0);
        return Ok(Some(Preview {
            title: row.title,
            display_path: row.locator.clone(),
            size,
            kind: row.kind,
            body_html: row.body_html,
        }));
    }
    let file_path = sel.space_root.join(&locator);
    if !file_path.exists() {
        return Ok(None);
    }
    let title = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&locator)
        .to_string();
    let size = file_path.metadata().map(|m| m.len()).unwrap_or(0);
    let body_html = crate::body::render_path(&file_path, &title, &sel.space_root);
    Ok(Some(Preview {
        title,
        display_path: locator,
        size,
        kind: "attachment".to_string(),
        body_html,
    }))
}

/// AND-substring search across titles, filenames, and small file bodies.
/// Returns at most 80 hits, ordered as the projection returns them.
#[server]
pub async fn search_palette(
    space_root: String,
    q: String,
) -> Result<Vec<SearchHit>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::routes::auto_start_watch(&sel.space_root);
    let facade = open_facade(&sel.space_root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_search(&facade, &sel.space_root, &q)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// Walk the space directory and return every non-hidden file. Used
// by the left-column files panel and the per-space home page so
// users can preview loose attachments (PDFs, images, archives)
// the moment they drop them in, even before the projection has
// indexed them.
#[server]
pub async fn list_filesystem(space_root: String) -> Result<Vec<SourceFileRow>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&space_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tree::build_filesystem_listing(&sel.space_root))
}
// ---- _impl helpers (unit-testable) -----------------------------------------

/// Open the projection store + facade for `space_root`, going through
/// the `WebState` cache so repeat calls reuse the same SQLite handle.
fn open_facade(space_root: &Path) -> Result<std::sync::Arc<std::sync::Mutex<ApplicationFacade<SqliteProjection>>>, String> {
    let state = crate::routes::state_snapshot();
    state.facade_for(&space_root.to_path_buf()).map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn list_resources_impl(space_root: &Path) -> Result<Vec<ResourceRow>, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    crate::routes::auto_start_watch(&sel.space_root);
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    let page = facade.query(&Selector::new()).map_err(|e| e.to_string())?;
    Ok(page.items.into_iter().map(ResourceRow::from).collect())
}

#[cfg(target_arch = "wasm32")]
pub async fn list_resources_impl(_space_root: &Path) -> Result<Vec<ResourceRow>, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_resource_impl(
    space_root: &Path,
    ref_str: &str,
) -> Result<Option<ResourceRow>, String> {
    let r_ref = ResourceRef::parse(ref_str)
        .map_err(|e| WebServerError::InvalidRef { raw: ref_str.to_string(), message: e.to_string() }.to_string())?;
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    let res: Option<notez_core::domain::Resource> = facade.read(&r_ref).map_err(|e| e.to_string())?;
    Ok(res.map(|r| {
        let mut row = ResourceRow::from(r);
        row.body_html = render_body(&row, &sel.space_root);
        row
    }))
}

#[cfg(target_arch = "wasm32")]
pub async fn get_resource_impl(_space_root: &Path, _ref_str: &str) -> Result<Option<ResourceRow>, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn list_graph_impl(space_root: &Path) -> Result<Graph, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    Graph::from_facade(&facade).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn list_graph_impl(_space_root: &Path) -> Result<Graph, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn neighbor_graph_impl(space_root: &Path, focus_ref: &str) -> Result<Graph, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    Graph::neighborhood(&facade, focus_ref).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn neighbor_graph_impl(_space_root: &Path, _focus_ref: &str) -> Result<Graph, String> {
    Err("server-only".to_string())
}

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_minimal_space(root: &Path) {
        fs::create_dir_all(root.join(".notez")).unwrap();
        fs::write(
            root.join("notez.toml"),
            format!(
                "version = 1\n\n[space]\nname = \"{}\"\ndatabase = \".notez/index.sqlite\"\n",
                root.file_name().unwrap().to_string_lossy()
            ),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn list_resources_impl_rejects_missing_path() {
        let tmp = tempdir().unwrap();
        let err = list_resources_impl(&tmp.path().join("nope"))
            .await
            .expect_err("missing path must fail");
        assert!(
            err.contains("does not exist") || err.contains("not a notez space"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn list_resources_impl_rejects_plain_directory() {
        let tmp = tempdir().unwrap();
        let plain = tmp.path().join("plain");
        fs::create_dir_all(&plain).unwrap();
        let err = list_resources_impl(&plain).await.expect_err("plain dir must fail");
        assert!(err.contains("not a notez space"), "got: {err}");
    }

    #[tokio::test]
    async fn list_resources_impl_succeeds_for_valid_space() {
        let tmp = tempdir().unwrap();
        let space = tmp.path().join("work");
        write_minimal_space(&space);
        // No resources yet, but the projection should open and return empty.
        let rows = list_resources_impl(&space).await.expect("valid space");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn get_resource_impl_rejects_bad_ref() {
        let tmp = tempdir().unwrap();
        let space = tmp.path().join("work");
        write_minimal_space(&space);
        let err = get_resource_impl(&space, "not a ref")
            .await
            .expect_err("bad ref must fail");
        assert!(err.contains("invalid") || err.contains("Invalid"), "got: {err}");
    }

    #[test]
    fn web_server_error_from_web_space_error_preserves_variants() {
        let path = "/nope".to_string();
        let e: WebServerError = WebSpaceError::NotFound(path.clone()).into();
        assert!(matches!(e, WebServerError::NotFound { .. }));

        let e: WebServerError = WebSpaceError::NotASpace(path.clone()).into();
        assert!(matches!(e, WebServerError::NotASpace { .. }));

        let e: WebServerError = WebSpaceError::Other("oops".into()).into();
        assert!(matches!(e, WebServerError::Internal { .. }));
    }
}