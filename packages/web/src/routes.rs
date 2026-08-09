//! Custom HTTP routes for the v0.1+ web client.
//!
//! The Dioxus fullstack runtime ships with an SSR router and a set
//! of `#[server]` functions. The full client (with hydration) needs
//! a WASM bundle that we are not building in this repo. Without
//! hydration, the runtime still serves server functions over plain
//! HTTP, but the SSR pages render form-action targets as in-page
//! anchor and form buttons. To keep the click-to-action loop
//! working without JavaScript, we mount a small set of plain axum
//! routes that:
//!
//! - Accept `<form>` POST submissions (`application/x-www-form-urlencoded`).
//! - Mutate state in the server (register a space, scan a space,
//!   start / stop a watch).
//! - Issue an `HTTP 303 See Other` redirect back to the appropriate
//!   page so the browser follows the next SSR render.
//!
//! The custom routes live in the same `axum::Router` as the Dioxus
//! application; see `main.rs` for the merge.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::Form;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use notez_core::application::{
    ApplicationError, ApplicationFacade, ApplicationService, SpaceContext, WatchError, WatchService,
};
use notez_core::config::{
    web_space::{resolve_space as core_resolve_space, WebSpaceError},
    GlobalConfig, SpaceRegistration,
};
use notez_core::storage::SqliteProjection;
use serde::Deserialize;

use crate::router::{decode_space, route_for_space_list};

/// Shared state for the custom POST routes. The `WatchService` is
/// process-global (a `notez` web server typically watches one or two
/// spaces at a time); the per-space `ApplicationFacade` cache is
/// keyed by canonical path so a follow-up `scan` reuses the open
/// `SqliteProjection` instead of re-opening the SQLite file.
///
/// This state is captured by closure into each handler so the
/// resulting router is `Router<()>` — required so the Dioxus app
/// router (also `Router<()>` after its `with_state(FullstackState)`
/// call) can `merge` with it.
#[derive(Clone)]
pub struct WebState {
    pub watch: Arc<WatchService>,
    pub facades: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<ApplicationFacade<SqliteProjection>>>>>>,
}

impl WebState {
    pub fn new() -> Self {
        Self {
            watch: WatchService::new(),
            facades: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Acquire (or create) an `ApplicationFacade` for `space_root`.
    pub fn facade_for(
        &self,
        space_root: &PathBuf,
    ) -> Result<Arc<Mutex<ApplicationFacade<SqliteProjection>>>, WebRouteError> {
        let canonical = std::fs::canonicalize(space_root).unwrap_or_else(|_| space_root.clone());
        {
            let cache = self.facades.lock().expect("facade cache poisoned");
            if let Some(f) = cache.get(&canonical) {
                return Ok(f.clone());
            }
        }
        let sel = core_resolve_space(&canonical).map_err(WebRouteError::from)?;
        let env: std::collections::BTreeMap<String, std::ffi::OsString> =
            std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v)).collect();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let paths = notez_core::config::ConfigPaths::discover(&env, &cwd)
            .map_err(|e| WebRouteError::Internal(e.to_string()))?;
        let runtime = notez_core::config::load_runtime_config(&paths, &sel, &env, None)
            .map_err(|e| WebRouteError::Internal(e.to_string()))?;
        let db_path = runtime.database.clone();
        let store =
            SqliteProjection::open(&db_path).map_err(|e| WebRouteError::Internal(e.to_string()))?;
        let space =
            SpaceContext::new(runtime.space_name.clone(), runtime.space_root.clone(), runtime);
        let mut facade = ApplicationFacade::with_space(store, space);
        facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
        facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
        let facade = Arc::new(Mutex::new(facade));
        self.facades
            .lock()
            .expect("facade cache poisoned")
            .insert(canonical, facade.clone());
        Ok(facade)
    }
}

/// Error variants the POST handlers translate into HTTP responses.
#[derive(Debug)]
pub enum WebRouteError {
    Invalid(String),
    Web(WebSpaceError),
    Watch(WatchError),
    Internal(String),
}

impl std::fmt::Display for WebRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "{s}"),
            Self::Web(e) => write!(f, "{e}"),
            Self::Watch(e) => write!(f, "{e}"),
            Self::Internal(s) => write!(f, "{s}"),
        }
    }
}

impl From<WebSpaceError> for WebRouteError {
    fn from(e: WebSpaceError) -> Self {
        Self::Web(e)
    }
}

impl From<WatchError> for WebRouteError {
    fn from(e: WatchError) -> Self {
        Self::Watch(e)
    }
}

impl From<ApplicationError> for WebRouteError {
    fn from(e: ApplicationError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl IntoResponse for WebRouteError {
    fn into_response(self) -> Response {
        let body = format!("error: {self}");
        (StatusCode::BAD_REQUEST, body).into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    pub name: String,
    pub path: String,
}

fn do_register(state: &WebState, form: RegisterForm) -> Result<Redirect, WebRouteError> {
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
        default_space: None,
        spaces: Default::default(),
        preferences: Default::default(),
    });
    global.spaces.insert(
        name.to_string(),
        SpaceRegistration { path: path.clone(), config: None },
    );
    if global.default_space.is_none() {
        global.default_space = Some(name.to_string());
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

    let _ = state; // unused, but kept for future logging
    Ok(Redirect::to(&route_for_space_list(path_str)))
}

#[derive(Debug, Deserialize)]
pub struct SpaceForm {
    pub space_root: Option<String>,
    pub encoded: Option<String>,
}

fn resolve_form_space(form: &SpaceForm) -> Result<PathBuf, WebRouteError> {
    if let Some(p) = &form.space_root {
        if !p.is_empty() {
            return Ok(PathBuf::from(p));
        }
    }
    if let Some(enc) = &form.encoded {
        let decoded = decode_space(enc);
        if !decoded.is_empty() {
            return Ok(PathBuf::from(decoded));
        }
    }
    Err(WebRouteError::Invalid("missing space_root or encoded".into()))
}
fn do_scan(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let space_root = resolve_form_space(&form)?;
    let facade = state.facade_for(&space_root)?;
    let report = {
        let mut guard = facade.lock().expect("facade poisoned");
        ApplicationService::scan_native(&mut *guard, &space_root)?
    };
    let _ = report;
    Ok(Redirect::to(&route_for_space_list(&space_root.to_string_lossy())))
}

fn do_watch_start(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let space_root = resolve_form_space(&form)?;
    state.watch.start(&space_root)?;
    Ok(Redirect::to(&format!(
        "{}{}",
        route_for_space_list(&space_root.to_string_lossy()),
        "?watch=1"
    )))
}

fn do_watch_stop(state: &WebState, form: SpaceForm) -> Result<Redirect, WebRouteError> {
    let space_root = resolve_form_space(&form)?;
    state.watch.stop(&space_root);
    Ok(Redirect::to(&route_for_space_list(&space_root.to_string_lossy())))
}

/// Build the sub-router with all custom POST endpoints. Each
/// handler is a closure that captures `state`, so the resulting
/// router is `Router<()>` and can be `merge`d with the Dioxus
/// app router.
pub fn build_router(state: WebState) -> axum::Router {
    let state_r = state.clone();
    let state_s = state.clone();
    let state_ws = state.clone();
    let state_wt = state.clone();
    let state_wg = state.clone();
    axum::Router::new()
        .route(
            "/api/spaces/register",
            post(move |Form(form): Form<RegisterForm>| {
                let s = state_r.clone();
                async move { do_register(&s, form) }
            }),
        )
        .route(
            "/api/spaces/scan",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_s.clone();
                async move { do_scan(&s, form) }
            }),
        )
        .route(
            "/api/spaces/watch/start",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_ws.clone();
                async move { do_watch_start(&s, form) }
            }),
        )
        .route(
            "/api/spaces/watch/stop",
            post(move |Form(form): Form<SpaceForm>| {
                let s = state_wt.clone();
                async move { do_watch_stop(&s, form) }
            }),
        )
        .route(
            "/api/spaces/watch/state",
            get(move |axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>| {
                let s = state_wg.clone();
                async move { watch_state_get(&s, params).await }
            }),
        )
}

async fn watch_state_get(
    state: &WebState,
    params: std::collections::HashMap<String, String>,
) -> Result<axum::Json<serde_json::Value>, WebRouteError> {
    let path = params
        .get("path")
        .ok_or_else(|| WebRouteError::Invalid("missing path".into()))?;
    let status = state.watch.status(std::path::Path::new(path));
    let events = state.watch.events(std::path::Path::new(path), 50);
    Ok(axum::Json(serde_json::json!({
        "status": status,
        "events": events,
    })))
}
/// Re-export the encoded-space helper for callers that need to
/// build URLs from a server-side path.
pub fn encoded_for(space_root: &str) -> String {
    crate::router::encode_space(space_root)
}
