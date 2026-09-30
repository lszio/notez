//! Server-side data access and the workspace's utility endpoints.
//!
//! The Dioxus app in [`crate::app`] renders every page; this module
//! owns the non-rendering HTTP surface plus the data layer the
//! components read during SSR:
//!
//! * [`space`] — open a space, list files, read/save documents;
//! * [`urls`] — space/locator encoding and URL builders;
//! * [`html`] — escaping and relative-URL rewriting for rendered bodies;
//! * the router below — raw bytes, form writes, live preview, and
//!   status endpoints the pages call from small JS islands.
//!
//! Every read goes through the composition `Runtime` engine cache and
//! every write through the protocol spine, so the UI, the HTTP API and
//! MCP observe one state.

pub mod html;
pub mod pending;
pub mod picker;
pub mod space;
pub mod urls;

pub use space::{Entry, Space, UiError};

use axum::extract::{Form, Multipart, Path, Query};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::server::{SaveFailure, SaveOutcome};

/// Cache-busting token for the embedded stylesheet/script, derived from
/// their contents at build time (see `build.rs`).
pub const ASSET_VERSION: &str = env!("NOTEZ_ASSET_VERSION");

/// Shared workspace stylesheet (owned by `packages/ui`).
const STYLE: &str = ui::WORKSPACE_CSS;
const SCRIPT: &str = include_str!("app.js");

/// Maximum accepted form body (a long note is still well under this).
const MAX_BODY: usize = 32 * 1024 * 1024;

/// Utility routes, mounted beside the Dioxus SSR app.
///
/// GET page routes are intentionally absent: `dioxus::server::router`
/// renders those through [`crate::app`].
pub fn router() -> Router {
    Router::new()
        .route("/", get(home_get))
        .route("/app.css", get(stylesheet))
        .route("/app.js", get(script))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/raw/{encoded}/{*locator}", get(raw_get))
        .route("/register", post(register_post))
        .route("/save/{encoded}", post(save_post))
        .route("/replace/{encoded}", post(replace_post))
        .route("/scan/{encoded}", post(scan_post))
        .route("/new/{encoded}", post(new_post))
        .route("/api/render", post(render_post))
        .route("/api/space-status", get(space_status_get))
        .route("/api/janet/eval", post(crate::routes::janet_eval))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .layer(axum::middleware::from_fn(crate::routes::mutation_auth_middleware))
}

// ------------------------------------------------------------ responses

/// `/` — redirect into the configured default space, else show the picker.
async fn home_get() -> Response {
    if let Some(space) = space::default_space() {
        return redirect(&urls::space_url(&space.encoded));
    }
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
        picker::picker_page(&space::list_spaces()),
    )
        .into_response()
}

async fn stylesheet() -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("text/css; charset=utf-8")),
            (header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400")),
        ],
        STYLE,
    )
        .into_response()
}

async fn script() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/javascript; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400")),
        ],
        SCRIPT,
    )
        .into_response()
}

impl IntoResponse for UiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
             <link rel=\"stylesheet\" href=\"/app.css?v={}\"></head>\
             <body><main class=\"content\"><div class=\"banner err\">{}</div>\
             <p class=\"edit-hint\"><a href=\"/\">back to the space picker</a></p></main></body></html>",
            html::esc(self.message()),
            ASSET_VERSION,
            html::esc(self.message()),
        );
        (
            status,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
            body,
        )
            .into_response()
    }
}

// ------------------------------------------------------------ handlers

#[derive(Deserialize)]
struct RegisterForm {
    path: String,
    #[serde(default)]
    name: Option<String>,
}

async fn register_post(Form(form): Form<RegisterForm>) -> Result<Response, UiError> {
    let path = crate::routes::register_source(&form.path, form.name.as_deref())
        .map_err(UiError::BadRequest)?;
    let root = path.to_string_lossy().into_owned();
    Ok(redirect(&urls::space_url(&urls::encode_space(&root))))
}

async fn raw_get(
    Path((encoded, locator)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, UiError> {
    use std::io::{Read, Seek, SeekFrom};

    let space = space::open(&encoded)?;
    let locator = urls::decode_locator(locator.trim_start_matches('/'));
    let full = space::safe_join(&space.root, &locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let total = std::fs::metadata(&full)
        .map_err(|e| UiError::Internal(format!("stat {}: {e}", full.display())))?
        .len();
    let mime = mime_guess::from_path(&full)
        .first_or_octet_stream()
        .to_string();
    let mut response = HeaderMap::new();
    response.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    // no-store: files change on disk and the projection's fingerprint
    // is the source of truth; never let the browser serve stale bytes.
    response.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let name = std::path::Path::new(&locator)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    response.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "inline; filename*=UTF-8''{}",
            urlencoding::encode(name)
        ))
        .unwrap_or(HeaderValue::from_static("inline")),
    );
    // Range support (single byte-range only — all browsers use that for
    // <video>/<audio>). Media playback 卡顿 was caused by the server
    // always answering 200 + the whole 90 MB file: the player can neither
    // seek nor start until the full download lands. A 206 slice lets the
    // browser stream and seek; document viewers are unaffected (they
    // fetch without Range and get the plain 200 body).
    response.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    let range = if let Some(spec) = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
    {
        Some(single_byte_range(spec, total))
    } else {
        None
    };
    if let Some(range) = range {
        let (start, end) = match range {
            Ok(r) => r,
            Err(()) => {
                response.insert(
                    header::CONTENT_RANGE,
                    HeaderValue::from_str(&format!("bytes */{total}"))
                        .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
                );
                return Ok((StatusCode::RANGE_NOT_SATISFIABLE, response).into_response());
            }
        };
        let len = (end - start + 1) as usize;
        let mut buf = vec![0u8; len];
        let mut file = std::fs::File::open(&full)
            .map_err(|e| UiError::Internal(format!("open {}: {e}", full.display())))?;
        file.seek(SeekFrom::Start(start))
            .map_err(|e| UiError::Internal(format!("seek {}: {e}", full.display())))?;
        file.read_exact(&mut buf)
            .map_err(|e| UiError::Internal(format!("read {}: {e}", full.display())))?;
        response.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{total}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
        );
        return Ok((StatusCode::PARTIAL_CONTENT, response, buf).into_response());
    }
    let bytes = std::fs::read(&full)
        .map_err(|e| UiError::Internal(format!("read {}: {e}", full.display())))?;
    Ok((response, bytes).into_response())
}

/// Parse a single `bytes=` range spec (`N-`, `N-M`, `-S`) into an
/// inclusive `(start, end)` within `[0, total)`. Returns `Err(())`
/// for specs that are unsatisfiable (start past EOF, or empty file).
fn single_byte_range(spec: &str, total: u64) -> Result<(u64, u64), ()> {
    if total == 0 {
        return Err(());
    }
    let (start_s, end_s) = spec.split_once('-').ok_or(())?;
    if start_s.is_empty() {
        // Suffix range: the last `S` bytes.
        let suffix: u64 = end_s.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let start = total.saturating_sub(suffix);
        return Ok((start, total - 1));
    }
    let start: u64 = start_s.parse().map_err(|_| ())?;
    if start >= total {
        return Err(());
    }
    let end = if end_s.is_empty() {
        total - 1
    } else {
        end_s.parse::<u64>().map_err(|_| ())?.min(total - 1)
    };
    // end < start after clamping a spec like `5-3` against a small file.
    if end < start {
        return Err(());
    }
    Ok((start, end))
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
    let full = space::safe_join(&space.root, &locator)?;
    if !full.is_file() {
        return Err(UiError::NotFound(format!("not found: {locator}")));
    }
    let outcome = space::save_doc(&space.root, &locator, &form.revision, &form.content)?;
    match outcome {
        SaveOutcome::Saved { revision, .. } => Ok(redirect(&format!(
            "{}?saved={}",
            urls::view_url(&encoded, &locator),
            urlencoding::encode(&revision)
        ))),
        SaveOutcome::Failed(failure) => {
            // Keep the user's text: stash the submission and send the
            // browser back to the editor, which restores it.
            let (kind, revision) = match &failure {
                SaveFailure::StaleRevision { actual, .. } => ("stale", actual.clone()),
                _ => ("error", form.revision.clone()),
            };
            let token = pending::stash(pending::PendingSave {
                locator: locator.clone(),
                content: form.content,
                kind: kind.to_string(),
                message: space::save_failure_text(&failure),
                revision,
            });
            Ok(redirect(&format!(
                "{}?edit=1&restore={token}",
                urls::view_url(&encoded, &locator),
            )))
        }
    }
}

#[derive(Deserialize)]
struct ScanForm {
    #[serde(default)]
    next: Option<String>,
}

/// `POST /replace/{encoded}` — multipart upload that replaces an
/// attachment's bytes behind a content-hash revision precondition.
/// Used by the generic "edit · replace file" affordance that
/// `PreviewCapabilities::can_edit` enables.
async fn replace_post(
    Path(encoded): Path<String>,
    mut multipart: Multipart,
) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let mut locator: Option<String> = None;
    let mut revision: Option<String> = None;
    let mut payload: Vec<u8> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| UiError::BadRequest(format!("multipart: {e}")))?
    {
        match field.name().unwrap_or_default() {
            "locator" => {
                locator = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| UiError::BadRequest(format!("locator: {e}")))?,
                );
            }
            "revision" => {
                revision = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| UiError::BadRequest(format!("revision: {e}")))?,
                );
            }
            "file" => {
                payload = field
                    .bytes()
                    .await
                    .map_err(|e| UiError::BadRequest(format!("file: {e}")))?
                    .to_vec();
            }
            _ => {}
        }
    }
    let locator = locator.ok_or_else(|| UiError::BadRequest("missing locator field".into()))?;
    let revision = revision.unwrap_or_default();
    if payload.is_empty() {
        return Err(UiError::BadRequest("missing or empty file field".into()));
    }
    match space::replace_attachment(&space.root, &locator, &revision, &payload)? {
        space::ReplaceOutcome::Saved { revision } => Ok(redirect(&format!(
            "{}?saved={}",
            urls::view_url(&encoded, &locator),
            urlencoding::encode(&revision)
        ))),
        space::ReplaceOutcome::Failed(failure) => {
            let message = space::save_failure_text(&failure);
            Ok(redirect(&format!(
                "{}?error={}",
                urls::view_url(&encoded, &locator),
                urlencoding::encode(&message)
            )))
        }
    }
}

async fn scan_post(
    Path(encoded): Path<String>,
    Form(form): Form<ScanForm>,
) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    space::scan(&space.root)?;
    // Return the user where they were; only local paths are honoured.
    let target = form
        .next
        .filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .unwrap_or_else(|| urls::space_url(&encoded));
    Ok(redirect(&target))
}

#[derive(Deserialize)]
struct NewForm {
    name: String,
    #[serde(default)]
    body: Option<String>,
}

async fn new_post(
    Path(encoded): Path<String>,
    Form(form): Form<NewForm>,
) -> Result<Response, UiError> {
    let space = space::open(&encoded)?;
    let locator = space::sanitize_new_locator(&form.name)?;
    let title = space::slugify(
        std::path::Path::new(&locator)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled"),
    );
    space::create_doc(&space.root, &locator, &title, form.body.as_deref())?;
    Ok(redirect(&urls::edit_url(&encoded, &locator)))
}

/// Live-preview endpoint: render `content` exactly as the read page
/// would, so the editor's preview pane and the saved page can never
/// disagree (one renderer, one source of truth).
#[derive(Deserialize)]
struct RenderForm {
    encoded: String,
    locator: String,
    content: String,
}

async fn render_post(Form(form): Form<RenderForm>) -> Result<Response, UiError> {
    let space = space::open(&form.encoded)?;
    let locator = form.locator.trim().to_string();
    let _ = space::safe_join(&space.root, &locator)?;
    let html = space::render_document(&space, &locator, &form.content)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
        html,
    )
        .into_response())
}

#[derive(Deserialize)]
struct StatusQuery {
    encoded: String,
}

/// Cheap change fingerprint for the space + watcher state, polled by
/// the page to surface "changed on disk — reload" without a full
/// re-render.
async fn space_status_get(Query(query): Query<StatusQuery>) -> Result<Response, UiError> {
    let space = space::open(&query.encoded)?;
    let (fingerprint, files) = space::fingerprint(&space.root);
    let watching = crate::routes::GLOBAL_WATCH.status(&space.root).is_some();
    Ok(axum::Json(serde_json::json!({
        "fingerprint": fingerprint,
        "files": files,
        "watching": watching,
    }))
    .into_response())
}

fn redirect(location: &str) -> Response {
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, HeaderValue::from_str(location).unwrap())],
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_version_is_part_of_the_asset_urls() {
        assert!(!ASSET_VERSION.is_empty());
        assert!(STYLE.contains(".sidebar"));
        assert!(SCRIPT.contains("addEventListener"));
    }

    #[test]
    fn stylesheet_covers_the_shared_workspace_classes() {
        for class in [".shell", ".sidebar", ".tree-file", ".doc", ".toolbar", ".edit-split"] {
            assert!(STYLE.contains(class), "missing {class}");
        }
    }

    #[test]
    fn single_byte_range_parses_open_closed_and_suffix_specs() {
        assert_eq!(single_byte_range("0-", 10), Ok((0, 9)));
        assert_eq!(single_byte_range("2-5", 10), Ok((2, 5)));
        assert_eq!(single_byte_range("-100", 10), Ok((0, 9))); // suffix clamps to total
        assert_eq!(single_byte_range("-3", 10), Ok((7, 9)));
        assert_eq!(single_byte_range("8-99", 10), Ok((8, 9))); // end clamps
        assert_eq!(single_byte_range("5-3", 10), Err(()));
        assert_eq!(single_byte_range("99-", 10), Err(()));
        assert_eq!(single_byte_range("-0", 10), Err(()));
        assert_eq!(single_byte_range("0-0", 0), Err(()));
    }
}
