//! M6 hydration: adopt the supported dioxus 0.7 dual-entry pattern.
//!
//! * server build (`--features server`, default): `dioxus::server::router`
//!   installs SSR, static-asset serving, hydration serialization and
//!   server functions together; our `/api/sources/*` routes merge into
//!   that router instead of wrapping/replacing it.
//! * client build (`--features web`, no server): `launch(app)` hydrates
//!   the SSR output so signal-driven interactions come alive.

#[cfg(feature = "server")]
use notez_web::routes::{self, WebState};
use notez_web::app;

/// Unified router: our /api/sources/* routes merged ahead of the
/// dioxus app fallback (SSR + assets + hydration + server fns).
#[cfg(feature = "server")]
fn build_router() -> Result<axum::Router, anyhow::Error> {
    let state = WebState::new();
    let custom = routes::build_router(state);
    Ok(dioxus::server::router(app).merge(custom))
}

#[cfg(feature = "server")]
fn main() {
    // NOTE: `dioxus::serve` blocks internally (block_on); it must NOT
    // run inside a tokio runtime.
    // Bind address comes from IP/PORT env (dx / our scripts set these);
    // `dioxus::serve` resolves it via fullstack_address_or_localhost.
    // Blocks forever; internally binds via fullstack_address_or_localhost
    // unless IP/PORT env are set (dx injects them in dev).
    dioxus::serve(move || {
        let built = build_router();
        async move { built }
    });
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
