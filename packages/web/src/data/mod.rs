//! Minimal server-rendered workspace UI.
//!
//! Design goals (see the session brief): start from a usable reader —
//! browse a space, read a note, edit and save it, preview attachments
//! — with a SilverBullet-style shell: one narrow sidebar listing the
//! pages, one content column, no client framework.
//!
//! Everything is plain HTTP: `GET` renders, `POST` mutates, every
//! interaction is a normal navigation or form submit. The only
//! JavaScript is the page filter, the editor shortcuts and the
//! unsaved-changes guard.
//!
//! Route table:
//!
//! ```text
//! GET  /                        space picker (or redirect to the only space)
//! GET  /app.css                 embedded stylesheet
//! POST /register                register a directory as a space
//! GET  /s/{space}               space index (README/index page or page list)
//! GET  /s/{space}/{*locator}    note view, `?edit=1` for the editor,
//!                               attachment preview for non-documents
//! GET  /raw/{space}/{*locator}  raw bytes (images, PDFs, downloads)
//! GET  /new/{space}             new-note form
//! POST /new/{space}             create the note and open the editor
//! POST /save/{space}            save a document (revision guarded)
//! POST /scan/{space}            rescan the space
//! POST /api/janet/eval          restricted Janet evaluation (debug)
//! ```

mod page;
mod space;
pub mod urls;

pub use space::{Entry, Space, UiError};

use axum::extract::{Form, Path, Query};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::server::{SaveFailure, SaveOutcome};

/// Cache-busting token for the embedded stylesheet. Bump on edit.
pub const STYLE_HASH: &str = "sb-1";

const STYLE: &str = include_str!("style.css");

/// Maximum accepted form body (a long note is still well under this).
const MAX_BODY: usize = 32 * 1024 * 1024;

/// UI routes. Mounted beside the protocol API and MCP routers.
pub fn router() -> Router {
    Router::new()
        .route("/", get(home))
        .route("/app.css", get(stylesheet))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/register", post(register_post))
        .route("/s/{encoded}", get(space_index))
        .route("/s/{encoded}/{*locator}", get(doc_get))
        .route("/raw/{encoded}/{*locator}", get(raw_get))
        .route("/new/{encoded}", get(new_get).post(new_post))
        .route("/save/{encoded}", post(save_post))
        .route("/scan/{encoded}", post(scan_post))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .layer(axum::middleware::from_fn(crate::routes::mutation_auth_middleware))
}

// ------------------------------------------------------------ responses

fn html(status: StatusCode, body: String) -> Response {
    (
        status,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

fn page_response(
    status: StatusCode,
    title: &str,
    space: Option<&Space>,
    current: Option<&str>,
    entries: &[Entry],
    topbar: &str,
    body: &str,
) -> Response {
    html(
        status,
        page::shell(title, space, current, entries, topbar, body),
    )
}

impl IntoResponse for UiError {
    fn into_response(self) -> Response {
        let status = self.status();
        page_response(
            status,
            "error",
            None,
            None,
            &[],
            &format!("<span class=\"title\">{}</span>", page::esc(self.message())),
            &page::error_page(self.message()),
        )
    }
}

async fn stylesheet() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/css; charset=utf-8"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=86400"),
            ),
        ],
        STYLE,
    )
        .into_response()
}

// ------------------------------------------------------------ handlers

async fn home() -> Response {
    if let Some(space) = space::default_space() {
        return Redirect::to(&urls::space_url(&space.encoded)).into_response();
    }
    page_response(
        StatusCode::OK,
        "spaces",
        None,
        None,
        &[],
        "<span class=\"title\">notez</span>",
        &page::picker(&space::list_spaces(), None),
    )
}

#[derive(Deserialize)]
struct RegisterForm {
    path: String,
    #[serde(default)]
    name: Option<String>,
}

async fn register_post(Form(form): Form<RegisterForm>) -> Result<Redirect, UiError> {
    let path = crate::routes::register_source(&form.path, form.name.as_deref())
        .map_err(UiError::BadRequest)?;
    let root = path.to_string_lossy().into_owned();
    Ok(Redirect::to(&urls::space_url(&urls::encode_space(&root))))
}

async fn space_index(Path(encoded): Path<String>) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let entries = space::entries(&space.root)?;

    // A README/index page at the space root is the landing document.
    let landing = ["README.org", "README.md", "index.org", "index.md"]
        .into_iter()
        .find(|name| space.root.join(name).is_file());

    let (title, body) = match landing {
        Some(locator) => {
            let (content, _rev) = space::read_doc(&space.root, locator)?;
            let title = space::doc_title(&space.root, locator, &content);
            let row = doc_row(locator, &title, "document");
            let rendered = crate::body::render_body(&row, &space.root);
            let rendered = page::rewrite_local_urls(&rendered, &encoded, "");
            (
                title,
                format!(
                    "<article class=\"doc\">{rendered}</article><hr><p class=\"edit-hint\"><a href=\"{edit}\">edit {locator}</a></p>",
                    edit = urls::edit_url(&encoded, locator),
                ),
            )
        }
        None => {
            let body = format!(
                "<h1>{}</h1><p class=\"edit-hint\">{} files · {} documents</p>{}",
                page::esc(&space.name),
                entries.len(),
                entries.iter().filter(|e| e.editable()).count(),
                page::page_list(&space, &entries),
            );
            (space.name.clone(), body)
        }
    };

    Ok(page_response(
        StatusCode::OK,
        &title,
        Some(&space),
        None,
        &entries,
        &page::topbar_space(&space, &title),
        &body,
    ))
}

#[derive(Deserialize, Default)]
struct ViewQuery {
    #[serde(default)]
    edit: Option<String>,
    #[serde(default)]
    saved: Option<String>,
    #[serde(default)]
    err: Option<String>,
}

async fn doc_get(
    Path((encoded, locator)): Path<(String, String)>,
    Query(query): Query<ViewQuery>,
) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let locator = urls::decode_locator(locator.trim_start_matches('/'));
    let full = safe_join(&space.root, &locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let entries = space::entries(&space.root)?;
    let is_doc = is_doc_locator(&locator);

    if !is_doc {
        let title = std::path::Path::new(&locator)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&locator)
            .to_string();
        let preview = crate::body::render_path(&full, &title, &space.root);
        let doc_dir = urls::split_locator(&locator).0;
        let preview = page::rewrite_local_urls(&preview, &encoded, doc_dir);
        return Ok(page_response(
            StatusCode::OK,
            &title,
            Some(&space),
            Some(&locator),
            &entries,
            &page::topbar_file(&space, &locator, &title),
            &page::file_view(&preview, ""),
        ));
    }

    let (content, revision) = space::read_doc(&space.root, &locator)?;
    let title = space::doc_title(&space.root, &locator, &content);
    let notice = saved_notice(query.saved.as_deref(), query.err.as_deref());

    if query.edit.as_deref() == Some("1") {
        return Ok(page_response(
            StatusCode::OK,
            &title,
            Some(&space),
            Some(&locator),
            &entries,
            &page::topbar_doc(&space, &locator, &title, true),
            &page::editor(&space, &locator, &revision, &content, &notice),
        ));
    }

    let row = doc_row(&locator, &title, "document");
    let rendered = crate::body::render_body(&row, &space.root);
    let doc_dir = urls::split_locator(&locator).0;
    let rendered = page::rewrite_local_urls(&rendered, &encoded, doc_dir);
    Ok(page_response(
        StatusCode::OK,
        &title,
        Some(&space),
        Some(&locator),
        &entries,
        &page::topbar_doc(&space, &locator, &title, false),
        &page::doc_view(&space, &locator, &rendered, &notice),
    ))
}

async fn raw_get(Path((encoded, locator)): Path<(String, String)>) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let locator = urls::decode_locator(locator.trim_start_matches('/'));
    let full = safe_join(&space.root, &locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let bytes = std::fs::read(&full)
        .map_err(|e| UiError::Internal(format!("read {}: {e}", full.display())))?;
    let mime = mime_guess::from_path(&full)
        .first_or_octet_stream()
        .to_string();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let name = std::path::Path::new(&locator)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "inline; filename*=UTF-8''{}",
            urlencoding::encode(name)
        ))
        .unwrap_or(HeaderValue::from_static("inline")),
    );
    Ok((headers, bytes).into_response())
}

#[derive(Deserialize)]
struct SaveForm {
    locator: String,
    revision: String,
    content: String,
}

async fn save_post(
    Path(encoded): Path<String>,
    Form(form): Form<SaveForm>,
) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let locator = form.locator.trim().to_string();
    let full = safe_join(&space.root, &locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let entries = space::entries(&space.root)?;
    let title = space::doc_title(&space.root, &locator, &form.content);

    let outcome = space::save_doc(&space.root, &locator, &form.revision, &form.content)?;
    match outcome {
        SaveOutcome::Saved { revision, .. } => Ok(Redirect::to(&format!(
            "{}?saved={}",
            urls::view_url(&encoded, &locator),
            urlencoding::encode(&revision)
        ))
        .into_response()),
        SaveOutcome::Failed(failure) => {
            // Keep the user's text: re-render the editor with the
            // current on-disk revision so a deliberate second save
            // overwrites instead of silently losing the edit.
            let (banner, revision) = match &failure {
                SaveFailure::StaleRevision { actual, .. } => (
                    page::banner(
                        "warn",
                        "The file changed on disk. Your text is kept; Save again overwrites the on-disk version.",
                    ),
                    actual.clone(),
                ),
                other => (page::banner("err", &page::save_failure_text(other)), form.revision.clone()),
            };
            Ok(page_response(
                StatusCode::CONFLICT,
                &title,
                Some(&space),
                Some(&locator),
                &entries,
                &page::topbar_doc(&space, &locator, &title, true),
                &page::editor(&space, &locator, &revision, &form.content, &banner),
            ))
        }
    }
}

async fn new_get(Path(encoded): Path<String>) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let entries = space::entries(&space.root)?;
    let body = format!(
        r#"<h1>New note</h1>
<form method="post" action="{action}" class="picker">
<input type="text" name="name" placeholder="notes/my-idea" autocomplete="off" spellcheck="false" autofocus>
<button class="btn primary" type="submit">Create</button>
</form>
<p class="edit-hint">Path is relative to <code>{root}</code>. <code>.md</code> is appended when no extension is given.</p>"#,
        action = urls::new_url(&encoded),
        root = page::esc(&space.root.to_string_lossy()),
    );
    Ok(page_response(
        StatusCode::OK,
        "new note",
        Some(&space),
        None,
        &entries,
        &page::topbar_space(&space, "New note"),
        &body,
    ))
}

#[derive(Deserialize)]
struct NewForm {
    name: String,
}

async fn new_post(
    Path(encoded): Path<String>,
    Form(form): Form<NewForm>,
) -> Result<Redirect, UiError> {
    let space = space::open(&encoded)?;
    let locator = space::sanitize_new_locator(&form.name)?;
    let title = space::slugify(
        std::path::Path::new(&locator)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled"),
    );
    space::create_doc(&space.root, &locator, &title)?;
    Ok(Redirect::to(&urls::edit_url(&encoded, &locator)))
}

async fn scan_post(Path(encoded): Path<String>) -> Result<Redirect, UiError> {
    let space = space::open(&encoded)?;
    space::scan(&space.root)?;
    Ok(Redirect::to(&urls::space_url(&encoded)))
}

// ------------------------------------------------------------ helpers

fn is_doc_locator(locator: &str) -> bool {
    matches!(
        std::path::Path::new(locator)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "org")
    )
}

/// Join `locator` onto `root` and refuse anything that escapes the
/// space (absolute paths, `..`, symlinks pointing outside).
fn safe_join(root: &std::path::Path, locator: &str) -> Result<std::path::PathBuf, UiError> {
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

fn doc_row(locator: &str, title: &str, kind: &str) -> crate::model::ResourceRow {
    crate::model::ResourceRow {
        ref_str: String::new(),
        kind: kind.to_string(),
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

fn saved_notice(saved: Option<&str>, err: Option<&str>) -> String {
    if let Some(revision) = saved {
        let short = urlencoding::decode(revision)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| revision.to_string());
        return page::banner("ok", &format!("Saved · {}", &short[..short.len().min(12)]));
    }
    if let Some(err) = err {
        return page::banner("err", err);
    }
    String::new()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_locators_are_recognized() {
        assert!(is_doc_locator("a/b.md"));
        assert!(is_doc_locator("a/B.ORG"));
        assert!(!is_doc_locator("a/b.pdf"));
        assert!(!is_doc_locator("noext"));
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let root = std::path::Path::new("/tmp");
        assert!(safe_join(root, "../etc/passwd").is_err());
        assert!(safe_join(root, "/abs").is_err());
        assert!(safe_join(root, "ok/x.md").is_ok());
    }
}
