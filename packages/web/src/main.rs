//! Web host binary: Dioxus fullstack SSR plus the HTTP protocol API
//! (`/api/v1/*`) and the MCP streamable-HTTP endpoint (`/mcp`), all
//! sharing one composition `Runtime` per process.
//!
//! Two modes selected by `NOTEZ_MODE`:
//!
//! * default (`web`) — full SSR app + protocol API + MCP over one
//!   axum router, served by `dioxus::serve`.
//! * `server` — headless: only the protocol API + MCP, served by
//!   `axum::serve` directly. Same binary, same auth contract, no UI.
//!
//! Auth and bind policy follow `notez_api::ServerConfig::from_env`:
//! the host reads `NOTEZ_API_BIND` (or `IP`/`PORT` as defaults) and
//! refuses to serve a public bind without `NOTEZ_API_TOKEN` or
//! `NOTEZ_API_OIDC_*`.

#[cfg(feature = "server")]
use notez_web::app;
#[cfg(feature = "server")]
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

/// Web mode (default): dioxus SSR app + API + MCP merged together.
#[cfg(feature = "server")]
fn run_web_mode() -> Result<(), anyhow::Error> {
    let services = host::build_services().map_err(anyhow::Error::msg)?;
    let protocol = host::protocol_router(&services);

    dioxus::serve(move || {
        let custom = notez_web::routes::build_router(notez_web::routes::state_snapshot());
        let merged = dioxus::server::router(app).merge(custom).merge(protocol.clone());
        async move { Ok::<_, anyhow::Error>(merged) }
    });
    Ok(())
}

/// Headless mode (`NOTEZ_MODE=server`): no UI, only API + MCP.
#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
fn main() -> Result<(), anyhow::Error> {
    ensure_default_bind();
    match Mode::from_env() {
        Mode::Web => run_web_mode(),
        Mode::Server => run_server_mode(),
    }
}

/// Client entry (wasm32). Hydration is explicitly disabled: with the
/// current split server/client builds the interpreter's hydrate walk
/// crashes (`hydrate_node` TypeError), killing all element events.
/// A fresh client mount attaches every handler and renders fully
/// interactive; once upstream hydration is fixed, flip to
/// `.hydrate(true)` to resume resuming-from-SSR.
#[cfg(not(feature = "server"))]
fn main() {
    use dioxus_web::Config;
    dioxus::LaunchBuilder::new()
        .with_cfg(Config::new().hydrate(false))
        .launch(app);
}
