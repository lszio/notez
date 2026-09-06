//! MCP over streamable HTTP (`/mcp`) for the shared host process.
//!
//! The web host owns one engine per canonical source root in the
//! composition `Runtime`; this module turns one engine handle plus the
//! shared watch service into an axum router speaking MCP streamable
//! HTTP, so a remote agent reaches a running notez server the same way
//! it reaches the stdio surface. The stdio CLI does not enable the
//! `http` feature and never pulls axum.
//!
//! Auth is intentionally left to the host: mount the returned router
//! behind the same bearer/OIDC middleware the `/api/v1/*` routes use.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body as AxBody;
use axum::extract::{Request as AxRequest, State};
use axum::response::Response as AxResponse;
use axum::routing::any;
use axum::Router;
use notez_core::application::{Engine, WatchService};
use notez_core::storage::SqliteProjection;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tower::Service as _;

use crate::NotezMcpServer;

/// Streamable-HTTP service over the notez MCP handler.
pub type McpHttpService = StreamableHttpService<NotezMcpServer, LocalSessionManager>;

/// Build an axum router exposing the notez MCP server over streamable
/// HTTP at `/mcp` for one shared engine handle.
///
/// `engine` comes from the host's composition `Runtime` (one engine per
/// canonical source root); `watch` is the host's shared watch service so
/// MCP `watch_*` tools observe the same live-reindex state as the rest
/// of the process.
pub fn router(
    engine: Arc<Mutex<Engine<SqliteProjection>>>,
    watch: Arc<WatchService>,
) -> Router {
    let service = McpHttpService::new(
        move || Ok(NotezMcpServer::with_runtime(engine.clone(), watch.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    Router::new().route("/mcp", any(mcp_handler)).with_state(service)
}

async fn mcp_handler(State(mut service): State<McpHttpService>, req: AxRequest) -> AxResponse {
    match service.call(req).await {
        Ok(resp) => resp.map(|body| AxBody::new(body)),
        Err(Infallible) => unreachable!("rmcp http service is infallible"),
    }
}
