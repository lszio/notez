//! `/s/:space/q` — placeholder for the not-yet-implemented ad-hoc
//! query endpoint.
//!
//! The route is intentionally scaffolded now so that:
//!   - Client code can build the right URL today without a redirect dance.
//!   - The Phase D acceptance script and any in-flight integrations fail
//!     *explicitly* with `501 Not Implemented` rather than receiving a
//!     generic 404.
//!
//! Wiring the actual SQL-backed implementation is left for a future
//! milestone once the read-side `ApplicationService<SqliteProjection>`
//! is plumbed through `WebState`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::state::WebState;

/// `GET /s/:space/q`
///
/// Returns [`StatusCode::NOT_IMPLEMENTED`]. The body carries a short
/// human-readable explanation so the page is debuggable from a browser.
pub async fn query(
    _state: State<WebState>,
    Path(space): Path<String>,
) -> (StatusCode, String) {
    tracing::debug!(%space, "GET /s/.../q — endpoint not implemented yet");
    (
        StatusCode::NOT_IMPLEMENTED,
        format!(
            "GET /s/{}/q is not implemented in this build. \
             The ad-hoc query endpoint is tracked for a future milestone.",
            space
        ),
    )
}
