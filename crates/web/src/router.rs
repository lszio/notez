//! Build the axum `Router` for the web server.
//!
//! All routes live in [`crate::routes`]. The router is wired with the
//! [`WebState`](crate::state::WebState) provided by the caller.

use axum::{routing::get, Router};
use tower_http::normalize_path::NormalizePathLayer;

use crate::routes;
use crate::state::WebState;

pub fn router(state: WebState) -> Router {
    let inner = Router::new()
        .route("/", get(routes::root::root))
        .route("/healthz", get(routes::healthz::healthz))
        .route("/s/:space", get(routes::space_hub::hub))
        .route("/s/:space/", get(routes::space_hub::hub))
        .route("/s/:space/agenda", get(routes::agenda::agenda))
        .route("/s/:space/r/:ref", get(routes::resource::show))
        .route(
            "/s/:space/r/:ref/preview.json",
            get(routes::resource::preview_json),
        )
        .route("/s/:space/a/:att", get(routes::attachment::raw))
        .route(
            "/s/:space/a/:att/preview.json",
            get(routes::attachment::preview_json),
        )
        .route("/static/*path", get(routes::static_files::static_file))
        .with_state(state);
    // `NormalizePathLayer::trim_trailing_slash()` is applied via
    // `tower::ServiceBuilder` so we don't fight axum's `Router::layer`
    // type plumbing.
    inner.layer(NormalizePathLayer::trim_trailing_slash())
}