//! `/static/*path` — read-only file server.
//!
//! First tries the on-disk `<space>/static/` directory so users can drop
//! their own assets. Falls back to the bundled static assets shipped with
//! the binary (mermaid.mjs, d2.mjs) so the server works out of the box.
//!
//! Path traversal is blocked by stripping `..` segments before joining.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};

use crate::state::WebState;
use crate::static_assets;

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
/// cannot escape the static root.
fn sanitize(path: &str) -> String {
    path.replace("..", "_")
        .trim_start_matches('/')
        .to_string()
}

/// Serve a single file. Lookup order:
///   1. `<space_root>/static/<path>` (operator-provided assets)
///   2. Bundled assets baked into the binary (mermaid, d2)
///   3. 404
pub async fn static_file(
    State(state): State<WebState>,
    Path(path): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let safe = sanitize(&path);
    let rel = std::path::Path::new(&safe);

    // 1. Operator-provided static assets.
    let disk_path = state.space_root.join("static").join(&safe);
    if disk_path.exists() && disk_path.is_file() {
        return serve_file(&disk_path);
    }

    // 2. Bundled preview assets.
    if let Some(body) = bundled_for(rel) {
        return Ok(text_response(mime_for(rel), body));
    }

    Err((StatusCode::NOT_FOUND, "missing".into()))
}

fn bundled_for(path: &std::path::Path) -> Option<&'static str> {
    // Path components come from the sanitized URL; only literal
    // `js/preview/<file>` lookups are recognised.
    let mut comps = path.components();
    let first = comps.next()?.as_os_str();
    if first != "js" {
        return None;
    }
    let second = comps.next()?.as_os_str();
    if second != "preview" {
        return None;
    }
    let file = comps.next()?.as_os_str();
    if comps.next().is_some() {
        return None;
    }
    match file.to_str()? {
        "mermaid.mjs" => Some(static_assets::MERMAID_MJS),
        "d2.mjs" => Some(static_assets::D2_MJS),
        _ => None,
    }
}

fn serve_file(path: &std::path::Path) -> Result<Response<Body>, (StatusCode, String)> {
    let bytes = std::fs::read(path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mime = mime_for(path);
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap())
}

fn text_response(mime: &'static str, body: &'static str) -> Response<Body> {
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(body))
        .unwrap()
}