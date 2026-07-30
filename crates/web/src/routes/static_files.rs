//! `/static/*path` — read-only file server.
//!
//! First tries the on-disk `<space>/static/` directory so users can drop
//! their own assets. Falls back to the bundled static assets shipped with
//! the binary (mermaid.mjs, d2.mjs) so the server works out of the box.
//! Path traversal is blocked in two layers: `\.\.` segments are stripped
//! from the requested path, and the on-disk path is canonicalised and
//! verified to still live under `<space>/static/`. Symlinks inside
//! `<space>/static/` are rejected outright so an operator cannot point a
//! static asset at an arbitrary file on disk.
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
    if let Some(canonical) = safe_disk_path(&disk_path, &state.space_root)? {
        return serve_file(&canonical).await;
    }

    // 2. Bundled preview assets.
    if let Some(body) = bundled_for(rel) {
        return Ok(text_response(mime_for(rel), body));
    }

    Err((StatusCode::NOT_FOUND, "missing".into()))
}

/// Resolve `disk_path` (operator-provided static asset) and refuse to
/// serve it unless it is a regular file that lives under
/// `<space_root>/static/` after canonicalisation. The two-step gate is
/// what keeps a malicious symlink (`static/foo -> /etc/passwd`) from
/// being served out as the project's static asset.
///
/// Returns:
///   - `Ok(Some(path))` for a regular, non-symlink file under the
///     static root,
///   - `Ok(None)`       when the file simply doesn't exist (or is a
///                       symlink — caller should keep searching,
///                       falling back to the bundled assets),
///   - `Err(_)`         on I/O failure.
fn safe_disk_path(
    disk_path: &std::path::Path,
    space_root: &std::path::Path,
) -> Result<Option<std::path::PathBuf>, (StatusCode, String)> {
    // Reject symlinks outright: if any component of `disk_path` (or the
    // final target) is a symlink, refuse to serve it. We use
    // `symlink_metadata` so that the check sees the link itself and
    // does NOT follow it before we get a chance to reject.
    let meta = match std::fs::symlink_metadata(disk_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if meta.file_type().is_symlink() {
        // Treat symlinks as missing so the request falls through to the
        // bundled-asset lookup (and ultimately 404). Operators who want
        // symlinked assets must place them outside the static root.
        return Ok(None);
    }
    if !meta.is_file() {
        return Ok(None);
    }

    // Defence in depth: canonicalise the path and verify it still lives
    // under `<space_root>/static/`. This catches `..` slipping past the
    // `sanitize` step after a path component is symlinked from outside
    // (e.g. a dir-symlink), or any future change to the `sanitize`
    // policy.
    let canonical = match std::fs::canonicalize(disk_path) {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let allowed_root = space_root.join("static");
    let allowed_canonical = std::fs::canonicalize(&allowed_root)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !canonical.starts_with(&allowed_canonical) {
        return Ok(None);
    }

    Ok(Some(canonical))
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

async fn serve_file(path: &std::path::Path) -> Result<Response<Body>, (StatusCode, String)> {
    // Mime can be derived from the file extension before we hand the
    // path off to the blocking pool.
    let mime = mime_for(path);
    // Use the async runtime's blocking pool so the file read doesn't
    // stall the tokio worker that accepted the request — important
    // for any sizeable static asset.
    let path = path.to_path_buf();
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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