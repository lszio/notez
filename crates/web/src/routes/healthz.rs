//! `/healthz` — trivial readiness probe.

use axum::http::StatusCode;

/// Returns `200 OK` with body `"ok"`. Used by the acceptance script as a
/// readiness signal.
pub async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}