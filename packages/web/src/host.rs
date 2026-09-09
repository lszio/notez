//! `host` — server-only host assembly.
//!
//! Mounts the web SSR surface, the HTTP protocol API (`/api/v1/*`) and
//! the MCP streamable-HTTP endpoint (`/mcp`) on top of one shared
//! composition `Runtime` (one engine cache + watcher set per process).
//!
//! The same binary runs in two modes selected by `NOTEZ_MODE`:
//!
//! * `server` (or `NOTEZ_MODE=server`): headless — no UI, only the
//!   protocol API and MCP. Bound by `IP`/`PORT` like the web mode.
//! * default: web — dioxus fullstack SSR plus the API and MCP mounted
//!   on the same router.
//!
//! Auth comes from the same environment variables
//! (`NOTEZ_API_TOKEN` / `NOTEZ_API_OIDC_*`) the standalone API server
//! uses, evaluated against the actual bind address — public binds
//! without auth are refused at startup, matching the standalone API.

use std::net::SocketAddr;

use axum::middleware::from_fn_with_state;
use axum::Router;
use notez_api::auth_middleware;

use crate::routes::{state_snapshot, WebState};

/// What kind of `ui::Backend` the host installs for pages that prefer

/// What kind of [`ui::Backend`] the host installs.
///
/// * default (`Embedded`) — composition `Runtime`, in-process; one
///   Engine cache per source root.
/// * `Http` — `notez_api::NotezClient` talking to a remote notez
///   server; the web shell becomes a thin client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Embedded,
    Http,
}

impl BackendKind {
    pub fn from_env() -> Self {
        match std::env::var("NOTEZ_DATA_BACKEND").ok().as_deref() {
            Some("http") | Some("HTTP") => BackendKind::Http,
            _ => BackendKind::Embedded,
        }
    }
}

/// Surface-agnostic data port enum — `dyn ui::Backend` is not dyn
/// compatible (async fns), so we wrap the two production impls in an
/// enum and dispatch by variant. Each variant owns its own
/// configuration (Runtime for embedded, NotezClient for http).
pub enum DataBackend {
    Embedded(crate::backend::EmbeddedBackend),
    Http(crate::backend::HttpBackend),
}

impl DataBackend {
    /// Build the backend the host uses for `#[server]` fns that have
    /// migrated to the surface-agnostic port. Pick with
    /// `NOTEZ_DATA_BACKEND=http` to forward to a remote notez server.
    pub fn from_env() -> Self {
        match BackendKind::from_env() {
            BackendKind::Embedded => {
                let state = state_snapshot();
                let root = state.default_root().clone();
                DataBackend::Embedded(crate::backend::EmbeddedBackend::new(root))
            }
            BackendKind::Http => DataBackend::Http(
                crate::backend::HttpBackend::from_env().unwrap_or_else(|err| {
                    panic!("notez host: cannot build HttpBackend: {err}")
                }),
            ),
        }
    }
}

impl ui::Backend for DataBackend {
    async fn list_spaces(&self) -> Result<Vec<ui::SpaceRow>, String> {
        match self {
            DataBackend::Embedded(b) => b.list_spaces().await,
            DataBackend::Http(b) => b.list_spaces().await,
        }
    }
    async fn scan_space(&self, root: &str) -> Result<u32, String> {
        match self {
            DataBackend::Embedded(b) => b.scan_space(root).await,
            DataBackend::Http(b) => b.scan_space(root).await,
        }
    }
    async fn query_resources(
        &self,
        root: &str,
        title_contains: Option<&str>,
        kind: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<ui::ResourceRow>, String> {
        match self {
            DataBackend::Embedded(b) => b.query_resources(root, title_contains, kind, limit).await,
            DataBackend::Http(b) => b.query_resources(root, title_contains, kind, limit).await,
        }
    }
}


/// What kind of surface the host is currently building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Full web SSR + API + MCP.
    Web,
    /// Headless: API + MCP only.
    Server,
}

impl Mode {
    /// Parse `NOTEZ_MODE` (`server` → headless, anything else → web).
    pub fn from_env() -> Self {
        match std::env::var("NOTEZ_MODE").ok().as_deref() {
            Some("server") | Some("SERVER") => Mode::Server,
            _ => Mode::Web,
        }
    }
}

/// Service routers assembled against the shared `Runtime`.
pub struct Services {
    pub api: Router,
    pub mcp: Option<Router>,
}

fn bind_address() -> String {
    let host = std::env::var("IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8765".to_string());
    format!("{host}:{port}")
}

pub fn resolve_addr() -> Result<SocketAddr, String> {
    bind_address()
        .parse::<SocketAddr>()
        .map_err(|e| format!("invalid bind address '{}': {e}", bind_address()))
}

/// Build the API + MCP routers against the shared composition
/// `Runtime`. `WebState::new()` installs its `GLOBAL_WATCH` process
/// singleton, so MCP sessions observe the same watcher state as the
/// web form routes.
pub fn build_services() -> Result<Services, String> {
    let state: WebState = state_snapshot();
    let runtime = state.runtime();

    // Resolve auth + default source via the API's env contract,
    // pinned to the actual bind address so the public-bind rule fires.
    let bind_text = bind_address();
    let config = notez_api::ServerConfig::from_env(Some(bind_text.clone()), None)
        .map_err(|e| format!("server config: {e}"))?;
    let api_state = notez_api::ApiState::with_runtime(
        config.default_source.clone(),
        config.auth.mode_label(),
        runtime.clone(),
    )
    .map_err(|e| format!("api state: {e}"))?;
    let api = notez_api::build_router(config.auth.clone(), api_state);

    let mcp = match notez_api::transport::resolve_selection(config.default_source.as_deref()) {
        Ok(selected) => match runtime.open(&selected) {
            Ok(engine) => {
                let mcp_router =
                    notez_mcp::http::router(engine, state.watch.clone());
                let auth_layer = from_fn_with_state(config.auth.clone(), auth_middleware);
                Some(mcp_router.route_layer(auth_layer))
            }
            Err(err) => {
                eprintln!(
                    "notez host: skipping /mcp — could not open engine for default source: {err}"
                );
                None
            }
        },
        Err(err) => {
            eprintln!(
                "notez host: skipping /mcp — no default source resolved: {err}"
            );
            None
        }
    };

    Ok(Services { api, mcp })
}

/// Router containing the protocol API + MCP, shared between both modes.
pub fn protocol_router(services: &Services) -> Router {
    let mut router = services.api.clone();
    if let Some(mcp) = services.mcp.clone() {
        router = router.merge(mcp);
    }
    router
}
