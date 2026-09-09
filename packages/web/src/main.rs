//! Web host binary: the server-rendered workspace UI plus the HTTP
//! protocol API (`/api/v1/*`) and the MCP streamable-HTTP endpoint
//! (`/mcp`), all sharing one composition `Runtime` per process.
//!
//! Two modes selected by `NOTEZ_MODE`:
//!
//! * default (`web`) — workspace UI + protocol API + MCP over one
//!   axum router.
//! * `server` — headless: only the protocol API + MCP. Same binary,
//!   same auth contract, no UI.
//!
//! Auth and bind policy follow `notez_api::ServerConfig::from_env`:
//! the host reads `NOTEZ_API_BIND` (or `IP`/`PORT` as defaults) and
//! refuses to serve a public bind without `NOTEZ_API_TOKEN` or
//! `NOTEZ_API_OIDC_*`.

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

fn serve(mode: Mode) -> Result<(), anyhow::Error> {
    let addr = host::resolve_addr().map_err(anyhow::Error::msg)?;
    let services = host::build_services().map_err(anyhow::Error::msg)?;
    let mut router = host::protocol_router(&services);

    if mode == Mode::Web {
        let state = notez_web::routes::state_snapshot();
        router = notez_web::ui::router()
            .merge(notez_web::routes::build_router(state))
            .merge(router);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("tokio runtime: {e}"))?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))?;
        match mode {
            Mode::Web => eprintln!("notez web listening on http://{addr}"),
            Mode::Server => eprintln!("notez headless server listening on {addr}"),
        }
        axum::serve(listener, router)
            .await
            .map_err(|e| anyhow::anyhow!("server error: {e}"))
    })
}

fn main() -> Result<(), anyhow::Error> {
    ensure_default_bind();
    serve(Mode::from_env())
}
