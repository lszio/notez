//! `web` — notez dioxus fullstack web client binary entrypoint.
//!
//! The server binary in this repo ships without a WASM hydration
//! client. The reader relies on plain HTML forms and links, with the
//! Dioxus fullstack runtime rendering the SSR pages and serving
//! `#[server]` functions. To make every operation (add space, scan,
//! watch) reachable from the UI without JavaScript, we also mount a
//! small set of plain axum routes in [`notez_web::routes`].
//!
//! We use `dioxus_server::serve` so the merged router is built
//! dynamically and the same axum process handles both Dioxus SSR
//! and the custom POST routes. The `LaunchBuilder` flow is not
//! used because we need access to the `Router<()>` returned by
//! `serve_dioxus_application` in order to merge in our handlers.
//!
//! The `index.html` template lives in `packages/web/public/` and is
//! compiled into the binary via `include_str!`. We feed it to Dioxus
//! via `ServeConfig::with_index_html` so SSR pages inherit the
//! hand-tuned "paper-terminal" stylesheet, the custom `<div
//! id="main">`, and the `<head>` meta tags.

use axum::Router;
use dioxus::prelude::*;
use dioxus_server::serve;
use notez_web::app;
use notez_web::routes::{self, WebState};

/// The SSR HTML template. Compiled into the binary so the running
/// server does not need a `public/` directory next to the executable.
const INDEX_HTML: &str = include_str!("../public/index.html");

fn main() {
    if std::env::var("PORT").is_err() {
        // SAFETY: single-threaded early init; no other thread
        // observes the env var in a partially-updated state.
        unsafe {
            std::env::set_var("PORT", "8765");
        }
    }
    if std::env::var("IP").is_err() {
        // SAFETY: see above.
        unsafe {
            std::env::set_var("IP", "127.0.0.1");
        }
    }
    // Dioxus ServeConfig needs to read index.html from disk; we
    // can't construct `IndexHtml` manually because it's
    // pub(crate). So we write the compiled-in string to a
    // temporary directory and point Dioxus at it.
    let temp_dir = std::env::temp_dir().join(format!("notez-web-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(temp_dir.join("index.html"), INDEX_HTML).unwrap();
    unsafe { std::env::set_var("DIOXUS_PUBLIC_PATH", &temp_dir); }

    let state = WebState::new();
    let _ = serve(move || {
        let state = state.clone();
        async move {
            let cfg = dioxus_server::ServeConfig::new();

            let app_router: Router = Router::new()
                .serve_dioxus_application(cfg, app)
                .with_state(());
            // Now both routers are `Router<()>`. Custom routes
            // take precedence because they are added last.
            let router = app_router.merge(routes::build_router(state));
            Ok::<_, anyhow::Error>(router)
        }
    });
}
