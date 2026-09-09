//! Space resolution and data access for the workspace UI.
//!
//! A *space* is a notez source root (a directory with an optional
//! `notez.toml`). The UI addresses it by its base64url-encoded
//! absolute path so a single server can serve several spaces without
//! per-space state.
//!
//! Every read here goes through the same layers the rest of the app
//! uses: the composition `Runtime` engine cache (via
//! [`crate::routes::WebState`]) for the projection, and the
//! `ApplicationDispatcher` for writes — no direct projection or file
//! writes bypass the protocol spine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use notez_core::application::dispatcher::{ApplicationDispatcher, Response as DispatchResponse};
use notez_core::config::web_space::{list_sources, WebSourceError};
use notez_core::domain::Resource;
use notez_protocol::request::{Request, ScanNativeRequest, UpdateDocumentRequest};

use crate::routes::WebState;
use crate::server::{SaveFailure, SaveOutcome};

/// A resolved space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    pub root: PathBuf,
    pub name: String,
    pub encoded: String,
}

/// One file the space owns, as listed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub locator: String,
    pub title: String,
    /// `document` | `attachment`
    pub kind: String,
    pub ext: String,
    pub size: u64,
    pub mtime_ms: u64,
}

impl Entry {
    /// Documents are the files the editor can write back (the parser
    /// registry only knows markdown and org).
    pub fn editable(&self) -> bool {
        self.kind == "document"
    }

    pub fn dir(&self) -> &str {
        match self.locator.rfind('/') {
            Some(i) => &self.locator[..i],
            None => "",
        }
    }

    pub fn file_name(&self) -> &str {
        match self.locator.rfind('/') {
            Some(i) => &self.locator[i + 1..],
            None => &self.locator,
        }
    }
}

/// Errors surfaced to the browser as a plain page.
#[derive(Debug)]
pub enum UiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl UiError {
    pub fn status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            UiError::NotFound(_) => StatusCode::NOT_FOUND,
            UiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            UiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            UiError::NotFound(m) | UiError::BadRequest(m) | UiError::Internal(m) => m,
        }
    }
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl From<WebSourceError> for UiError {
    fn from(e: WebSourceError) -> Self {
        UiError::NotFound(e.to_string())
    }
}

fn state() -> WebState {
    crate::routes::state_snapshot()
}

/// Open the space addressed by a URL segment.
pub fn open(encoded: &str) -> Result<Space, UiError> {
    let root_str = crate::data::urls::decode_space(encoded);
    if root_str.is_empty() {
        return Err(UiError::BadRequest("malformed space segment".into()));
    }
    let root = PathBuf::from(&root_str);
    if !root.is_dir() {
        return Err(UiError::NotFound(format!("space not found: {root_str}")));
    }
    Ok(space_from_root(root, None))
}

/// Spaces the picker can offer: the global registrations first, then
/// any source discovered upward from the current directory.
pub fn list_spaces() -> Vec<Space> {
    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut out: Vec<Space> = Vec::new();
    if let Ok(listed) = list_sources(&env, &cwd) {
        for s in listed {
            let root_str = s.root.to_string_lossy().into_owned();
            out.push(Space {
                name: s.name,
                encoded: crate::data::urls::encode_space(&root_str),
                root: s.root,
            });
        }
    }
    out
}

/// The space to land on when the user opens `/`.
///
/// Order: explicit env pin, then the global config's `default_source`,
/// then the only listed space, then `None` (the picker is shown).
pub fn default_space() -> Option<Space> {
    for var in ["NOTEZ_SPACE_ROOT", "NOTEZ_DEFAULT_SPACE"] {
        if let Ok(p) = std::env::var(var) {
            if !p.trim().is_empty() {
                let root = PathBuf::from(p.trim());
                if root.is_dir() {
                    return Some(space_from_root(root, None));
                }
            }
        }
    }
    if let Some(space) = configured_default_space() {
        return Some(space);
    }
    let spaces = list_spaces();
    if spaces.len() == 1 { spaces.into_iter().next() } else { None }
}

/// The `default_source` from the global config, when it still resolves
/// to a directory. This is what the user picked in `notez source add`,
/// so it outranks "the only source discovered from the cwd".
fn configured_default_space() -> Option<Space> {
    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = notez_core::config::ConfigPaths::discover(&env, &cwd).ok()?;
    let global = paths.global_config?;
    let name = global.default_source.clone()?;
    let registered = global.sources.get(&name)?;
    if !registered.path.is_dir() {
        return None;
    }
    Some(space_from_root(registered.path.clone(), Some(name)))
}

fn space_from_root(root: PathBuf, name: Option<String>) -> Space {
    let root_str = root.to_string_lossy().into_owned();
    let name = name.unwrap_or_else(|| {
        root.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&root_str)
            .to_string()
    });
    Space { encoded: crate::data::urls::encode_space(&root_str), root, name }
}

/// Open the space's engine handle (cached per root by the Runtime).
#[allow(clippy::type_complexity)]
fn engine(
    root: &Path,
) -> Result<std::sync::Arc<std::sync::Mutex<notez_core::application::Engine<notez_core::storage::SqliteProjection>>>, UiError> {
    state()
        .engine_for(&root.to_path_buf())
        .map_err(UiError::Internal)
}

/// Every indexed resource in the space.
pub fn resources(root: &Path) -> Result<Vec<Resource>, UiError> {
    crate::server::dispatch_query(root).map_err(UiError::Internal)
}

/// The indexed resource backing `locator`, if the projection knows it.
///
/// Headings and blocks share their file's locator, so the document row
/// (or the attachment row) wins over them.
pub fn resource_for(root: &Path, locator: &str) -> Result<Option<Resource>, UiError> {
    let mut fallback: Option<Resource> = None;
    for resource in resources(root)? {
        if resource.locator != locator {
            continue;
        }
        let is_file_row = matches!(
            resource.kind,
            notez_core::domain::ResourceKind::Document
                | notez_core::domain::ResourceKind::Attachment
        );
        if is_file_row {
            return Ok(Some(resource));
        }
        if fallback.is_none() {
            fallback = Some(resource);
        }
    }
    Ok(fallback)
}

/// Page list: every non-hidden file on disk, enriched with the
/// projection's title and kind where the file is indexed.
pub fn entries(root: &Path) -> Result<Vec<Entry>, UiError> {
    let loose = list_files(root);
    let indexed = index_by_locator(resources(root).unwrap_or_default());

    let mut out: Vec<Entry> = loose
        .into_iter()
        .map(|f| {
            let (title, kind) = match indexed.get(&f.locator) {
                Some((t, k)) if !t.trim().is_empty() => (t.clone(), k.clone()),
                _ => (f.title.clone(), f.kind.clone()),
            };
            Entry {
                locator: f.locator,
                title,
                kind,
                ext: f.ext,
                size: f.size,
                mtime_ms: f.mtime_ms,
            }
        })
        .collect();
    out.sort_by(|a, b| a.locator.to_lowercase().cmp(&b.locator.to_lowercase()));
    Ok(out)
}

/// Every file the space owns: documents *and* attachments.
///
/// The walk applies the Source's structural rules (hidden paths,
/// `exclude` globs, symlinks) but not the indexer's `include` globs or
/// size cap — a 30 MB PDF is still a file the user must be able to see
/// and preview.
fn list_files(root: &Path) -> Vec<Entry> {
    const MAX_FILES: usize = 20_000;
    let policy = notez_core::source::policy::SourcePolicy::load_for_root(root);
    let mut out: Vec<Entry> = Vec::new();
    walk(root, root, &policy, &mut out);
    if out.len() > MAX_FILES {
        out.truncate(MAX_FILES);
    }
    out
}

fn walk(
    root: &Path,
    dir: &Path,
    policy: &notez_core::source::policy::SourcePolicy,
    out: &mut Vec<Entry>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let is_symlink = file_type.is_symlink();
        if file_type.is_dir() {
            if !policy.hides_dir(rel, is_symlink) {
                walk(root, &path, policy, out);
            }
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let locator = rel.to_string_lossy().replace('\\', "/");
        if locator.is_empty() {
            continue;
        }
        let rel_path = Path::new(&locator);
        if let notez_core::source::policy::Decision::Ignore(_) = policy.decide_file(rel_path, is_symlink)
        {
            continue;
        }
        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime_ms = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let title = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&locator)
            .to_string();
        out.push(Entry {
            locator,
            title,
            kind: "attachment".to_string(),
            ext,
            size,
            mtime_ms,
        });
    }
}

/// Map locator → (title, kind) for the file-level rows only.
///
/// Headings and blocks share their file's locator; letting them into
/// the map would shadow the document row and misclassify every note
/// that has headings as an attachment.
fn index_by_locator(resources: Vec<Resource>) -> HashMap<String, (String, String)> {
    resources
        .into_iter()
        .filter(|r| {
            !r.locator.is_empty()
                && matches!(
                    r.kind,
                    notez_core::domain::ResourceKind::Document
                        | notez_core::domain::ResourceKind::Attachment
                )
        })
        .map(|r| {
            (
                r.locator.clone(),
                (r.title.clone(), r.kind.as_str().to_string()),
            )
        })
        .collect()
}

/// Read a document's raw text plus its revision (sha256 of the bytes),
/// which is the precondition the write path expects.
pub fn read_doc(root: &Path, locator: &str) -> Result<(String, String), UiError> {
    let full = root.join(locator);
    let bytes = std::fs::read(&full)
        .map_err(|e| UiError::NotFound(format!("cannot read {}: {e}", full.display())))?;
    let revision = crate::server::sha256_hex(&bytes);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), revision))
}

/// Render a document body (with relative links rewritten to notez URLs).
///
/// The editor's live preview and the read page call this with the same
/// input, so they can never disagree.
pub fn render_document(space: &Space, locator: &str, content: &str) -> Result<String, UiError> {
    let full = safe_join(&space.root, locator)?;
    let title = doc_title(&space.root, locator, content);
    let row = document_row(locator, &title);
    // `render_body` reads the on-disk bytes for the previewer context;
    // for the live preview we want the *edited* text, so seed the body
    // property explicitly.
    let mut row = row;
    row.properties.insert("body".to_string(), content.to_string());
    let html = crate::body::render_body(&row, &space.root);
    let dir = crate::data::urls::split_locator(locator).0;
    let _ = full;
    Ok(crate::data::html::rewrite_local_urls(&html, &space.encoded, dir))
}

/// Build the `ResourceRow` the previewer needs for `locator`.
pub fn document_row(locator: &str, title: &str) -> crate::model::ResourceRow {
    crate::model::ResourceRow {
        ref_str: String::new(),
        kind: "document".to_string(),
        title: title.to_string(),
        source_id: String::new(),
        locator: locator.to_string(),
        object_id: String::new(),
        revision: String::new(),
        properties: std::collections::BTreeMap::new(),
        body_html: String::new(),
        raw_content: String::new(),
    }
}

/// Render an attachment (non-document) file.
pub fn render_file(space: &Space, locator: &str, title: &str) -> Result<String, UiError> {
    let full = safe_join(&space.root, locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let html = crate::body::render_path(&full, title, &space.root);
    let dir = crate::data::urls::split_locator(locator).0;
    Ok(crate::data::html::rewrite_local_urls(&html, &space.encoded, dir))
}

/// Title for a document: `#+TITLE`/`# ` heading when present, else the
/// first `* ` heading, else the file name.
pub fn doc_title(root: &Path, locator: &str, content: &str) -> String {
    for line in content.lines().take(80) {
        let t = line.trim();
        for prefix in ["#+TITLE:", "# ", "* "] {
            if let Some(rest) = t.strip_prefix(prefix) {
                let v = rest.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    let stem = Path::new(locator)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(locator);
    if stem == "README" || stem == "index" {
        if let Some(parent) = Path::new(locator).parent().and_then(|p| p.file_name()) {
            return parent.to_string_lossy().into_owned();
        }
        if let Some(name) = root.file_name().and_then(|s| s.to_str()) {
            return name.to_string();
        }
    }
    stem.to_string()
}

/// Save a document through the protocol write spine (revision guard +
/// journal + projection refresh). The locator must be indexed as a
/// document; a not-yet-indexed file is scanned once and retried.
pub fn save_doc(
    root: &Path,
    locator: &str,
    expected_revision: &str,
    content: &str,
) -> Result<SaveOutcome, UiError> {
    let mut resource = resource_for(root, locator)?;
    if resource.is_none() {
        scan(root)?;
        resource = resource_for(root, locator)?;
    }
    let Some(resource) = resource else {
        return Ok(SaveOutcome::Failed(SaveFailure::NotFound {
            path: root.join(locator).display().to_string(),
        }));
    };
    if resource.kind != notez_core::domain::ResourceKind::Document {
        return Ok(SaveOutcome::Failed(SaveFailure::Unsupported {
            reason: format!("{} is not an editable document", resource.kind.as_str()),
        }));
    }

    let facade = engine(root)?;
    let mut guard = facade
        .lock()
        .map_err(|e| UiError::Internal(format!("engine lock: {e}")))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let response = dispatcher.dispatch(Request::UpdateDocument(UpdateDocumentRequest {
        source_id: resource.source_id.clone(),
        locator: locator.to_string(),
        content: content.to_string(),
        base_revision: Some(expected_revision.to_string()),
        format: None,
        expected_revision: Some(expected_revision.to_string()),
    }));
    match response {
        Ok(DispatchResponse::DocumentUpdated(report)) => Ok(SaveOutcome::Saved {
            row: crate::model::ResourceRow::from(resource),
            revision: report.revision,
        }),
        Err(notez_core::application::ApplicationError::RevisionConflict { expected, actual }) => {
            Ok(SaveOutcome::Failed(SaveFailure::StaleRevision { expected, actual }))
        }
        Err(notez_core::application::ApplicationError::ReadOnlySource { source_id }) => {
            Ok(SaveOutcome::Failed(SaveFailure::ReadOnly {
                reason: format!("source is read-only: {source_id}"),
            }))
        }
        Err(e) => Ok(SaveOutcome::Failed(SaveFailure::Internal { message: e.to_string() })),
        Ok(other) => Err(UiError::Internal(format!("unexpected update response: {other:?}"))),
    }
}

/// Human text for a failed save.
pub fn save_failure_text(failure: &SaveFailure) -> String {
    match failure {
        SaveFailure::StaleRevision { expected, actual } => format!(
            "The file changed on disk since you opened it (expected {}, found {}). Your text is kept; saving again overwrites the on-disk version.",
            &expected[..expected.len().min(12)],
            &actual[..actual.len().min(12)],
        ),
        SaveFailure::ReadOnly { reason } => format!("Read-only source: {reason}"),
        SaveFailure::NotFound { path } => format!("File not found: {path}"),
        SaveFailure::Unsupported { reason } => format!("Cannot edit this file: {reason}"),
        SaveFailure::Internal { message } => format!("Save failed: {message}"),
    }
}

/// Rescan the space (index new/changed files). Returns the number of
/// indexed resources after the scan.
pub fn scan(root: &Path) -> Result<usize, UiError> {
    let facade = engine(root)?;
    let mut guard = facade
        .lock()
        .map_err(|e| UiError::Internal(format!("engine lock: {e}")))?;
    ApplicationDispatcher::new(&mut *guard)
        .dispatch(Request::ScanNative(ScanNativeRequest {}))
        .map_err(|e| UiError::Internal(e.to_string()))?;
    drop(guard);
    Ok(resources(root).map(|r| r.len()).unwrap_or(0))
}

/// Cheap change fingerprint: newest mtime + file count. Polled by the
/// page to detect external edits without re-rendering.
pub fn fingerprint(root: &Path) -> (String, usize) {
    let files = list_files(root);
    let newest = files.iter().map(|f| f.mtime_ms).max().unwrap_or(0);
    (format!("{newest}-{}", files.len()), files.len())
}

/// Create a note if absent, then return its locator. `locator` is a
/// sanitized space-relative path ending in `.md` or `.org`.
pub fn create_doc(
    root: &Path,
    locator: &str,
    title: &str,
    body: Option<&str>,
) -> Result<(), UiError> {
    let full = root.join(locator);
    if full.exists() {
        return Ok(());
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| UiError::Internal(format!("create dirs: {e}")))?;
    }
    let content = match body {
        Some(text) if !text.trim().is_empty() => text.to_string(),
        _ if locator.ends_with(".org") => format!("* {title}\n\n"),
        _ => format!("# {title}\n\n"),
    };
    std::fs::write(&full, content).map_err(|e| UiError::Internal(format!("write note: {e}")))?;
    let _ = scan(root);
    Ok(())
}

/// Validate a user-supplied locator for creation.
pub fn sanitize_new_locator(input: &str) -> Result<String, UiError> {
    let trimmed = input.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err(UiError::BadRequest("file name must not be empty".into()));
    }
    let mut segments: Vec<&str> = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') || seg.contains('\\') {
            return Err(UiError::BadRequest(format!("invalid path segment `{seg}`")));
        }
        segments.push(seg);
    }
    let mut loc = segments.join("/");
    let ext = Path::new(&loc)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        loc.push_str(".md");
    } else if !matches!(ext.as_str(), "md" | "org") {
        return Err(UiError::BadRequest(format!(
            "unsupported extension `.{ext}` (use .md or .org)"
        )));
    }
    Ok(loc)
}

/// Slug for a new note title (`My Idea` → `my-idea`).
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "untitled".to_string() } else { out }
}

/// Join `locator` onto `root` and refuse anything that escapes the
/// space (absolute paths, `..`, symlinks pointing outside).
pub fn safe_join(root: &Path, locator: &str) -> Result<PathBuf, UiError> {
    if locator.is_empty() || locator.starts_with('/') {
        return Err(UiError::BadRequest("empty or absolute path".into()));
    }
    for seg in locator.split('/') {
        if seg == ".." || seg == "." || seg.is_empty() {
            return Err(UiError::BadRequest(format!("invalid path segment `{seg}`")));
        }
    }
    let full = root.join(locator);
    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canonical_full = std::fs::canonicalize(&full).unwrap_or_else(|_| full.clone());
    if !canonical_full.starts_with(&canonical_root) {
        return Err(UiError::BadRequest("path escapes the space".into()));
    }
    Ok(full)
}

/// Whether a locator is an editable document.
pub fn is_doc_locator(locator: &str) -> bool {
    matches!(
        Path::new(locator)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "org")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_traversal_and_hidden_paths() {
        assert!(sanitize_new_locator("../etc/passwd").is_err());
        assert!(sanitize_new_locator(".secret.md").is_err());
        assert!(sanitize_new_locator("a//b.md").is_err());
    }

    #[test]
    fn sanitize_appends_markdown_and_keeps_org() {
        assert_eq!(sanitize_new_locator("notes/idea").unwrap(), "notes/idea.md");
        assert_eq!(sanitize_new_locator("notes/idea.org").unwrap(), "notes/idea.org");
        assert!(sanitize_new_locator("notes/idea.pdf").is_err());
    }

    #[test]
    fn slugify_matches_note_filenames() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("   "), "untitled");
    }

    #[test]
    fn doc_locators_are_recognized() {
        assert!(is_doc_locator("a/b.md"));
        assert!(is_doc_locator("a/B.ORG"));
        assert!(!is_doc_locator("a/b.pdf"));
        assert!(!is_doc_locator("noext"));
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let root = Path::new("/tmp");
        assert!(safe_join(root, "../etc/passwd").is_err());
        assert!(safe_join(root, "/abs").is_err());
        assert!(safe_join(root, "ok/x.md").is_ok());
    }

    fn resource(kind: notez_core::domain::ResourceKind, locator: &str, title: &str) -> Resource {
        Resource {
            r#ref: notez_core::domain::ResourceRef::parse("document:01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .unwrap(),
            kind,
            title: title.to_string(),
            revision: "r".to_string(),
            source_id: "native".to_string(),
            locator: locator.to_string(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectIdentity::default(),
            primary_source_id: "native".to_string(),
        }
    }

    #[test]
    fn heading_rows_do_not_shadow_the_document_row() {
        use notez_core::domain::ResourceKind;
        let map = index_by_locator(vec![
            resource(ResourceKind::Document, "notes/a.org", "A note"),
            resource(ResourceKind::Heading, "notes/a.org", "A heading"),
        ]);
        assert_eq!(
            map.get("notes/a.org"),
            Some(&("A note".to_string(), "document".to_string()))
        );
    }

    #[test]
    fn attachment_rows_keep_their_kind() {
        use notez_core::domain::ResourceKind;
        let map = index_by_locator(vec![resource(
            ResourceKind::Attachment,
            "assets/x.png",
            "x.png",
        )]);
        assert_eq!(
            map.get("assets/x.png"),
            Some(&("x.png".to_string(), "attachment".to_string()))
        );
    }

    #[test]
    fn doc_title_prefers_org_title_then_heading() {
        assert_eq!(
            doc_title(Path::new("/tmp"), "a.org", "#+TITLE: Hello\n* other\n"),
            "Hello"
        );
        assert_eq!(
            doc_title(Path::new("/tmp"), "a.org", "* Hello\n"),
            "Hello"
        );
        assert_eq!(doc_title(Path::new("/tmp"), "notes/a.org", "body\n"), "a");
    }
}
