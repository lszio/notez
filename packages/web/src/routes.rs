//! Custom HTTP routes for the v0.1+ web client.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::Form;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use toml;
use notez_core::application::{ApplicationError, Engine, WatchError, WatchService};
use notez_core::config::{web_space::{resolve_source as core_resolve_space, WebSourceError}, GlobalConfig, SourceRegistration};
use notez_core::storage::SqliteProjection;
use notez_core::application::dispatcher::{ApplicationDispatcher, Response as DispatchResponse};
use notez_protocol::request::{Request, ScanNativeRequest};
use serde::Deserialize;
use crate::server::{SaveFailure, SaveOutcome};
use crate::router::{decode_space, route_for_space_files, route_for_space_home, route_for_space_note};
/// Process-global watch service.
pub static GLOBAL_WATCH: std::sync::LazyLock<Arc<WatchService>> =
    std::sync::LazyLock::new(|| WatchService::new());

/// Process-global `WebState` used by the server functions.
pub static GLOBAL_STATE: std::sync::LazyLock<WebState> =
    std::sync::LazyLock::new(WebState::new);

/// Convenience accessor the server functions use to reach the global state.
pub fn state_snapshot() -> WebState {
    GLOBAL_STATE.clone()
}

pub fn auto_start_watch(source_root: &std::path::Path) {
    let canonical = std::fs::canonicalize(source_root).unwrap_or_else(|_| source_root.to_path_buf());
    let _ = GLOBAL_WATCH.start(&canonical);
}

#[derive(Clone)]
pub struct WebState {
    /// Process-global watch service (shared with the composition
    /// `Runtime` so form routes and server functions observe one
    /// live-reindex state).
    pub watch: Arc<WatchService>,
    /// Shared engine cache + watcher set. Engine acquisition now
    /// delegates to the composition root so the web surface, the HTTP
    /// API and the MCP host all open the same engine per source root.
    runtime: notez_composition::native::Runtime,
    /// Optional default space root (resolved at startup from
    /// =NOTEZ_DEFAULT_SPACE= or cwd/notez.toml). Used by the embedded
    /// Backend when no per-call space is supplied.
    default_root: Option<PathBuf>,
}

fn default_space_root() -> Option<PathBuf> {
    if let Ok(env_root) = std::env::var("NOTEZ_DEFAULT_SPACE") {
        let p = PathBuf::from(env_root);
        if p.exists() {
            return Some(p);
        }
    }
    std::env::current_dir().ok().filter(|p| p.join("notez.toml").exists())
}

impl WebState {
    pub fn new() -> Self {
        let runtime = notez_composition::native::Runtime::with_watch(GLOBAL_WATCH.clone());
        let watch = runtime.watch();
        let default_root = default_space_root();
        Self { watch, runtime, default_root }
    }

    /// Default space root, if configured.
    pub fn default_root(&self) -> &Option<PathBuf> {
        &self.default_root
    }

    /// Clone the shared runtime (for surfaces mounted beside the web
    /// routes in the same process).
    pub fn runtime(&self) -> notez_composition::native::Runtime {
        self.runtime.clone()
    }

    /// Acquire (or create) an `Engine` for `source_root`.
    pub fn engine_for(
        &self,
        source_root: &PathBuf,
    ) -> Result<Arc<Mutex<Engine<SqliteProjection>>>, WebRouteError> {
        let canonical = std::fs::canonicalize(source_root).unwrap_or_else(|_| source_root.clone());
        let sel = core_resolve_space(&canonical).map_err(WebRouteError::from)?;
        self.runtime
            .open(&sel)
            .map_err(|e| WebRouteError::Internal(e.to_string()))
    }
}

#[derive(Debug)]
pub enum WebRouteError {
    Invalid(String),
    Unauthorized(String),
    Internal(String),
    Status(StatusCode, String),
}
impl std::fmt::Display for WebRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) => write!(f, "invalid: {m}"),
            Self::Unauthorized(m) => write!(f, "unauthorized: {m}"),
            Self::Internal(m) => write!(f, "internal: {m}"),
            Self::Status(c, m) => write!(f, "{c}: {m}"),
        }
    }
}

impl From<WebSourceError> for WebRouteError {
    fn from(e: WebSourceError) -> Self {
        Self::Invalid(e.to_string())
    }
}

impl From<WatchError> for WebRouteError {
    fn from(e: WatchError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<ApplicationError> for WebRouteError {
    fn from(e: ApplicationError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl IntoResponse for WebRouteError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            Self::Invalid(m) => (StatusCode::BAD_REQUEST, m.clone()),
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            Self::Status(c, m) => (*c, m.clone()),
        };
        (status, msg).into_response()
    }
}

/// Mutation authentication policy: loopback (or direct in-process) requests
/// remain compatible with local acceptance; non-loopback requests require
/// NOTEZ_WEB_TOKEN via Bearer authorization or X-Notez-Token.
pub async fn mutation_auth_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path();
    let is_mutation = req.method() == axum::http::Method::POST
        && (path == "/api/sources/register"
            || path == "/api/sources/scan"
            || path == "/api/sources/document/edit"
            || path.starts_with("/api/sources/watch/")
            || path == "/api/janet/eval");
    if !is_mutation { return next.run(req).await; }
    let bound_public = std::env::var("IP")
        .ok()
        .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
        .is_some_and(|ip| !ip.is_loopback());
    // Dioxus does not reliably populate a peer SocketAddr for every local
    // SSR request. When the server itself is loopback-bound, the transport
    // is local by construction and must remain usable without a token.
    // Public binds require an explicit token even when peer metadata is absent.
    if !bound_public {
        return next.run(req).await;
    }
    let supplied = req
        .headers()
        .get("x-notez-token")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            req.headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        });
    let expected = std::env::var("NOTEZ_WEB_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty());
    if expected.as_deref().is_some_and(|token| supplied == Some(token)) {
        next.run(req).await
    } else {
        WebRouteError::Unauthorized("mutation requires NOTEZ_WEB_TOKEN".into()).into_response()
    }
}


#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    pub name: String,
    pub path: String,
}

fn do_register(_state: &WebState, form: RegisterForm) -> Result<Redirect, WebRouteError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(WebRouteError::Invalid("name must not be empty".into()));
    }
    let path_str = form.path.trim();
    if path_str.is_empty() {
        return Err(WebRouteError::Invalid("path must not be empty".into()));
    }
    let path = std::path::PathBuf::from(path_str);
    core_resolve_space(&path).map_err(WebRouteError::from)?;

    let env: std::collections::BTreeMap<String, std::ffi::OsString> =
        std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v)).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = notez_core::config::ConfigPaths::discover(&env, &cwd)
        .map_err(|e| WebRouteError::Internal(e.to_string()))?;
    let mut global: GlobalConfig = paths.global_config.clone().unwrap_or(GlobalConfig {
        version: 1,
        default_source: None,
        sources: Default::default(),
        preferences: Default::default(),
    });
    global.sources.insert(
        name.to_string(),
        SourceRegistration { path: path.clone(), config: None },
    );
    if global.default_source.is_none() {
        global.default_source = Some(name.to_string());
    }
    let text = toml::to_string(&global).map_err(|e| WebRouteError::Internal(e.to_string()))?;
    let cfg_path = paths.global;
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WebRouteError::Internal(e.to_string()))?;
    }
    let tmp = cfg_path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| WebRouteError::Internal(e.to_string()))?;
    std::fs::rename(&tmp, &cfg_path).map_err(|e| WebRouteError::Internal(e.to_string()))?;

    auto_start_watch(&path);
    Ok(Redirect::to(&route_for_space_files(path_str)))
}

#[derive(Debug, Deserialize)]
pub struct SpaceForm {
    pub source_root: Option<String>,
    pub encoded: Option<String>,
}

fn resolve_form_space(form: &SpaceForm) -> Result<PathBuf, WebRouteError> {
    if let Some(p) = &form.source_root {
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if !pb.exists() {
                return Err(WebRouteError::Invalid(format!("path does not exist: {pb:?}")));
            }
            return Ok(pb);
        }
    }
    if let Some(enc) = &form.encoded {
        if !enc.is_empty() {
            let decoded = decode_space(enc);
            let pb = PathBuf::from(&decoded);
            if pb.exists() {
                return Ok(pb);
            }
        }
    }
    Err(WebRouteError::Invalid("source_root or encoded is required".into()))
}

fn do_scan(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let source_root = resolve_form_space(&form)?;
    let facade = state.engine_for(&source_root)?;
    let mut guard = facade.lock().map_err(|e| WebRouteError::Internal(format!("facade lock: {e}")))?;
    match ApplicationDispatcher::new(&mut *guard).dispatch(Request::ScanNative(ScanNativeRequest {})) {
        Ok(DispatchResponse::Scan(_)) => Ok(Redirect::to(&route_for_space_files(&source_root.to_string_lossy()))),
        Ok(other) => Err(WebRouteError::Internal(format!("unexpected scan response: {other:?}"))),
        Err(e) => Err(e.into()),
    }
}

fn do_watch_start(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let source_root = resolve_form_space(&form)?;
    // Idempotent: the auto-watch middleware may already have started
    // this watch; re-posting the form must not surface as a 500.
    match state.watch.start(&source_root) {
        Ok(_) => {}
        Err(WatchError::AlreadyActive(_)) => {}
        Err(e) => return Err(e.into()),
    }
    Ok(Redirect::to(&format!(
        "{}{}",
        route_for_space_files(&source_root.to_string_lossy()),
        "?watch=1"
    )))
}

fn do_watch_stop(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let source_root = resolve_form_space(&form)?;
         state.watch.stop(&source_root);
     Ok(Redirect::to(&route_for_space_files(&source_root.to_string_lossy())))
 }

// ---- v2 note-workspace write endpoints -------------------------------------

/// Reorder / hide / show a home dashboard widget. The ordered widget
/// list lives in `web.toml`; see `crate::ui_config`.
#[derive(Debug, Deserialize)]
pub struct WidgetOpForm {
    pub widget: String,
    /// `up` | `down` | `hide` | `show`
    pub op: String,
}

fn do_widget_op(_state: &WebState, form: WidgetOpForm) -> Result<Redirect, WebRouteError> {
    let mut cfg = crate::ui_config::load_web_ui_config();
    match form.op.as_str() {
        "up" => cfg.move_widget(&form.widget, true),
        "down" => cfg.move_widget(&form.widget, false),
        "hide" => cfg.set_widget_hidden(&form.widget, true),
        "show" => cfg.set_widget_hidden(&form.widget, false),
        other => return Err(WebRouteError::Invalid(format!("unknown widget op `{other}`"))),
    }
    crate::ui_config::store_web_ui_config(&cfg).map_err(|e| WebRouteError::Internal(e))?;
    Ok(Redirect::to("/"))
}

/// Toggle the star on a note (star button in the note header).
#[derive(Debug, Deserialize)]
pub struct StarForm {
    pub source_root: Option<String>,
    pub encoded: Option<String>,
    pub ref_str: String,
}

fn do_star_toggle(_state: &WebState, form: StarForm) -> Result<Redirect, WebRouteError> {
    let ref_str = form.ref_str.trim();
    if ref_str.is_empty() {
        return Err(WebRouteError::Invalid("ref_str is required".into()));
    }
    let root = resolve_form_space(&SpaceForm {
        source_root: form.source_root,
        encoded: form.encoded,
    })?;
    let root_str = root.to_string_lossy().into_owned();
    let mut cfg = crate::ui_config::load_web_ui_config();
    cfg.toggle_star(&root_str, ref_str);
    crate::ui_config::store_web_ui_config(&cfg).map_err(WebRouteError::Internal)?;
    Ok(Redirect::to(&route_for_space_note(&root_str, ref_str)))
}

/// Set the landing mode of a space home (`journal` | `index` | `files`).
#[derive(Debug, Deserialize)]
pub struct LandingForm {
    pub source_root: Option<String>,
    pub encoded: Option<String>,
    pub landing: String,
}

fn do_set_landing(_state: &WebState, form: LandingForm) -> Result<Redirect, WebRouteError> {
    if !crate::ui_config::LANDING_MODES.contains(&form.landing.as_str()) {
        return Err(WebRouteError::Invalid(format!(
            "landing must be one of {}",
            crate::ui_config::LANDING_MODES.join(", ")
        )));
    }
    let root = resolve_form_space(&SpaceForm {
        source_root: form.source_root,
        encoded: form.encoded,
    })?;
    let root_str = root.to_string_lossy().into_owned();
    let mut cfg = crate::ui_config::load_web_ui_config();
    cfg.landing.insert(root_str.clone(), form.landing.clone());
    crate::ui_config::store_web_ui_config(&cfg).map_err(WebRouteError::Internal)?;
    Ok(Redirect::to(&route_for_space_home(&root_str)))
}

/// Lowercase ASCII slug: non-alphanumeric runs become `-`, edges
/// trimmed. Empty input yields `untitled`.
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true; // suppress leading dashes
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
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// Validate and normalize a space-relative locator for creation.
/// Rejects absolute paths, traversal, hidden files, and unsupported
/// extensions; appends `.md` when no extension is given.
pub fn sanitize_new_locator(input: &str) -> Result<String, WebRouteError> {
    let trimmed = input.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err(WebRouteError::Invalid("locator must not be empty".into()));
    }
    let mut segments: Vec<&str> = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') || seg.contains('\\') {
            return Err(WebRouteError::Invalid(format!("invalid locator segment `{seg}`")));
        }
        segments.push(seg);
    }
    let mut loc = segments.join("/");
    let ext = std::path::Path::new(&loc)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        loc.push_str(".md");
    } else if !matches!(ext.as_str(), "md" | "org") {
        return Err(WebRouteError::Invalid(format!(
            "unsupported note extension `.{ext}` (use .md or .org)"
        )));
    }
    Ok(loc)
}

/// Validate a `YYYY-MM-DD` journal date supplied by a form.
pub fn sanitize_journal_date(input: &str) -> Result<String, WebRouteError> {
    let mut parts = input.split('-');
    let (y, m, d) = (
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
        parts.next().unwrap_or(""),
    );
    if parts.next().is_some() {
        return Err(WebRouteError::Invalid("date must be YYYY-MM-DD".into()));
    }
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if !(digits(y, 4) && digits(m, 2) && digits(d, 2)) {
        return Err(WebRouteError::Invalid("date must be YYYY-MM-DD".into()));
    }
    Ok(input.to_string())
}

/// Human title from a locator stem (`notes/my-idea.md` → `My Idea`).
fn title_from_locator(locator: &str) -> String {
    let stem = std::path::Path::new(locator)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let mut out = stem.replace(['-', '_'], " ");
    if let Some(first) = out.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    out
}

/// Create a new note file inside a space (journal entry or ad-hoc
/// note), index it, and open it. An existing file at the locator is
/// opened, never overwritten (open-or-create semantics).
#[derive(Debug, Deserialize)]
pub struct CreateDocForm {
    pub source_root: Option<String>,
    pub encoded: Option<String>,
    /// Space-relative locator (`journal/2026-09-01.md`, `notes/idea.md`).
    pub locator: Option<String>,
    /// Used to derive the locator when `locator` is absent.
    pub title: Option<String>,
    /// Shorthand for daily notes: `journal/<journal_date>.md`.
    pub journal_date: Option<String>,
}

fn do_create_document(state: &WebState, form: CreateDocForm) -> Result<Redirect, WebRouteError> {
    let root = resolve_form_space(&SpaceForm {
        source_root: form.source_root,
        encoded: form.encoded,
    })?;
    let root_str = root.to_string_lossy().into_owned();
    let locator = if let Some(date) = form
        .journal_date
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        let date = sanitize_journal_date(date)?;
        format!("journal/{date}.md")
    } else {
        match form.locator.as_deref().map(str::trim) {
            Some(l) if !l.is_empty() => sanitize_new_locator(l)?,
            _ => {
                let title = form.title.as_deref().map(str::trim).unwrap_or("untitled");
                sanitize_new_locator(&format!("{}.md", slugify(title)))?
            }
        }
    };
    let full = root.join(&locator);
    if !full.exists() {
        let title = form
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .unwrap_or_else(|| {
                // Daily notes read better with the raw ISO date as the
                // title instead of "2026 09 01".
                match std::path::Path::new(&locator).file_stem().and_then(|s| s.to_str()) {
                    Some(stem) if locator.starts_with("journal/") && stem.len() == 10 => stem.to_string(),
                    _ => title_from_locator(&locator),
                }
            });
        let content = if locator.ends_with(".org") {
            format!("* {title}\n\n")
        } else {
            format!("# {title}\n\n")
        };
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| WebRouteError::Internal(format!("create dirs: {e}")))?;
        }
        std::fs::write(&full, content)
            .map_err(|e| WebRouteError::Internal(format!("write note: {e}")))?;
        // Index the new file so the note route can resolve it.
        let facade = state.engine_for(&root)?;
        let mut guard = facade
            .lock()
            .map_err(|e| WebRouteError::Internal(format!("facade lock: {e}")))?;
        let _ = ApplicationDispatcher::new(&mut *guard).dispatch(Request::ScanNative(ScanNativeRequest {}));
    }
    if let Some(r) = ref_for_locator(state, &root, &locator) {
        return Ok(Redirect::to(&route_for_space_note(&root_str, &r)));
    }
    Ok(Redirect::to(&crate::router::route_for_space_preview(
        &root_str,
        &locator,
    )))
}

/// Find the indexed ref for `locator`, or `None` when the scan has
/// not picked the file up (caller falls back to the preview route).
fn ref_for_locator(_state: &WebState, root: &PathBuf, locator: &str) -> Option<String> {
    let resources = crate::server::dispatch_query(root).ok()?;
    resources
        .into_iter()
        .find(|r| r.locator.to_string() == locator)
        .map(|r| r.r#ref.to_string())
}

#[derive(Debug, Deserialize)]
pub struct DocumentEditForm {
    pub source_root: String,
    pub ref_str: String,
    pub expected_revision: String,
    pub content: String,
}

fn save_failure_parts(f: &SaveFailure) -> (String, String) {
    match f {
        SaveFailure::StaleRevision { expected, actual } => (
            "stale_revision".to_string(),
            format!("the file changed while editing (expected {expected}, found {actual})"),
        ),
        SaveFailure::ReadOnly { reason } => ("read_only".to_string(), reason.clone()),
        SaveFailure::NotFound { path } => ("not_found".to_string(), format!("file not found: {path}")),
        SaveFailure::Unsupported { reason } => ("unsupported".to_string(), reason.clone()),
        SaveFailure::Internal { message } => ("internal".to_string(), message.clone()),
    }
}

fn do_edit_document(_state: &WebState, form: DocumentEditForm) -> Result<Redirect, WebRouteError> {
    let base = crate::router::route_for_space_note(&form.source_root, &form.ref_str);
    let r_ref = match notez_core::domain::ResourceRef::parse(&form.ref_str) {
        Ok(r) => r,
        Err(e) => return Ok(Redirect::to(&format!("{base}?edit_err=invalid_ref&edit_msg={}", urlencoding::encode(&e.to_string())))),
    };
    let outcome = crate::server::update_document_impl(PathBuf::from(&form.source_root).as_path(), &r_ref, &form.expected_revision, &form.content)
        .map_err(WebRouteError::Internal)?;
    match outcome {
        SaveOutcome::Saved { .. } => Ok(Redirect::to(&format!("{base}?edited=1"))),
        SaveOutcome::Failed(f) => { let (kind, msg) = save_failure_parts(&f); Ok(Redirect::to(&format!("{base}?edit_err={kind}&edit_msg={}", urlencoding::encode(&msg)))) }
    }
}

#[derive(Debug, Deserialize)]
pub struct JanetEvalForm {
    pub script: String,
}

fn do_janet_eval(form: JanetEvalForm) -> Result<axum::response::Response, WebRouteError> {
    let (status, body) = match crate::janet::eval_janet_checked(&form.script) {
        Ok(v) => (StatusCode::OK, format!("<!doctype html><html><body><h1>janet result</h1><pre class='janet-result'>{}</pre></body></html>", html_escape::encode_text(&serde_json::to_string_pretty(&v).unwrap_or_default()))),
        Err(e) => (StatusCode::BAD_REQUEST, format!("<!doctype html><html><body><h1>janet error</h1><pre class='janet-error' data-kind='{}'>{}</pre></body></html>", e.kind(), html_escape::encode_text(&e.to_string()))),
    };
    Ok((status, axum::response::Html(body)).into_response())
}
#[derive(Debug, Deserialize)]
pub struct AttachmentRawQuery { pub source_root: String, pub locator: String }

async fn raw_attachment_get(
    axum::extract::Query(query): axum::extract::Query<AttachmentRawQuery>,
) -> Result<impl axum::response::IntoResponse, WebRouteError> {
    let space_path = PathBuf::from(&query.source_root);
    let file_path = space_path.join(&query.locator);
    let canonical_space = std::fs::canonicalize(&space_path).unwrap_or(space_path);
    let canonical_file = match std::fs::canonicalize(&file_path) {
        Ok(f) => f,
        Err(_) => file_path.clone(),
    };
    if !canonical_file.starts_with(&canonical_space) || !file_path.exists() {
        return Err(WebRouteError::Invalid("file not found or access denied".into()));
    }
    let bytes = std::fs::read(&file_path).map_err(|e| WebRouteError::Internal(e.to_string()))?;
    let mime = mime_guess::from_path(&file_path).first_or_octet_stream().to_string();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        "inline".parse().unwrap(),
    );
    Ok((headers, bytes))
}

pub async fn auto_watch_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path();
    if path.starts_with("/source/") {
        if let Some(rest) = path.strip_prefix("/source/") {
            let encoded = rest.split('/').next().unwrap_or("");
            if !encoded.is_empty() {
                let space_path = crate::router::decode_space(encoded);
                if !space_path.is_empty() {
                    auto_start_watch(std::path::Path::new(&space_path));
                }
            }
        }
    }
    next.run(req).await
}

/// Build the sub-router with all custom POST endpoints.
pub fn build_router(state: WebState) -> axum::Router {
    let state_r = state.clone();
    let state_s = state.clone();
    let state_ed = state.clone();
    let state_ws = state.clone();
    let state_wt = state.clone();
    let state_ed2 = state.clone();
    let state_st = state.clone();
    let state_ld = state.clone();
    let state_cr = state.clone();
    let state_wg = state.clone();
    axum::Router::new()
        .route(
            "/api/sources/register",
            post(move |Form(form): Form<RegisterForm>| {
                let s = state_r.clone();
                async move { do_register(&s, form) }
            }),
        )
        .route(
            "/api/sources/scan",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_s.clone();
                async move { do_scan(&s, form) }
            }),
        )
        .route(
            "/api/sources/document/edit",
            post(move |Form(form): Form<DocumentEditForm>| {
                let s = state_ed.clone();
                async move { do_edit_document(&s, form) }
            }),
        )
        .route(
            "/api/sources/watch/start",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_ws.clone();
                async move { do_watch_start(&s, form) }
            }),
        )
        .route(
            "/api/sources/watch/stop",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_wt.clone();
                async move { do_watch_stop(&s, form) }
            }),
        )
        .route(
            "/api/sources/watch/state",
            get(move |axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>| {
                let s = state_wg.clone();
                async move { watch_state_get(&s, params).await }
            }),
        )
        .route(
            "/api/sources/attachment/raw",
            get(move |query| async move { raw_attachment_get(query).await }),
        )
        .route(
            "/api/janet/eval",
            post(move |Form(form): Form<JanetEvalForm>| {
                async move { do_janet_eval(form) }
            }),
        )
        .route(
            "/api/web/widgets",
            post(move |Form(form): Form<WidgetOpForm>| {
                let s = state_ed2.clone();
                async move { do_widget_op(&s, form) }
            }),
        )
        .route(
            "/api/web/star",
            post(move |Form(form): Form<StarForm>| {
                let s = state_st.clone();
                async move { do_star_toggle(&s, form) }
            }),
        )
        .route(
            "/api/web/landing",
            post(move |Form(form): Form<LandingForm>| {
                let s = state_ld.clone();
                async move { do_set_landing(&s, form) }
            }),
        )
        .route(
            "/api/sources/document/create",
            post(move |Form(form): Form<CreateDocForm>| {
                let s = state_cr.clone();
                async move { do_create_document(&s, form) }
            }),
        )
        .layer(axum::middleware::from_fn(mutation_auth_middleware))
        .layer(axum::middleware::from_fn(auto_watch_middleware))
}

async fn watch_state_get(
    state: &WebState,
    params: std::collections::HashMap<String, String>,
) -> Result<axum::Json<serde_json::Value>, WebRouteError> {
    let path = params
        .get("path")
        .ok_or_else(|| WebRouteError::Invalid("missing path".into()))?;
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let watching = state.watch.status(&canonical).is_some();
    Ok(axum::Json(serde_json::json!({
        "watching": watching,
        "path": canonical.to_string_lossy(),
    })))
}

/// Re-export the encoded-space helper.
pub fn encoded_for(source_root: &str) -> String {
    crate::router::encode_space(source_root)
}