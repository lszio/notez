//! `ApplicationError` → HTTP status + protocol error mapping.
//!
//! The HTTP surface is a protocol translator, so failures are rendered
//! in the shared [`notez_protocol::Error`] vocabulary (the same JSON
//! shape MCP and the engine's serde contract use), not in ad-hoc
//! strings. The mapping is total and stable; clients match on `kind`,
//! never on prose.

use axum::http::StatusCode;
use notez_core::application::ApplicationError;
use notez_protocol::Error as ProtocolError;

/// Map a dispatch failure to a status code and its wire form.
pub fn map_application_error(err: &ApplicationError) -> (StatusCode, ProtocolError) {
    match err {
        ApplicationError::NotFound { r_ref, .. } => (
            StatusCode::NOT_FOUND,
            ProtocolError::NotFound {
                target: r_ref.to_string(),
            },
        ),
        ApplicationError::SourceNotFound { source_id } => (
            StatusCode::NOT_FOUND,
            ProtocolError::NotFound {
                target: source_id.clone(),
            },
        ),
        ApplicationError::RevisionConflict { expected, actual } => (
            StatusCode::CONFLICT,
            ProtocolError::StaleRevision {
                expected: expected.clone(),
                actual: actual.clone(),
            },
        ),
        ApplicationError::AddressUniqueness {
            addr,
            existing,
            candidate,
        } => (
            StatusCode::CONFLICT,
            ProtocolError::Conflict {
                target: format!("{addr:?}"),
                message: format!("address already bound to {existing}; refusing {candidate}"),
            },
        ),
        ApplicationError::InvalidRequest { message } => (
            StatusCode::BAD_REQUEST,
            ProtocolError::InvalidRequest {
                message: message.clone(),
            },
        ),
        ApplicationError::UnsupportedCapability { capability } => (
            StatusCode::NOT_IMPLEMENTED,
            ProtocolError::UnsupportedCapability {
                capability: capability.to_string(),
            },
        ),
        ApplicationError::ReadOnlySource { source_id } => (
            StatusCode::FORBIDDEN,
            ProtocolError::Unauthorized {
                message: format!("source '{source_id}' is read-only; write refused"),
            },
        ),
        ApplicationError::Janet { kind, message } => {
            let error = match kind.as_str() {
                "syntax" => ProtocolError::ScriptSyntax {
                    message: message.clone(),
                },
                "timeout" => ProtocolError::ScriptTimeout,
                _ => ProtocolError::ScriptRuntime {
                    message: message.clone(),
                },
            };
            (StatusCode::UNPROCESSABLE_ENTITY, error)
        }
        // Storage / Document / Io are environment failures: 500-class,
        // rendered as `Unavailable` so clients can retry later.
        ApplicationError::Storage { message, .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ProtocolError::Unavailable {
                source: "storage".to_string(),
                message: message.clone(),
            },
        ),
        ApplicationError::Document { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ProtocolError::Unavailable {
                source: "parser".to_string(),
                message: err.to_string(),
            },
        ),
        ApplicationError::Io { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ProtocolError::Unavailable {
                source: "filesystem".to_string(),
                message: err.to_string(),
            },
        ),
    }
}

/// JSON error envelope written by the transport on any failure:
/// `{"error": <protocol Error>}`.
pub(crate) fn error_envelope(error: &ProtocolError) -> serde_json::Value {
    serde_json::json!({ "error": error })
}

/// Wire a protocol error as an axum response with its mapped status.
pub(crate) fn error_response(error: ProtocolError, status: StatusCode) -> axum::response::Response {
    use axum::response::IntoResponse;
    (status, axum::Json(error_envelope(&error))).into_response()
}
