//! Projection access shared by the UI and the preview renderer.
//!
//! Everything here dispatches through the protocol spine
//! (`ApplicationDispatcher`), so the UI never touches the projection
//! store directly and every read observes the same engine cache the
//! HTTP API and MCP surfaces use.

use std::path::Path;

use notez_core::application::dispatcher::{ApplicationDispatcher, Response as DispatchResponse};
use notez_core::domain::{Resource, ResourceKind, ResourceRef};
use notez_protocol::request::{QueryResourcesRequest, Request};
use serde::{Deserialize, Serialize};

use crate::model::ResourceRow;

/// Every indexed resource in the space, newest projection state.
///
/// The page limit is pushed all the way into the store query; the UI
/// needs the whole index (page list titles, attachment kinds), so it
/// asks for a generous cap instead of the dispatcher's default page.
pub(crate) fn dispatch_query(source_root: &Path) -> Result<Vec<Resource>, String> {
    let engine = open_engine(source_root)?;
    let mut guard = engine.lock().map_err(|e| format!("engine lock: {e}"))?;
    let mut dispatcher = ApplicationDispatcher::new(&mut *guard);
    let response = dispatcher
        .dispatch(Request::QueryResources(QueryResourcesRequest {
            kind: None,
            title_contains: None,
            exact_ref: None,
            source_id: None,
            limit: Some(100_000),
        }))
        .map_err(|e| e.to_string())?;
    match response {
        DispatchResponse::ResourcePage(page) => page
            .items
            .into_iter()
            .map(|item| {
                Ok(Resource {
                    r#ref: ResourceRef::parse(&item.ref_).map_err(|e| e.to_string())?,
                    kind: match item.kind {
                        notez_protocol::response::ResourceKind::Document => ResourceKind::Document,
                        notez_protocol::response::ResourceKind::Heading => ResourceKind::Heading,
                        notez_protocol::response::ResourceKind::Block => ResourceKind::Block,
                        notez_protocol::response::ResourceKind::Attachment => {
                            ResourceKind::Attachment
                        }
                    },
                    title: item.title,
                    revision: item.revision,
                    source_id: item.source_id,
                    locator: item.locator,
                    properties: item.properties,
                    object_id: notez_core::domain::ObjectIdentity::default(),
                    primary_source_id: item.primary_source_id,
                })
            })
            .collect(),
        other => Err(format!("unexpected query response: {other:?}")),
    }
}

/// Open the projection store + engine for `source_root`, reusing the
/// process-wide cache so repeat calls share one SQLite handle.
fn open_engine(
    source_root: &Path,
) -> Result<std::sync::Arc<std::sync::Mutex<notez_core::application::Engine<notez_core::storage::SqliteProjection>>>, String> {
    crate::routes::state_snapshot()
        .engine_for(&source_root.to_path_buf())
        .map_err(|e| e.to_string())
}

/// sha256 of raw bytes — the revision form the write path guards on.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Structured failure a save can return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SaveFailure {
    /// The file changed between load and save.
    StaleRevision { expected: String, actual: String },
    /// The source does not accept writes.
    ReadOnly { reason: String },
    /// The underlying file no longer exists.
    NotFound { path: String },
    /// The target is not a document, so raw text editing is unsafe.
    Unsupported { reason: String },
    /// Anything else.
    Internal { message: String },
}

/// Result of a document save.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SaveOutcome {
    Saved { row: ResourceRow, revision: String },
    Failed(SaveFailure),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn save_failure_serializes_with_kind_tag() {
        let json = serde_json::to_value(SaveFailure::ReadOnly {
            reason: "remote".into(),
        })
        .unwrap();
        assert_eq!(json["kind"], "read_only");
        assert_eq!(json["reason"], "remote");
    }
}
