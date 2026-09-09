//! Process-global web state and the small set of non-UI endpoints.
//!
//! The workspace UI lives in [`crate::ui`]; this module owns what every
//! surface in the process shares:
//!
//! * the single [`WatchService`] and the composition `Runtime` engine
//!   cache ([`WebState`]) — the same handles the HTTP API and MCP
//!   routers are mounted against;
//! * source registration (the picker's "open a directory" form);
//! * mutation authentication for public binds;
//! * the restricted Janet evaluation endpoint.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::Form;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use notez_core::application::{Engine, WatchService};
use notez_core::config::{
    web_space::{resolve_source as core_resolve_space, WebSourceError},
    GlobalConfig, SourceRegistration,
};
use notez_core::storage::SqliteProjection;
use serde::Deserialize;

/// Process-global watch service, shared with the composition `Runtime`
/// so every surface observes one live-reindex state.
pub static GLOBAL_WATCH: std::sync::LazyLock<Arc<WatchService>> =
    std::sync::LazyLock::new(|| WatchService::new());

/// Process-global state used by the UI handlers and server helpers.
pub static GLOBAL_STATE: std::sync::LazyLock<WebState> =
    std::sync::LazyLock::new(WebState::new);

pub fn state_snapshot() -> WebState {
    GLOBAL_STATE.clone()
}

/// Start watching `source_root` (idempotent).
pub fn auto_start_watch(source_root: &std::path::Path) {
    let canonical =
        std::fs::canonicalize(source_root).unwrap_or_else(|_| source_root.to_path_buf());
    let _ = GLOBAL_WATCH.start(&canonical);
}

/// Shared engine cache + watcher container for the web surface.
#[derive(Clone)]
pub struct WebState {
    pub watch: Arc<WatchService>,
    runtime: notez_composition::native::Runtime,
    default_root: Option<PathBuf>,
}

fn default_space_root() -> Option<PathBuf> {
    if let Ok(env_root) = std::env::var("NOTEZ_DEFAULT_SPACE") {
        let p = PathBuf::from(env_root);
        if p.exists() {
            return Some(p);
        }
    }
    std::env::current_dir()
        .ok()
        .filter(|p| p.join("notez.toml").exists())
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

    /// Clone the shared runtime (for surfaces mounted beside the UI).
    pub fn runtime(&self) -> notez_composition::native::Runtime {
        self.runtime.clone()
    }

    /// Acquire (or create) the `Engine` for `source_root`.
    pub fn engine_for(
        &self,
        source_root: &PathBuf,
    ) -> Result<Arc<Mutex<Engine<SqliteProjection>>>, String> {
        let canonical = std::fs::canonicalize(source_root).unwrap_or_else(|_| source_root.clone());
        let sel = core_resolve_space(&canonical).map_err(|e: WebSourceError| e.to_string())?;
        self.runtime.open(&sel).map_err(|e| e.to_string())
    }
}

impl Default for WebState {
    fn default() -> Self {
        Self::new()
    }
}

/// Register `path` as a named source in the global config and start
/// watching it. Returns the canonical root that was registered.
pub fn register_source(path: &str, name: Option<&str>) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path must not be empty".into());
    }
    let requested = PathBuf::from(trimmed);
    let resolved = core_resolve_space(&requested).map_err(|e| e.to_string())?;
    let root = resolved.root;

    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = notez_core::config::ConfigPaths::discover(&env, &cwd)
        .map_err(|e| e.to_string())?;

    let mut global: GlobalConfig = paths.global_config.clone().unwrap_or(GlobalConfig {
        version: 1,
        default_source: None,
        sources: Default::default(),
        preferences: Default::default(),
    });
    let key = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("space")
            .to_string(),
    };
    global
        .sources
        .insert(key, SourceRegistration { path: root.clone(), config: None });
    if global.default_source.is_none() {
        global.default_source = Some(
            global
                .sources
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "space".to_string()),
        );
    }

    let text = toml::to_string(&global).map_err(|e| e.to_string())?;
    let cfg_path = paths.global;
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = cfg_path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &cfg_path).map_err(|e| e.to_string())?;

    auto_start_watch(&root);
    Ok(root)
}

/// Require a token for mutations when the server is bound to a public
/// interface. Loopback binds stay usable without configuration.
pub async fn mutation_auth_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if req.method() != axum::http::Method::POST {
        return next.run(req).await;
    }
    let bound_public = std::env::var("IP")
        .ok()
        .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
        .is_some_and(|ip| !ip.is_loopback());
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
        (StatusCode::UNAUTHORIZED, "mutation requires NOTEZ_WEB_TOKEN").into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct JanetEvalForm {
    pub script: String,
}

async fn janet_eval(Form(form): Form<JanetEvalForm>) -> Response {
    match crate::janet::eval_janet_checked(&form.script) {
        Ok(value) => (
            StatusCode::OK,
            axum::response::Html(format!(
                "<!doctype html><html><body><h1>janet result</h1><pre class='janet-result'>{}</pre></body></html>",
                html_escape::encode_text(
                    &serde_json::to_string_pretty(&value).unwrap_or_default()
                )
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::response::Html(format!(
                "<!doctype html><html><body><h1>janet error</h1><pre class='janet-error' data-kind='{}'>{}</pre></body></html>",
                e.kind(),
                html_escape::encode_text(&e.to_string())
            )),
        )
            .into_response(),
    }
}

/// Router for the non-UI endpoints (Janet debug evaluation).
pub fn build_router(_state: WebState) -> axum::Router {
    axum::Router::new()
        .route("/api/janet/eval", post(janet_eval))
        .layer(axum::middleware::from_fn(mutation_auth_middleware))
}

/// Re-export the encoded-space helper used by older call sites.
pub fn encoded_for(source_root: &str) -> String {
    crate::ui::urls::encode_space(source_root)
}
