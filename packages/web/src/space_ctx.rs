//! Shared state for the active space.
//!
//! The web reader is a single-page reader: the active space travels in
//! the URL path (`/space/<encoded>/...`) but every page in the app
//! needs to know which space is active to:
//! 1. Pass it back to server functions (`list_resources(space_root)`).
//! 2. Render the friendly space name in the header.
//! 3. Build "back to list" / "switch space" links.
//!
//! We centralise that here as a `Signal<SpaceState>` that the `Layout`
//! owns and the pages read.

use crate::server::{SelectedSpaceDto, WebServerError};

/// Whether we have a fully validated space, are still resolving it, or
/// hit an error.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpaceStatus {
    /// Resolving the path against the server (first load).
    Resolving,
    /// We have a validated space + DTO.
    Ready(SelectedSpaceDto),
    /// The path could not be resolved (NotFound, NotASpace, …).
    Error(WebServerError),
}

/// The information a page needs to act on the current space.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpaceState {
    /// The urlencoded path segment, kept verbatim so navigation
    /// (`/space/<encoded>/list`) round-trips without re-encoding.
    pub encoded: String,
    /// The decoded absolute filesystem path.
    pub path: String,
    /// Resolution status: the server may still be answering.
    pub status: SpaceStatus,
}

impl SpaceState {
    /// A state that says "we don't know the space yet" — the Layout
    /// uses this as the seed before the server returns.
    pub fn resolving(encoded: String, path: String) -> Self {
        Self {
            encoded,
            path,
            status: SpaceStatus::Resolving,
        }
    }
}
