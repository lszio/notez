//! Web server functions for the v0.2 reader.
//!
//! Each `#[server]` function takes the per-request `source_root` (an
//! absolute path the user picked in the UI) and a `ResourceRef` when
//! relevant. The `source_root` is opened into a `SqliteProjection`
//! lazily per call so a single server can serve many sources.
//!
//! There is no `NOTEZ_SPACE_ROOT` env var anymore: sources are chosen
//! dynamically by the user, so the only thing the server process needs
//! at startup is the bind address (`PORT` / `IP`, handled by the
//! Dioxus fullstack runtime).
//!
//! v0.2 adds four derived views over the projection:
//! `list_space_tree`, `list_source_files`, `search_palette`,
//! `resolve_index`. See the `tree` module for details.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use notez_core::config::web_space::{
    list_sources, resolve_source as core_resolve_space, ListedSource, SourceOrigin,
    WebSourceError,
};
use notez_core::config::SelectedSource;
use notez_core::source::{SourceCapabilities, SourceKind};
 
 use serde::{Deserialize, Serialize};

use crate::body::render_body;
use crate::model::ResourceRow;
#[cfg(not(target_arch = "wasm32"))]
use crate::tree::{
    self, IndexEntryDto, KindCounts, SearchHit, SourceFileRow, TreeNode,
};
use notez_core::application::Graph;
#[cfg(not(target_arch = "wasm32"))]
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::application::Engine;
use notez_core::domain::change::ChangeOp;
#[cfg(not(target_arch = "wasm32"))]
use notez_core::domain::{Resource, ResourceKind, ResourceRef, Selector};
#[cfg(not(target_arch = "wasm32"))]
use notez_core::storage::SqliteProjection;
#[cfg(not(target_arch = "wasm32"))]
use notez_protocol::request::{
    ExecuteJanetRequest, QueryResourcesRequest, ReadResourceRequest, Request, UpdateDocumentRequest,
};
/// Server-only: kick the global watch service if running.
#[cfg(not(target_arch = "wasm32"))]
fn auto_start_watch(source_root: &std::path::Path) {
    crate::routes::auto_start_watch(source_root);
}
#[cfg(target_arch = "wasm32")]
#[inline]
fn auto_start_watch(_source_root: &std::path::Path) {}
/// Server-only: returns the cached `WebState` (wasm builds never
/// instantiate one — the stub returns `()` and the call site is cfg-guard).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn web_state() -> crate::routes::WebState {
    crate::routes::state_snapshot()
}
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn web_state() -> () { () }
#[cfg(not(target_arch = "wasm32"))]


#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dispatch_query(source_root: &Path) -> Result<Vec<Resource>, String> {
    let engine = open_engine(source_root)?;
    let mut guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let response = dispatcher.dispatch(Request::QueryResources(QueryResourcesRequest {
        kind: None, title_contains: None, exact_ref: None, source_id: None, limit: None,
    })).map_err(|e| e.to_string())?;
    match response {
        Response::ResourcePage(page) => page.items.into_iter().map(|item| {
            Ok(Resource {
                r#ref: ResourceRef::parse(&item.ref_).map_err(|e| e.to_string())?,
                kind: match item.kind {
                    notez_protocol::response::ResourceKind::Document => ResourceKind::Document,
                    notez_protocol::response::ResourceKind::Heading => ResourceKind::Heading,
                    notez_protocol::response::ResourceKind::Block => ResourceKind::Block,
                    notez_protocol::response::ResourceKind::Attachment => ResourceKind::Attachment,
                },
                title: item.title, revision: item.revision, source_id: item.source_id,
                locator: item.locator, properties: item.properties,
                object_id: notez_core::domain::ObjectIdentity::default(), primary_source_id: item.primary_source_id,
            })
        }).collect(),
        other => Err(format!("unexpected query response: {other:?}")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_read(source_root: &Path, r_ref: &ResourceRef) -> Result<Option<Resource>, String> {
    let engine = open_engine(source_root)?;
    let mut guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let response = dispatcher.dispatch(Request::ReadResource(ReadResourceRequest { r_ref: r_ref.to_string() })).map_err(|e| e.to_string())?;
    match response {
        Response::Resource(item) => item.map(|item| Ok(Resource {
            r#ref: ResourceRef::parse(&item.ref_).map_err(|e| e.to_string())?,
            kind: match item.kind {
                notez_protocol::response::ResourceKind::Document => ResourceKind::Document,
                notez_protocol::response::ResourceKind::Heading => ResourceKind::Heading,
                notez_protocol::response::ResourceKind::Block => ResourceKind::Block,
                notez_protocol::response::ResourceKind::Attachment => ResourceKind::Attachment,
            },
            title: item.title, revision: item.revision, source_id: item.source_id,
            locator: item.locator, properties: item.properties,
            object_id: notez_core::domain::ObjectIdentity::default(), primary_source_id: item.primary_source_id,
        })).transpose(),
        other => Err(format!("unexpected read response: {other:?}")),
    }
}

// ---- DTO surface -----------------------------------------------------------

/// Lightweight snapshot of a registered space, returned to the UI for
/// the picker's dropdown. Mirrors `RegisteredSource` but with
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

impl From<ListedSource> for RegisteredSpaceDto {
    fn from(r: ListedSource) -> Self {
        let source = match r.origin {
            SourceOrigin::Registered(_) => "registered",
            SourceOrigin::Discovered => "discovered",
            SourceOrigin::Ephemeral => "ephemeral",
        };
        Self {
            name: r.name,
            path: r.root.to_string_lossy().into_owned(),
            source: source.to_string(),
        }
    }
}

/// A configured source instance shown in the web source switcher. Remote
/// entries retain their namespace and health: an unavailable upstream is
/// never represented as an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceNamespaceDto {
    pub id: String,
    pub kind: String,
    pub url: Option<String>,
    pub read_only: bool,
    pub capabilities: Option<SourceCapabilities>,
    pub status: String,
    pub reason: Option<String>,
}
fn inspect_remote_source(cfg: notez_core::config::SourceInstanceConfig) -> SourceNamespaceDto {
    let read_only = true;
    let id = cfg.id.clone();
    let kind = cfg.kind.to_string();
    let url = cfg.url.clone();
    let adapter = notez_core::source::NotezRestSourceAdapter::new(cfg.into_source_config());
    match adapter.discover_capabilities() {
        Ok(capabilities) => SourceNamespaceDto {
            id, kind, url, read_only, capabilities: Some(capabilities),
            status: "ready".into(), reason: None,
        },
        Err(err) => SourceNamespaceDto {
            id, kind, url, read_only, capabilities: None,
            status: "unavailable".into(), reason: Some(err.to_string()),
        },
    }
}

#[server]
pub async fn list_source_namespaces(source_root: String) -> Result<Vec<SourceNamespaceDto>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let mut configs = sel.source_config.sources;
    if let Ok(cache) = notez_core::storage::SourceInstancesCache::load(&sel.root) {
        for cfg in cache.sources {
            if !configs.iter().any(|existing| existing.id == cfg.id) {
                configs.push(cfg);
            }
        }
    }
    Ok(configs.into_iter()
        .filter(|cfg| cfg.kind == SourceKind::NotezRest)
        .map(inspect_remote_source)
        .collect())
}
/// header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedSpaceDto {
    pub name: String,
    pub path: String,
    pub db_path: String,
}

impl From<SelectedSource> for SelectedSpaceDto {
    fn from(s: SelectedSource) -> Self {
        let db_path = s
            .source_config
            .source
            .database
            .to_string_lossy()
            .into_owned();
        Self {
            name: s.source_name,
            path: s.root.to_string_lossy().into_owned(),
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
    /// Machine-readable variant name, used to render a structured
    /// error label in the UI instead of a bare string.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::NotASpace { .. } => "not_a_space",
            Self::InvalidRef { .. } => "invalid_ref",
            Self::Internal { .. } => "internal",
        }
    }
}
impl std::fmt::Display for WebServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { message, .. }
            | Self::NotASpace { message, .. }
            | Self::InvalidRef { message, .. }
            | Self::Internal { message } => write!(f, "{message}"),
        }
    }
}

impl From<WebSourceError> for WebServerError {
    fn from(err: WebSourceError) -> Self {
        let msg = err.to_string();
        match err {
            WebSourceError::NotFound(p) => Self::NotFound { path: p, message: msg },
            WebSourceError::NotASource(p) => Self::NotASpace { path: p, message: msg },
            WebSourceError::Source(s)
            | WebSourceError::InvalidPath(s)
            | WebSourceError::GlobalConfig(s) => Self::Internal { message: format!("{s}: {msg}") },
        }
    }
}


/// List every space registered in the global XDG config.
///
/// Returns an empty vec when the global config is missing or unreadable;
/// the picker is allowed to be empty, never crashes the server.
#[server]
pub async fn list_registered_spaces() -> Result<Vec<RegisteredSpaceDto>, ServerFnError> {
    let env: std::collections::BTreeMap<String, std::ffi::OsString> =
        std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v)).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(list_sources(&env, &cwd)
        .unwrap_or_default()
        .into_iter()
        .map(RegisteredSpaceDto::from)
        .collect())
}

/// Validate a user-typed path and resolve it to a `SelectedSource`.
///
/// The UI calls this when the user types a path in the manual input,
/// before navigating to `/space/<encoded>/list`. The encoded path
/// embeds an absolute filesystem path.
#[server]
pub async fn resolve_space_path(path: String) -> Result<SelectedSpaceDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&path))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    Ok(SelectedSpaceDto::from(sel))
}

/// Return the friendly space info for a given path. Used by the layout
/// to render the current space name in the header without forcing the
/// page to re-derive it.
#[server]
pub async fn selected_space(source_root: String) -> Result<SelectedSpaceDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    Ok(SelectedSpaceDto::from(sel))
}

#[server]

/// List resources through the surface-agnostic `ui::Backend` rather
/// than the embedded engine. When the host runs with
/// `NOTEZ_DATA_BACKEND=http` this fn talks to the remote notez server
/// via `notez_api::NotezClient`; otherwise it stays in-process. The
/// returned DTO is the slim `ui::ResourceRow` (no `body_html`); pages
/// that need the rich DTO should keep using the typed `#[server]`
/// helpers below.
#[server]
pub async fn list_resources_via_backend(
    source_root: String,
) -> Result<Vec<crate::model::BackendResourceRow>, ServerFnError> {
    let backend = crate::host::DataBackend::from_env();
    let rows = ui::Backend::query_resources(&backend, &source_root, None, None, Some(50))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(crate::model::BackendResourceRow::from)
        .collect())
}
pub async fn list_resources(source_root: String) -> Result<Vec<ResourceRow>, ServerFnError> {

/// Scan a space through the surface-agnostic `ui::Backend`. Mirrors
/// `list_resources_via_backend`: when the host runs with
/// `NOTEZ_DATA_BACKEND=http` this fn goes through `notez-api::NotezClient`,
/// otherwise it stays in-process.
#[server]
pub async fn scan_space_via_backend(source_root: String) -> Result<u32, ServerFnError> {
    let backend = crate::host::DataBackend::from_env();
    ui::Backend::scan_space(&backend, &source_root)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Single-resource fetch through the surface-agnostic `ui::Backend`.
/// Returns a slim DTO (`BackendResourceRow`); pages that need a body
/// preview should keep using the typed `get_resource` server function
/// below (which retains the rich `ResourceRow` shape).
#[server]
pub async fn get_resource_via_backend(
    source_root: String,
    ref_str: String,
) -> Result<Option<crate::model::BackendResourceRow>, ServerFnError> {
    let backend = crate::host::DataBackend::from_env();
    let rows = ui::Backend::query_resources(&backend, &source_root, None, None, Some(50))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .find(|r| r.id == ref_str)
        .map(Into::into))
}
    list_resources_impl(Path::new(&source_root))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_resource(
    source_root: String,
    ref_str: String,
) -> Result<Option<ResourceRow>, ServerFnError> {
    get_resource_impl(Path::new(&source_root), &ref_str)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn list_graph(source_root: String) -> Result<Graph, ServerFnError> {
    list_graph_impl(Path::new(&source_root))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn neighbor_graph(
    source_root: String,
    ref_str: String,
) -> Result<Graph, ServerFnError> {
    neighbor_graph_impl(Path::new(&source_root), &ref_str)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// ---- v0.2 derived views -----------------------------------------------------

/// Folder hierarchy derived from every resource's `locator`.
#[server]
pub async fn list_space_tree(source_root: String) -> Result<TreeNode, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_tree_with_disk(&facade, &sel.root).map_err(|e| ServerFnError::new(e.to_string()))
}

/// Flat list of every org/md/attachment file in the space.
#[server]
pub async fn list_source_files(source_root: String) -> Result<Vec<SourceFileRow>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_source_files(&facade, &sel.root)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Aggregate counts of every resource kind in the space.
#[server]
pub async fn list_kind_counts(source_root: String) -> Result<KindCounts, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_kind_counts(&facade).map_err(|e| ServerFnError::new(e.to_string()))
}

// ---- activity (durable journal / audit) -------------------------------------

/// One row of the space activity stream, read from the durable event
/// journal (`event_journal`, written by the Projector before every
/// mutation). Entries are real recorded changes — never fabricated —
/// so an empty vec means "no writes recorded yet", not "nothing
/// happened".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityEntryDto {
    /// Monotonic journal sequence (newest entries carry the highest
    /// sequence).
    pub sequence: u64,
    /// ULID of the underlying Change event.
    pub change_id: String,
    /// Human-readable action label derived from the ChangeOp.
    pub action: String,
    /// Actor principal recorded on the Change ("web", "cli", …).
    pub actor: String,
    /// Wall-clock timestamp of the Change, unix millis.
    pub at_unix_millis: i64,
    /// Source id the Change ran against ("native", a remote id, …).
    pub source_id: String,
    /// Primary target ref, present only when the change affected
    /// exactly one resource (document writes, deletes, upserts).
    /// Bulk ops (scans, link resolution) expose `target_count`
    /// instead, so the UI never anchors to an arbitrary member of a
    /// large set.
    pub target: Option<String>,
    /// Number of resource refs the change touched.
    pub target_count: usize,
    /// Expected revision of the head resource when the op carried
    /// one (writeback / upsert).
    pub revision: Option<String>,
    /// Whether the Projector also appended audit records for this
    /// change. Ops that deliberately skip audit (segment extraction,
    /// link indexing) or that failed their store mutation after the
    /// journal row landed leave this false.
    pub audited: bool,
    /// Absolute space root the change belongs to — lets merged
    /// (cross-space) views anchor each row to its own space.
    pub source_root: String,
}

/// Map a journal `ChangeOp` to the human action label shown in the
/// activity stream.
#[cfg(not(target_arch = "wasm32"))]
pub fn activity_action_label(op: &ChangeOp) -> String {
    match op {
        ChangeOp::UpsertResource => "upsert resource".to_string(),
        ChangeOp::DeleteResource => "delete resource".to_string(),
        ChangeOp::InsertSegments => "extract segments".to_string(),
        ChangeOp::ReplaceLinkOccurrences => "index link occurrences".to_string(),
        ChangeOp::ReplaceResolvedRelations => "resolve links".to_string(),
        ChangeOp::WriteLinkDiagnostics => "link diagnostics".to_string(),
        ChangeOp::ReplaceConflicts => "replace conflicts".to_string(),
        ChangeOp::TransitionTask {
            from_state,
            to_state,
            ..
        } => format!("task transition: {from_state} → {to_state}"),
        ChangeOp::Writeback => "document write".to_string(),
        ChangeOp::Scan => "scan".to_string(),
        ChangeOp::Rebuilt => "rebuild index".to_string(),
    }
}


/// Convert the protocol-neutral journal record into the web activity DTO.
#[cfg(not(target_arch = "wasm32"))]
fn activity_entry_from_change(
    record: notez_core::domain::journal::ActivityRecord,
    source_root: &Path,
) -> Result<ActivityEntryDto, String> {
    let target_count = record.change.targets.len();
    let target = if target_count == 1 {
        Some(record.change.targets[0].to_string())
    } else {
        None
    };
    Ok(ActivityEntryDto {
        sequence: record.sequence,
        change_id: record.change.id.to_string(),
        action: activity_action_label(&record.change.op),
        actor: record.change.actor.principal,
        at_unix_millis: record.change.at_unix_millis,
        source_id: record.change.source_id,
        target,
        target_count,
        revision: record.change.expected_revision,
        audited: record.audited,
        source_root: source_root.to_string_lossy().into_owned(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_space_activity_impl(
    source_root: &Path,
    limit: usize,
) -> Result<Vec<ActivityEntryDto>, String> {
    let engine = open_engine(source_root)?;
    let guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let records = guard
        .journal_activity(limit.clamp(1, 200))
        .map_err(|e| e.to_string())?;
    records
        .into_iter()
        .map(|record| activity_entry_from_change(record, source_root))
        .collect()
}

#[cfg(target_arch = "wasm32")]
pub fn list_space_activity_impl(
    _source_root: &Path,
    _limit: usize,
) -> Result<Vec<ActivityEntryDto>, String> {
    Err("server-only".to_string())
}

/// Merge the newest journal entries across every registered space
/// (for the global home dashboard). Spaces whose journal cannot be
/// opened are skipped — the stream shows only real, readable
/// records.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_recent_activity_impl(limit: usize) -> Result<Vec<ActivityEntryDto>, String> {
    let env: std::collections::BTreeMap<String, std::ffi::OsString> =
        std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v)).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let spaces = list_sources(&env, &cwd).unwrap_or_default();
    let per_space = if spaces.is_empty() {
        0
    } else {
        (limit / spaces.len()).max(8)
    };
    let mut all: Vec<ActivityEntryDto> = Vec::new();
    for s in spaces {
        if let Ok(mut entries) = list_space_activity_impl(&s.root, per_space.max(1)) {
            all.append(&mut entries);
        }
    }
    all.sort_by(|a, b| {
        b.at_unix_millis
            .cmp(&a.at_unix_millis)
            .then(b.sequence.cmp(&a.sequence))
    });
    all.truncate(limit.max(1));
    Ok(all)
}

#[cfg(target_arch = "wasm32")]
pub fn list_recent_activity_impl(_limit: usize) -> Result<Vec<ActivityEntryDto>, String> {
    Err("server-only".to_string())
}

/// Activity stream for one space (`/source/:encoded/activity`).
#[server]
pub async fn list_space_activity(
    source_root: String,
    limit: usize,
) -> Result<Vec<ActivityEntryDto>, ServerFnError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        list_space_activity_impl(Path::new(&source_root), limit)
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (source_root, limit);
        Err(ServerFnError::new("server-only".to_string()))
    }
}

/// Merged activity across every registered space (home dashboard).
#[server]
pub async fn list_recent_activity(
    limit: usize,
) -> Result<Vec<ActivityEntryDto>, ServerFnError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        list_recent_activity_impl(limit).map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = limit;
        Err(ServerFnError::new("server-only".to_string()))
    }
}


/// Resolve the index.org-style landing entry together with the
/// matching document. Returns both as a single payload so the SSR
/// dashboard can render without the race that arises when two
/// independent `use_server_future`s fire (the document future runs
/// before the entry future resolves, so the body never appears).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IndexDocumentDto {
    pub entry: Option<IndexEntryDto>,
    pub document: Option<ResourceRow>,
}

#[server]
pub async fn load_index_document(source_root: String) -> Result<IndexDocumentDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    let entry = tree::build_index_entry(&facade).map_err(|e| ServerFnError::new(e.to_string()))?;
    let document = entry
        .as_ref()
        .and_then(|e| notez_core::domain::ResourceRef::parse(&e.ref_str).ok())
        .and_then(|r_ref| match facade.read(&r_ref) {
            Ok(Some(resource)) => {
                let mut row = ResourceRow::from(resource);
                row.body_html = crate::body::render_body(&row, &sel.root);
                Some(row)
            }
            _ => None,
        });
    Ok(IndexDocumentDto { entry, document })
}

/// Find the `index.org`/`index.md`/`README.*` document the home page
/// falls back to. `None` means the space has no index document and the
/// welcome page should fall through to a directory listing. Returns
/// only the entry; callers that need the rendered body should use
/// `load_index_document` instead.
#[server]
pub async fn resolve_index(source_root: String) -> Result<Option<IndexEntryDto>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
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
    source_root: String,
    locator: String,
) -> Result<Option<Preview>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
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
        row.body_html = crate::body::render_body(&row, &sel.root);
        let size = sel.root.join(&row.locator).metadata().map(|m| m.len()).unwrap_or(0);
        return Ok(Some(Preview {
            title: row.title,
            display_path: row.locator.clone(),
            size,
            kind: row.kind,
            body_html: row.body_html,
        }));
    }
    let file_path = sel.root.join(&locator);
    if !file_path.exists() {
        return Ok(None);
    }
    let title = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&locator)
        .to_string();
    let size = file_path.metadata().map(|m| m.len()).unwrap_or(0);
    let body_html = crate::body::render_path(&file_path, &title, &sel.root);
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
    source_root: String,
    q: String,
) -> Result<Vec<SearchHit>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    let facade = open_engine(&sel.root).map_err(|e| ServerFnError::new(e.to_string()))?;
    let facade = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
    tree::build_search(&facade, &sel.root, &q)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// Walk the space directory and return every non-hidden file. Used
// by the left-column files panel and the per-space home page so
// users can preview loose attachments (PDFs, images, archives)
// the moment they drop them in, even before the projection has
// indexed them.
#[server]
pub async fn list_filesystem(source_root: String) -> Result<Vec<SourceFileRow>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tree::build_filesystem_listing(&sel.root))
}
// ---- document editing -------------------------------------------------------

/// Raw source text for a document, returned to the edit mode so the
/// textarea can be primed with the current file body. `revision` is
/// the SHA-256 of the file content (same construction the Org /
/// Markdown scanners use), so the UI can send it back as the expected
/// revision when saving.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawDocument {
    pub content: String,
    pub revision: String,
    pub locator: String,
    pub kind: String,
}

/// Structured failure a save can return. Serialized over the wire so
/// the UI renders a specific state (StaleRevision, ReadOnly,
/// NotFound…) instead of a bare string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SaveFailure {
    /// The file changed between the time the UI loaded it and the save.
    StaleRevision { expected: String, actual: String },
    /// The source does not accept writes.
    ReadOnly { reason: String },
    /// The underlying file no longer exists.
    NotFound { path: String },
    /// The target is not a document, so raw text editing is unsafe.
    Unsupported { reason: String },
    /// Anything else.
    Internal { message: String },
}

/// Result of a document save. `Saved` carries the fresh row so the UI
/// can re-render without a full navigate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SaveOutcome {
    Saved { row: ResourceRow, revision: String },
    Failed(SaveFailure),
}

#[server]
pub async fn read_document_content(
    source_root: String,
    ref_str: String,
) -> Result<Option<RawDocument>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root)).map_err(WebServerError::from).map_err(|e| ServerFnError::new(e.to_string()))?;
    let r_ref = ResourceRef::parse(&ref_str).map_err(|e| ServerFnError::new(format!("invalid ref {ref_str}: {e}")))?;
    read_document_content_impl(&sel.root, &r_ref).map_err(|e| ServerFnError::new(e.to_string()))
}

/// Evaluate Janet through the shared protocol dispatcher.
#[server]
pub async fn janet_eval(script: String) -> Result<serde_json::Value, ServerFnError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let root = std::env::current_dir().map_err(|e| ServerFnError::new(e.to_string()))?;
        let facade = open_engine(&root).map_err(ServerFnError::new)?;
        let mut guard = facade.lock().map_err(|e| ServerFnError::new(format!("facade lock: {e}")))?;
        let req = ExecuteJanetRequest { script, source_id: None, document_ref: None, actor_id: "web".into(), expected_revision: None, trace_id: None, timeout_ms: notez_core::application::DEFAULT_TIMEOUT_MS, result_limit: notez_core::application::DEFAULT_RESULT_LIMIT };
        match ApplicationDispatcher::new(&mut *guard).dispatch(Request::ExecuteJanet(req)).map_err(|e| ServerFnError::new(e.to_string()))? {
            Response::Janet(result) => Ok(result.value),
            other => Err(ServerFnError::new(format!("unexpected response: {other:?}"))),
        }
    }
    #[cfg(target_arch = "wasm32")]
    { let _ = script; Err(ServerFnError::new("server-only".to_string())) }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn update_document(
    source_root: String, ref_str: String, expected_revision: String, content: String,
) -> Result<SaveOutcome, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root)).map_err(WebServerError::from).map_err(|e| ServerFnError::new(e.to_string()))?;
    let r_ref = ResourceRef::parse(&ref_str).map_err(|e| ServerFnError::new(format!("invalid ref {ref_str}: {e}")))?;
    update_document_impl(&sel.root, &r_ref, &expected_revision, &content).map_err(|e| ServerFnError::new(e.to_string()))
}
#[cfg(not(target_arch = "wasm32"))]
pub fn read_document_content_impl(source_root: &Path, r_ref: &ResourceRef) -> Result<Option<RawDocument>, String> {
    let res = dispatch_read(source_root, r_ref)?;
    let res = match res { Some(r) => r, None => return Ok(None) };
    if res.kind != ResourceKind::Document { return Ok(None); }
    let full = source_root.join(&res.locator);
    let bytes = std::fs::read(&full).map_err(|e| format!("read {}: {e}", full.display()))?;
    Ok(Some(RawDocument { content: String::from_utf8_lossy(&bytes).into_owned(), revision: sha256_hex(&bytes), locator: res.locator, kind: res.kind.as_str().to_string() }))
}

#[cfg(target_arch = "wasm32")]
pub fn read_document_content_impl(_source_root: &Path, _r_ref: &ResourceRef) -> Result<Option<RawDocument>, String> { Err("server-only".to_string()) }

#[cfg(not(target_arch = "wasm32"))]
pub fn update_document_impl(
    source_root: &Path, r_ref: &ResourceRef, expected_revision: &str, content: &str,
) -> Result<SaveOutcome, String> {
    let res = dispatch_read(source_root, r_ref)?;
    let res = match res { Some(r) => r, None => return Ok(SaveOutcome::Failed(SaveFailure::NotFound { path: source_root.display().to_string() })) };
    if res.kind != ResourceKind::Document { return Ok(SaveOutcome::Failed(SaveFailure::Unsupported { reason: format!("target {} is not a document", res.kind.as_str()) })); }
    let full = source_root.join(&res.locator);
    let canonical_space = std::fs::canonicalize(source_root).unwrap_or_else(|_| source_root.to_path_buf());
    let canonical_file = std::fs::canonicalize(&full).unwrap_or_else(|_| full.clone());
    if !canonical_file.starts_with(&canonical_space) || !full.is_file() { return Ok(SaveOutcome::Failed(SaveFailure::NotFound { path: full.display().to_string() })); }
    let facade = open_engine(source_root)?;
    let mut guard = facade.lock().map_err(|e| format!("facade lock: {e}"))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let response = dispatcher.dispatch(Request::UpdateDocument(UpdateDocumentRequest { source_id: res.source_id, locator: res.locator, content: content.to_string(), base_revision: Some(expected_revision.to_string()), format: None, expected_revision: Some(expected_revision.to_string()) }));
    match response {
        Ok(Response::DocumentUpdated(report)) => {
            drop(dispatcher);
            drop(guard);
            let updated = dispatch_read(source_root, r_ref)?.ok_or_else(|| "updated document disappeared".to_string())?;
            let mut row = ResourceRow::from(updated);
            row.body_html = render_body(&row, source_root);
            Ok(SaveOutcome::Saved { revision: report.revision, row })
        }
        Err(notez_core::application::ApplicationError::RevisionConflict { expected, actual }) => Ok(SaveOutcome::Failed(SaveFailure::StaleRevision { expected, actual })),
        Err(notez_core::application::ApplicationError::ReadOnlySource { source_id }) => Ok(SaveOutcome::Failed(SaveFailure::ReadOnly { reason: format!("source is read-only: {source_id}") })),
        Err(e) => Ok(SaveOutcome::Failed(SaveFailure::Internal { message: e.to_string() })),
        Ok(other) => Err(format!("unexpected update response: {other:?}")),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn update_document_impl(
    _source_root: &Path,
    _r_ref: &ResourceRef,
    _expected_revision: &str,
    _content: &str,
) -> Result<SaveOutcome, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

// ---- _impl helpers (unit-testable) -----------------------------------------

/// Open the projection store + facade for `source_root`, going through
/// the `WebState` cache so repeat calls reuse the same SQLite handle.
#[cfg(not(target_arch = "wasm32"))]
fn open_engine(source_root: &Path) -> Result<std::sync::Arc<std::sync::Mutex<Engine<SqliteProjection>>>, String> {
    let state = crate::routes::state_snapshot();
    state.engine_for(&source_root.to_path_buf()).map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn list_resources_impl(source_root: &Path) -> Result<Vec<ResourceRow>, String> {
    let resources = dispatch_query(source_root)?;
    Ok(resources.into_iter().map(ResourceRow::from).collect())
}

#[cfg(target_arch = "wasm32")]
pub async fn list_resources_impl(_space_root: &Path) -> Result<Vec<ResourceRow>, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_resource_impl(
    source_root: &Path,
    ref_str: &str,
) -> Result<Option<ResourceRow>, String> {
    let r_ref = ResourceRef::parse(ref_str).map_err(|e| WebServerError::InvalidRef { raw: ref_str.to_string(), message: e.to_string() }.to_string())?;
    let sel = core_resolve_space(source_root).map_err(|e| e.to_string())?;
    let res = dispatch_read(&sel.root, &r_ref)?;
    Ok(res.map(|r| { let mut row = ResourceRow::from(r); row.body_html = render_body(&row, &sel.root); row }))
}

#[cfg(target_arch = "wasm32")]
pub async fn get_resource_impl(_space_root: &Path, _ref_str: &str) -> Result<Option<ResourceRow>, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn list_graph_impl(source_root: &Path) -> Result<Graph, String> {
    let engine = open_engine(source_root)?;
    let guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    Graph::from_facade(&guard).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn list_graph_impl(_space_root: &Path) -> Result<Graph, String> {
    Err("server-only".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn neighbor_graph_impl(source_root: &Path, focus_ref: &str) -> Result<Graph, String> {
    let engine = open_engine(source_root)?;
    let guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    Graph::neighborhood(&guard, focus_ref).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn neighbor_graph_impl(_space_root: &Path, _focus_ref: &str) -> Result<Graph, String> {
    Err("server-only".to_string())
}

// ---- v2 note workspace: links / outline / journal / UI config --------------

/// One incoming or outgoing link row for the note-page right rail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRow {
    pub ref_str: String,
    pub title: String,
    pub kind: String,
}

/// Backlinks (incoming) and outgoing links for one resource. Both
/// directions come from the same resolved-relations query; a row whose
/// backing resource is gone renders with kind `missing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LinksDto {
    pub incoming: Vec<LinkRow>,
    pub outgoing: Vec<LinkRow>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_links_impl(source_root: &Path, ref_str: &str) -> Result<LinksDto, String> {
    use std::collections::BTreeSet;

    let engine = open_engine(source_root)?;
    let guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let page = guard.query(&Selector::new()).map_err(|e| e.to_string())?;
    let focus = ref_str
        .parse::<ResourceRef>()
        .map_err(|e| format!("bad ref `{ref_str}`: {e}"))?;
    let relations = guard
        .query_resolved_relations(&focus)
        .map_err(|e| e.to_string())?;

    let row_for = |other: &str| -> LinkRow {
        page.items
            .iter()
            .find(|item| item.r#ref.to_string() == other)
            .map(|item| LinkRow {
                ref_str: other.to_string(),
                title: item.title.clone(),
                kind: item.kind.to_string(),
            })
            .unwrap_or_else(|| LinkRow {
                ref_str: other.to_string(),
                title: other.to_string(),
                kind: "missing".to_string(),
            })
    };

    let mut dto = LinksDto::default();
    let mut seen_in: BTreeSet<String> = BTreeSet::new();
    let mut seen_out: BTreeSet<String> = BTreeSet::new();
    for rel in relations {
        let s = rel.source_ref.to_string();
        let t = rel.target_ref.to_string();
        if t == ref_str && s != ref_str && seen_in.insert(s.clone()) {
            dto.incoming.push(row_for(&s));
        } else if s == ref_str && t != ref_str && seen_out.insert(t.clone()) {
            dto.outgoing.push(row_for(&t));
        }
    }
    Ok(dto)
}

#[cfg(target_arch = "wasm32")]
pub fn list_links_impl(_source_root: &Path, _ref_str: &str) -> Result<LinksDto, String> {
    Err("server-only".to_string())
}

#[server]
pub async fn list_links(source_root: String, ref_str: String) -> Result<LinksDto, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    list_links_impl(&sel.root, &ref_str).map_err(|e| ServerFnError::new(e))
}

/// One heading in the document outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineItemDto {
    /// Heading level, 1..=6.
    pub level: u8,
    pub text: String,
}

/// Extract the heading outline from raw source text. `org` selects the
/// `* stars` syntax; otherwise Markdown `# hashes` are parsed. Fenced
/// code blocks and Org `#+begin_src` blocks are skipped; frontmatter is
/// skipped.
pub fn parse_outline(content: &str, org: bool) -> Vec<OutlineItemDto> {
    let mut items: Vec<OutlineItemDto> = Vec::new();
    let mut fence: Option<String> = None;
    let mut org_block = false;
    let mut in_frontmatter = false;
    let mut first_content = true;
    for raw in content.lines() {
        let trimmed = raw.trim_end();
        let t = trimmed.trim_start();
        if first_content && !t.is_empty() {
            first_content = false;
            if t == "---" {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter {
            if t == "---" || t == "..." {
                in_frontmatter = false;
            }
            continue;
        }
        if let Some(f) = &fence {
            if t.starts_with(f.as_str()) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some("```".to_string());
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some("~~~".to_string());
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if org_block {
            if lower.starts_with("#+end_src") || lower.starts_with("#+end_example") {
                org_block = false;
            }
            continue;
        }
        if lower.starts_with("#+begin_src") || lower.starts_with("#+begin_example") {
            org_block = true;
            continue;
        }
        let marker = if org { '*' } else { '#' };
        if !t.starts_with(marker) {
            continue;
        }
        let marks = t.len() - t.trim_start_matches(marker).len();
        if !(1..=6).contains(&marks) {
            continue;
        }
        let rest = &t[marks..];
        if !rest.starts_with(' ') || rest.starts_with("  ") {
            continue;
        }
        let text = rest.trim().trim_end_matches(&marker.to_string()).trim_end();
        if text.is_empty() {
            continue;
        }
        items.push(OutlineItemDto {
            level: marks as u8,
            text: text.to_string(),
        });
    }
    items
}

#[server]
pub async fn get_outline(
    source_root: String,
    ref_str: String,
) -> Result<Vec<OutlineItemDto>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let raw = read_document_content(sel.root.to_string_lossy().into_owned(), ref_str.clone())
        .await
        .ok()
        .flatten();
    let Some(doc) = raw else {
        return Ok(Vec::new());
    };
    let ext = std::path::Path::new(&doc.locator)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(parse_outline(&doc.content, ext == "org"))
}

/// Today's date as `YYYY-MM-DD` (UTC).
pub fn today_iso_date() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 →
/// (year, month, day). <http://howardhinnant.github.io/date_algorithms.html>
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Locator of today's journal note: `journal/YYYY-MM-DD.md`.
pub fn today_journal_locator() -> String {
    format!("journal/{}.md", today_iso_date())
}

/// `journal/2026-09-01.md` → `Some("2026-09-01")`. Only `journal/`
/// files whose stem is a strict ISO date count as journal entries.
pub fn journal_date_of(display_path: &str) -> Option<String> {
    let name = display_path.rsplit('/').next()?;
    let (stem, ext) = name.rsplit_once('.')?;
    if !matches!(ext, "md" | "org") {
        return None;
    }
    let mut parts = stem.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if digits(y, 4) && digits(m, 2) && digits(d, 2) {
        Some(stem.to_string())
    } else {
        None
    }
}

/// One journal entry row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntryDto {
    /// ISO date `YYYY-MM-DD` parsed from the filename.
    pub date: String,
    pub locator: String,
    pub title: String,
    /// Empty for loose (not yet scanned) files — the UI links those to
    /// the preview page instead of the note page.
    pub ref_str: String,
    pub mtime_ms: u64,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_journal_impl(source_root: &Path) -> Result<Vec<JournalEntryDto>, String> {
    let engine = open_engine(source_root)?;
    let guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let files = tree::build_source_files(&guard, source_root).map_err(|e| e.to_string())?;
    let mut entries: Vec<JournalEntryDto> = files
        .into_iter()
        .filter_map(|f| {
            journal_date_of(&f.display_path).map(|date| JournalEntryDto {
                date,
                locator: f.display_path.clone(),
                title: f.title,
                ref_str: f.ref_str,
                mtime_ms: f.mtime_ms,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(entries)
}

#[cfg(target_arch = "wasm32")]
pub fn list_journal_impl(_source_root: &Path) -> Result<Vec<JournalEntryDto>, String> {
    Err("server-only".to_string())
}

#[server]
pub async fn list_journal(source_root: String) -> Result<Vec<JournalEntryDto>, ServerFnError> {
    let sel = core_resolve_space(Path::new(&source_root))
        .map_err(WebServerError::from)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    auto_start_watch(&sel.root);
    list_journal_impl(&sel.root).map_err(|e| ServerFnError::new(e))
}

/// Landing mode + starred refs for one space, read from `web.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceUiDto {
    pub landing: String,
    pub starred: Vec<String>,
}

#[server]
pub async fn get_space_ui(source_root: String) -> Result<SpaceUiDto, ServerFnError> {
    let cfg = crate::ui_config::load_web_ui_config();
    Ok(SpaceUiDto {
        landing: cfg.landing_for(&source_root).to_string(),
        starred: cfg
            .starred
            .get(&source_root)
            .cloned()
            .unwrap_or_default(),
    })
}

/// Visible home dashboard widgets in display order.
#[server]
pub async fn get_home_widgets() -> Result<Vec<String>, ServerFnError> {
    Ok(crate::ui_config::load_web_ui_config().visible_home_widgets())
}

/// One projected notez card on the dashboard. Pure data; renderers
/// consume this from every surface (Web, CLI, future remote client).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NotezCardView {
    pub id: String,
    pub title: String,
    pub state: String,
    /// `json` | `list` | `object` | `html`
    pub output_type: String,
    pub locator: String,
    pub ordinal: usize,
    /// JSON body for `output_type = json`; ignored otherwise.
    pub value: Option<serde_json::Value>,
    /// List body for `output_type = list`; ignored otherwise.
    pub items: Vec<serde_json::Value>,
    /// Reference for `output_type = object`; ignored otherwise.
    pub object_ref: Option<String>,
    /// Sanitized HTML for `output_type = html`; ignored otherwise.
    pub html: Option<String>,
    /// Failure message for `state = failed`; ignored otherwise.
    pub error: Option<String>,
    pub error_kind: Option<String>,
}

/// One disk-scanned notez block: ready to project or filter.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NotezDefinitionView {
    pub id: String,
    pub title: String,
    pub language: String,
    pub format: String,
    pub declared_output: Option<String>,
    pub locator: String,
    pub ordinal: usize,
}

impl NotezCardView {
        fn ready(block: &notez_core::document::NotezBlock, output: &notez_core::document::CardOutput, locator: String) -> Self {
        let (output_type, value, items, object_ref, html) = match output {
            notez_core::document::CardOutput::Json(value) => ("json".to_string(), Some(value.clone()), Vec::new(), None, None),
            notez_core::document::CardOutput::List(items) => ("list".to_string(), None, items.clone(), None, None),
            notez_core::document::CardOutput::Object { reference } => ("object".to_string(), None, Vec::new(), Some(reference.clone()), None),
            notez_core::document::CardOutput::Html(html) => ("html".to_string(), None, Vec::new(), None, Some(html.as_str().to_string())),
        };
        Self {
            id: block.id.clone(),
            title: block.attrs.get("title").cloned().unwrap_or_else(|| block.id.clone()),
            state: "ready".to_string(),
            output_type,
            locator,
            ordinal: block.ordinal,
            value,
            items,
            object_ref,
            html,
            error: None,
            error_kind: None,
        }
    }

    fn failed(block: &notez_core::document::NotezBlock, error: &crate::janet::JanetScriptError, locator: String) -> Self {
        Self {
            id: block.id.clone(),
            title: block.attrs.get("title").cloned().unwrap_or_else(|| block.id.clone()),
            state: "failed".to_string(),
            output_type: String::new(),
            locator,
            ordinal: block.ordinal,
            value: None,
            items: Vec::new(),
            object_ref: None,
            html: None,
            error: Some(error.to_string()),
            error_kind: Some(error.kind().to_string()),
        }
    }
}

/// Project the primary space's notez cards. Bounded scan: at most
/// 50 candidate files and at most 8 executed cards per dashboard
/// render, so a misconfigured vault cannot wedge the home page.
#[cfg(not(target_arch = "wasm32"))]
pub fn collect_dashboard_cards(root: &Path) -> Vec<NotezCardView> {
    use std::collections::BTreeMap;
    const MAX_CARDS: usize = 8;
    let policy = notez_core::source::policy::SourcePolicy::load_for_root(root);
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    walk_for_cards(root, root, &policy, &mut files);
    files.truncate(50);
    let mut by_id: BTreeMap<String, NotezCardView> = BTreeMap::new();
    for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let locator = path
            .strip_prefix(root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let blocks = match path.extension().and_then(|e| e.to_str()) {
            Some("org") => notez_core::document::parse_org_notez_blocks(&text),
            _ => notez_core::document::parse_markdown_notez_blocks(&text),
        };
        for block in blocks {
            // First card wins: same id from a second file becomes a
            // visible duplicate in the dashboard layout and would also
            // collide on cache keys.
            if by_id.contains_key(&block.id) {
                continue;
            }
            if by_id.len() >= MAX_CARDS {
                return by_id.into_values().collect();
            }
            let view = match crate::janet::execute_card(&block) {
                Ok(output) => NotezCardView::ready(&block, &output, locator.clone()),
                Err(error) => NotezCardView::failed(&block, &error, locator.clone()),
            };
            by_id.insert(block.id.clone(), view);
        }
    }
    by_id.into_values().collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn walk_for_cards(
    root: &Path,
    dir: &Path,
    policy: &notez_core::source::policy::SourcePolicy,
    out: &mut Vec<std::path::PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let is_symlink = file_type.is_symlink()
            || std::fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if file_type.is_dir() {
            if policy.hides_dir(&rel, is_symlink) {
                continue;
            }
            walk_for_cards(root, &path, policy, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "md" | "markdown" | "org") {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if let notez_core::source::policy::Decision::Allow = policy.decide(&rel, is_symlink, size) {
                candidates.push(path);
            }
        }
    }
    out.extend(candidates);
}

/// Dashboard server function: project notez cards for the first
/// registered source's home.
#[cfg(not(target_arch = "wasm32"))]
#[server]
pub async fn list_dashboard_cards() -> Result<Vec<NotezCardView>, ServerFnError> {
    let spaces = list_registered_spaces().await.unwrap_or_default();
    let Some(primary) = spaces.into_iter().next() else {
        return Ok(Vec::new());
    };
    let root = std::path::PathBuf::from(&primary.path);
    Ok(collect_dashboard_cards(&root))
}



// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use notez_core::domain::audit::{AuditLog, AuditOutcome, AuditRecord};
    use notez_core::domain::change::{Actor, Change};
    use notez_core::domain::journal::EventJournal;
    use notez_core::domain::{ResourceKind, ResourceRef};
    use notez_core::storage::{SqliteAuditLog, SqliteEventJournal};

    fn write_minimal_space(root: &Path) {
        fs::create_dir_all(root.join(".notez")).unwrap();
        fs::write(
            root.join("notez.toml"),
            format!(
                "version = 2\n\n[source]\nname = \"{}\"\ndatabase = \".notez/index.sqlite\"\n",
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
            err.contains("does not exist") || err.contains("source not found"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn list_resources_impl_rejects_plain_directory() {
        let tmp = tempdir().unwrap();
        let plain = tmp.path().join("plain");
        fs::create_dir_all(&plain).unwrap();
        let result = list_resources_impl(&plain).await;
        assert!(result.is_ok(), "a directory without notez.toml is treated as an empty source");
        assert!(result.unwrap().is_empty());
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

    // ---- activity DTO conversion --------------------------------------------

    #[test]
    fn activity_action_label_covers_every_op() {
        assert_eq!(activity_action_label(&ChangeOp::UpsertResource), "upsert resource");
        assert_eq!(activity_action_label(&ChangeOp::DeleteResource), "delete resource");
        assert_eq!(activity_action_label(&ChangeOp::InsertSegments), "extract segments");
        assert_eq!(activity_action_label(&ChangeOp::ReplaceLinkOccurrences), "index link occurrences");
        assert_eq!(activity_action_label(&ChangeOp::ReplaceResolvedRelations), "resolve links");
        assert_eq!(activity_action_label(&ChangeOp::WriteLinkDiagnostics), "link diagnostics");
        assert_eq!(activity_action_label(&ChangeOp::ReplaceConflicts), "replace conflicts");
        assert_eq!(activity_action_label(&ChangeOp::Writeback), "document write");
        assert_eq!(activity_action_label(&ChangeOp::Scan), "scan");
        assert_eq!(activity_action_label(&ChangeOp::Rebuilt), "rebuild index");
        assert_eq!(
            activity_action_label(&ChangeOp::TransitionTask {
                from_state: "TODO".into(),
                to_state: "DONE".into(),
                timestamp: "t".into(),
                closed_timestamp: None,
                logbook_entry: String::new(),
            }),
            "task transition: TODO → DONE"
        );
    }


    // ---- activity impl (durable journal) ------------------------------------

    #[test]
    fn list_space_activity_impl_reads_durable_journal() {
        let tmp = tempdir().unwrap();
        let space = tmp.path().join("work");
        write_minimal_space(&space);
        let db_path = space.join(".notez/index.sqlite");
        let journal = SqliteEventJournal::new(SqliteProjection::open_for_adapter(&db_path).unwrap());
        let change = Change {
            id: Change::now_id(),
            actor: Actor::new("web"),
            at_unix_millis: 1_700_000_000_000,
            source_id: "native".to_string(),
            op: ChangeOp::Scan,
            targets: vec![],
            expected_revision: None,
            payload: serde_json::json!({ "resource_count": 0 }),
        };
        journal.append(&change).unwrap();
        // No audit row for this change → `audited` stays false.
        let entries = list_space_activity_impl(&space, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "scan");
        assert_eq!(entries[0].actor, "web");
        assert_eq!(entries[0].source_id, "native");
        assert_eq!(entries[0].at_unix_millis, 1_700_000_000_000);
        assert_eq!(entries[0].sequence, 1);
        assert!(!entries[0].audited);
    }

    #[test]
    fn list_space_activity_impl_flags_audited_changes() {
        let tmp = tempdir().unwrap();
        let space = tmp.path().join("work");
        write_minimal_space(&space);
        let db_path = space.join(".notez/index.sqlite");
        let journal = SqliteEventJournal::new(SqliteProjection::open_for_adapter(&db_path).unwrap());
        let audit = SqliteAuditLog::new(SqliteProjection::open_for_adapter(&db_path).unwrap());
        let change = Change {
            id: Change::now_id(),
            actor: Actor::new("cli"),
            at_unix_millis: 1_700_000_000_000,
            source_id: "native".to_string(),
            op: ChangeOp::Writeback,
            targets: vec![ResourceRef::new(ResourceKind::Document, ulid::Ulid::new())],
            expected_revision: Some("rev1".to_string()),
            payload: serde_json::json!({ "locator": "a.org" }),
        };
        journal.append(&change).unwrap();
        audit
            .append(AuditRecord {
                change_id: change.id,
                principal: "cli".to_string(),
                action: "Writeback".to_string(),
                target: change.targets[0].clone(),
                outcome: AuditOutcome::Success,
                recorded_at_unix_millis: change.at_unix_millis,
            })
            .unwrap();
        let entries = list_space_activity_impl(&space, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].audited);
        assert!(entries[0].target.is_some());
        assert_eq!(entries[0].revision.as_deref(), Some("rev1"));
    }

    #[test]
    fn list_space_activity_impl_respects_limit_and_newest_first() {
        let tmp = tempdir().unwrap();
        let space = tmp.path().join("work");
        write_minimal_space(&space);
        let db_path = space.join(".notez/index.sqlite");
        let journal = SqliteEventJournal::new(SqliteProjection::open_for_adapter(&db_path).unwrap());
        for i in 0..5 {
            journal
                .append(&Change {
                    id: Change::now_id(),
                    actor: Actor::new("web"),
                    at_unix_millis: 1_700_000_000_000 + i,
                    source_id: "native".to_string(),
                    op: ChangeOp::Scan,
                    targets: vec![],
                    expected_revision: None,
                    payload: serde_json::Value::Null,
                })
                .unwrap();
        }
        let entries = list_space_activity_impl(&space, 2).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].at_unix_millis > entries[1].at_unix_millis);
    }

    #[test]
    fn list_space_activity_impl_rejects_missing_space() {
        let tmp = tempdir().unwrap();
        let err = list_space_activity_impl(&tmp.path().join("nope"), 10)
            .expect_err("missing path must fail");
        assert!(
            err.contains("does not exist") || err.contains("source not found"),
            "got: {err}"
        );
    }
}
