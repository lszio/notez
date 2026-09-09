//! Web host binary: the Dioxus server-rendered workspace plus the HTTP
//! protocol API (`/api/v1/*`) and the MCP streamable-HTTP endpoint
//! (`/mcp`), all sharing one composition `Runtime` per process.
//!
//! Two modes selected by `NOTEZ_MODE`:
//!
//! * default (`web`) — workspace UI + protocol API + MCP over one
//!   axum router served by `dioxus::serve`.
//! * `server` — headless: only the protocol API + MCP, served by
//!   `axum::serve` directly. Same binary, same auth contract, no UI.
//!
//! Auth and bind policy follow `notez_api::ServerConfig::from_env`:
//! the host reads `NOTEZ_API_BIND` (or `IP`/`PORT` as defaults) and
//! refuses to serve a public bind without `NOTEZ_API_TOKEN` or
//! `NOTEZ_API_OIDC_*`.

use notez_web::app::App;
use notez_web::host::{self, Mode};

/// Default dev bind. Production must override via `IP`/`PORT` (web)
/// or `NOTEZ_API_BIND` (server), and pair public binds with auth.
fn ensure_default_bind() {
    if std::env::var_os("IP").is_none() && std::env::var_os("NOTEZ_API_BIND").is_none() {
        unsafe { std::env::set_var("IP", "127.0.0.1"); }
    }
    if std::env::var_os("PORT").is_none() {
        unsafe { std::env::set_var("PORT", "8765"); }
    }
}

/// Web mode (default): Dioxus SSR app + data endpoints + API + MCP.
fn run_web_mode() -> Result<(), anyhow::Error> {
    let addr = host::resolve_addr().map_err(anyhow::Error::msg)?;
    let services = host::build_services().map_err(anyhow::Error::msg)?;
    let protocol = host::protocol_router(&services);
    eprintln!("notez web listening on http://{addr}");

    dioxus::serve(move || {
        let custom = notez_web::data::router();
        let merged = dioxus::server::router(App).merge(custom).merge(protocol.clone());
        async move { Ok::<_, anyhow::Error>(merged) }
    })
}

/// Headless mode (`NOTEZ_MODE=server`): no UI, only API + MCP.
fn run_server_mode() -> Result<(), anyhow::Error> {
    let addr = host::resolve_addr().map_err(anyhow::Error::msg)?;
    let services = host::build_services().map_err(anyhow::Error::msg)?;
    let router = host::protocol_router(&services);

    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!("tokio runtime: {e}"))?,
    );
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))?;
        eprintln!("notez headless server listening on {addr}");
        axum::serve(listener, router)
            .await
            .map_err(|e| anyhow::anyhow!("server error: {e}"))
    })
}

fn main() -> Result<(), anyhow::Error> {
    ensure_default_bind();
    match Mode::from_env() {
        Mode::Web => run_web_mode(),
        Mode::Server => run_server_mode(),
    }
}
