//! `/static/*path` — read-only file server rooted at `<space>/static/`.
//!
//! Path traversal is blocked by stripping `..` segments before joining.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};

use crate::state::WebState;

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "mjs" | "js" => "text/javascript",
        "css" => "text/css",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Sanitize the requested path: drop any `..` segments so an attacker
/// cannot escape `<space>/static/`.
fn sanitize(path: &str) -> String {
    path.replace("..", "_")
        .trim_start_matches('/')
        .to_string()
}

/// Serve a single file from `<space_root>/static/`. Returns 404 if the
/// sanitized path doesn't exist on disk.
pub async fn static_file(
    State(state): State<WebState>,
    Path(path): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let safe = sanitize(&path);
    let p = state.space_root.join("static").join(&safe);
    if !p.exists() {
        return Err((StatusCode::NOT_FOUND, "missing".into()));
    }
    let bytes = std::fs::read(&p)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mime = mime_for(&p);
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap())
}