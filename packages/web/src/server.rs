//! Web server functions for the v0.1 reader.
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
use notez_core::domain::{Resource, ResourceRef, Selector};
use notez_core::storage::SqliteProjection;
use serde::{Deserialize, Serialize};

use crate::model::ResourceRow;
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
            WebSpaceError::NotFound(p) => Self::not_found(&p, msg),
            WebSpaceError::NotASpace(p) => Self::not_a_space(&p, msg),
            WebSpaceError::Other(m) => Self::internal(m),
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
    core_resolve_space(Path::new(&path))
        .map(SelectedSpaceDto::from)
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Return the friendly space info for a given path. Used by the layout
/// to render the current space name in the header without forcing the
/// page to re-derive it.
#[server]
pub async fn selected_space(space_root: String) -> Result<SelectedSpaceDto, ServerFnError> {
    core_resolve_space(Path::new(&space_root))
        .map(SelectedSpaceDto::from)
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))
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
// ---- _impl helpers (unit-testable) -----------------------------------------

pub async fn list_resources_impl(space_root: &Path) -> Result<Vec<ResourceRow>, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    let page = facade.query(&Selector::new()).map_err(|e| e.to_string())?;
    Ok(page.items.into_iter().map(ResourceRow::from).collect())
}

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
    let res: Option<Resource> = facade.read(&r_ref).map_err(|e| e.to_string())?;
    Ok(res.map(ResourceRow::from))
}
// ---- tests -----------------------------------------------------------------


/// Build the full force-directed graph for a space. Used by the
/// `/space/.../graph` page.
pub async fn list_graph_impl(space_root: &Path) -> Result<Graph, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    Graph::from_facade(&facade).map_err(|e| e.to_string())
}

/// Build a 1-hop subgraph around `focus_ref`. Used by the
/// detail-page neighbour graph.
pub async fn neighbor_graph_impl(space_root: &Path, focus_ref: &str) -> Result<Graph, String> {
    let sel = core_resolve_space(space_root).map_err(|e| e.to_string())?;
    let db_path = sel.space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    Graph::neighborhood(&facade, focus_ref).map_err(|e| e.to_string())
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

    #[tokio::test]
    async fn web_server_error_from_web_space_error_preserves_variants() {
        let path = "/nope".to_string();
        let e: WebServerError = WebSpaceError::NotFound(path.clone()).into();
        assert!(matches!(e, WebServerError::NotFound { .. }));

        let e: WebServerError = WebSpaceError::NotASpace(path.clone()).into();
        assert!(matches!(e, WebServerError::NotASpace { .. }));

        let e: WebServerError = WebSpaceError::Other("oops".into()).into();
        assert!(matches!(e, WebServerError::Internal { .. }));
    }
}
