//! HTTP transport: the protocol dispatch endpoint plus discovery
//! routes.
//!
//! Routes (all under one `axum::Router`, so deployments add their own
//! middleware layers the usual way):
//!
//! | Route | Auth | Purpose |
//! |---|---|---|
//! | `GET  /api/v1/healthz` | none | liveness + auth-mode diagnostics |
//! | `GET  /api/v1/schema` | bearer | JSON Schema for every protocol request |
//! | `GET  /api/v1/capabilities` | bearer | engine capability catalog |
//! | `POST /api/v1/dispatch` | bearer | one protocol `Request` → one `Response` |
//!
//! Source selection mirrors the web picker: `X-Notez-Source` carries a
//! registered source *name* or a filesystem *path*; without it the
//! server falls back to its configured default (`NOTEZ_API_SOURCE`),
//! then the global config's default source. Engines are opened through
//! the shared composition root and cached per canonical source root,
//! so a single server serves many sources.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use notez_core::application::Engine;
use notez_core::application::dispatcher::ApplicationDispatcher;
use notez_core::config::ConfigPaths;
use notez_core::config::discovery::{SelectedSource, SourceSelector, select_source};
use notez_core::storage::SqliteProjection;
use notez_protocol::Request;

use crate::auth::AuthConfig;
use crate::config::ServerConfig;
use crate::error_map::{error_response, map_application_error};

/// Maximum accepted dispatch body (protocol requests are small; the
/// limit guards against accidental huge uploads).
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Shared server state: engine cache + discovery payloads.
#[derive(Clone)]
pub struct ApiState {
    /// Shared engine cache + watchers; one runtime per process.
    runtime: notez_composition::native::Runtime,
    default_source: Option<String>,
    /// Pre-rendered capability catalog (in-memory engine; no source
    /// needed — the catalog is a build-time constant of the binary).
    capabilities: Arc<serde_json::Value>,
    auth_mode: &'static str,
}

impl ApiState {
    /// Build state owning a fresh runtime. Fails only if the
    /// capability catalog cannot be constructed (broken build), which
    /// is a start-up condition, not a per-request one.
    pub fn new(
        default_source: Option<String>,
        auth_mode: &'static str,
    ) -> Result<ApiState, String> {
        Self::with_runtime(
            default_source,
            auth_mode,
            notez_composition::native::Runtime::new(),
        )
    }

    /// Same as [`ApiState::new`], but sharing an externally owned
    /// runtime so one process (the web host) keeps a single engine
    /// cache and watcher set across every mounted surface.
    pub fn with_runtime(
        default_source: Option<String>,
        auth_mode: &'static str,
        runtime: notez_composition::native::Runtime,
    ) -> Result<ApiState, String> {
        let store = SqliteProjection::in_memory().map_err(|e| format!("in-memory store: {e}"))?;
        let capabilities = Arc::new(Engine::new(store).capabilities_json());
        Ok(ApiState {
            runtime,
            default_source,
            capabilities,
            auth_mode,
        })
    }
}

/// Assemble the API router. `auth` authenticates every route except
/// `healthz`; wrap or layer the returned router to add more
/// middleware (CORS, logging, rate limiting, ...).
pub fn build_router(auth: AuthConfig, state: ApiState) -> Router {
    Router::new()
        .route("/api/v1/healthz", get(healthz))
        .route("/api/v1/dispatch", post(dispatch))
        .route("/api/v1/schema", get(schema))
        .route("/api/v1/capabilities", get(capabilities))
        .layer(axum::middleware::from_fn_with_state(
            auth.clone(),
            crate::auth::auth_middleware,
        ))
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

/// Run the server until the process is stopped.
pub async fn serve(config: ServerConfig) -> Result<(), String> {
    let state = ApiState::new(config.default_source.clone(), config.auth.mode_label())?;
    let app = build_router(config.auth, state);
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("cannot bind {}: {e}", config.bind))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))
}

// ---- handlers -----------------------------------------------------------------

async fn healthz(State(state): State<ApiState>) -> Response {
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "status": "ok",
            "service": "notez-api",
            "auth": state.auth_mode,
        })),
    )
        .into_response()
}

async fn schema() -> Response {
    (
        StatusCode::OK,
        axum::Json(notez_protocol::schema::request_schemas()),
    )
        .into_response()
}

async fn capabilities(State(state): State<ApiState>) -> Response {
    let catalog = state.capabilities.as_ref().clone();
    (StatusCode::OK, axum::Json(catalog)).into_response()
}

/// One protocol request in, one response (or protocol error) out.
async fn dispatch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    payload: Result<Json<Request>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match payload {
        Ok(Json(request)) => request,
        Err(rejection) => {
            return error_response(
                notez_protocol::Error::InvalidRequest {
                    message: format!("body is not a protocol Request: {rejection}"),
                },
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let selector = headers
        .get("x-notez-source")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| state.default_source.clone());

    let engine = match engine_for(state, selector.as_deref()) {
        Ok(engine) => engine,
        Err(response) => return response,
    };

    // Dispatch is synchronous SQLite work; the std Mutex guard never
    // crosses an await, keeping the handler future Send.
    let outcome = {
        let mut guard = engine.lock().expect("engine cache poisoned");
        ApplicationDispatcher::new(&mut guard).dispatch(request)
    };

    match outcome {
        Ok(response) => (StatusCode::OK, axum::Json(response)).into_response(),
        Err(err) => {
            let (status, wire) = map_application_error(&err);
            error_response(wire, status)
        }
    }
}

// ---- engine cache -------------------------------------------------------------

/// Resolve the selector to an opened, cached engine through the shared
/// runtime. `Err` carries the ready-made error response (400 / 500 per
/// failure class).
fn engine_for(
    state: ApiState,
    selector: Option<&str>,
) -> Result<Arc<Mutex<Engine<SqliteProjection>>>, Response> {
    let selected = resolve_selection(selector).map_err(|message| {
        error_response(
            notez_protocol::Error::InvalidRequest { message },
            StatusCode::BAD_REQUEST,
        )
    })?;

    state.runtime.open(&selected).map_err(|e| {
        error_response(
            notez_protocol::Error::Unavailable {
                source: selected.source_name.clone(),
                message: e.to_string(),
            },
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })
}

/// Turn the `X-Notez-Source` header value (or the server default) into
/// a resolved selection. A *name* is looked up in the global config
/// first; anything else is treated as a path (root dir or notez.toml).
/// With no selector at all, the global config's default source is
/// used — and its absence is reported with remediation text.
pub fn resolve_selection(
    selector: Option<&str>,
) -> Result<SelectedSource, String> {
    let env: BTreeMap<String, OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?;
    let paths =
        ConfigPaths::discover(&env, &cwd).map_err(|e| format!("config discovery failed: {e}"))?;

    match selector {
        Some(sel) => {
            if let Ok(selected) = select_source(&paths, SourceSelector::Name(sel)) {
                return Ok(selected);
            }
            let path = PathBuf::from(sel);
            select_source(&paths, SourceSelector::Path(&path))
                .map_err(|e| format!("cannot resolve source '{sel}': {e}"))
        }
        None => select_source(&paths, SourceSelector::Default).map_err(|_| {
            "no source selected: send the X-Notez-Source header (registered source name \
             or path) or configure NOTEZ_API_SOURCE / the global default_source"
                .to_string()
        }),
    }
}
