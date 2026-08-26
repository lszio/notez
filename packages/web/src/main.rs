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
#[cfg(not(target_arch = "wasm32"))]
use axum::Router;
#[cfg(not(target_arch = "wasm32"))]
use dioxus::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use dioxus_server::serve;
#[cfg(not(target_arch = "wasm32"))]
use notez_web::app;
#[cfg(not(target_arch = "wasm32"))]
use notez_web::routes::{self, WebState};

/// The SSR HTML template. Compiled into the binary so the running
/// server does not need a `public/` directory next to the executable.
#[cfg(not(target_arch = "wasm32"))]
const INDEX_HTML: &str = include_str!("../public/index.html");

#[cfg(not(target_arch = "wasm32"))]
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
    // M6 hydration: prefer the dx build output directory, which
    // contains both index.html AND the hydrated client payload
    // (`public/wasm/web_bg.wasm` + loader). Only fall back to the
    // embedded-HTML temp dir for plain `cargo run` builds where no
    // client was compiled.
    let exe_public = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("public")))
        .filter(|p| p.join("wasm").join("web_bg.wasm").exists());
    let temp_dir;
    let public_path = match exe_public {
        Some(p) => {
            println!("[notez-web] serving hydrated client assets from {}", p.display());
            p
        }
        None => {
            eprintln!(
                "[notez-web] no hydrated client found next to binary; \
                 serving SSR-only (build with `dx build --platform web`)"
            );
            temp_dir = std::env::temp_dir().join(format!("notez-web-{}", std::process::id()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            std::fs::write(temp_dir.join("index.html"), INDEX_HTML).unwrap();
            temp_dir
        }
    };
    unsafe { std::env::set_var("DIOXUS_PUBLIC_PATH", &public_path); }

    let state = WebState::new();
    let _ = serve(move || {
        let state = state.clone();
        async move {
            let cfg = dioxus_server::ServeConfig::new();

            let app_router: Router = Router::new()
                .serve_dioxus_application(cfg, app)
                .with_state(());
            // Custom routes take precedence by placing them first in the merge.
            let router = routes::build_router(state)
                .merge(app_router)
                .layer(axum::middleware::from_fn(routes::auto_watch_middleware));
            Ok::<_, anyhow::Error>(router)
        }
    });
}

/// WASM client entry: hydrate the SSR output. The server renders the
/// full page via `serve_dioxus_application`; this client boots the same
/// `notez_web::app` component tree on top of it, which is what turns
/// the previously inert signal handlers (palette, list filters) into
/// live interactions.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Re-exported closure keeps parity with the server's app tree.
    dioxus::launch(notez_web::app);
}
