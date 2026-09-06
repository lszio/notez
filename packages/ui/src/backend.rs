//! `Backend` — surface-agnostic data port for Dioxus surfaces.
//!
//! UI components (pages, cards, lists) talk to a [`Backend`], not to a
//! transport. Each surface picks the implementation that fits its
//! runtime:
//!
//! * `web` SSR — current `crate::server::list_*` server functions
//!   implement the trait; the trait itself lives here so the dioxus
//!   pages can stay free of transport details.
//! * `desktop` — `notez_composition::native::Runtime` opens a local
//!   engine and dispatches the protocol request directly
//!   (`EmbeddedBackend`).
//! * `mobile` — `notez_api::NotezClient` over HTTP, talking to a
//!   running notez server (`HttpBackend`).
//!
//! All methods are async because the web SSR bridge compiles each
//! surface call into a server function; the embedded implementation
//! runs them in a blocking-friendly context via `tokio::task::spawn_blocking`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A registered space the user can open.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpaceRow {
    /// Stable identifier of the space (file path or registered name).
    pub source_id: String,
    /// Display name; falls back to the directory basename.
    pub display_name: String,
    /// Absolute path of the space root.
    pub root: PathBuf,
}

/// One resource (document / heading / attachment / …) shown in lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRow {
    /// Stable identity within the source.
    pub id: String,
    /// `Document` / `Heading` / `Section` / `Task` / `Attachment` / `…`.
    pub kind: String,
    /// Visible title.
    pub title: String,
    /// Source-relative locator (path / heading ref / task id).
    pub locator: String,
}

/// All the operations a Dioxus surface needs.
///
/// Surfaces must not assume a specific transport. Implementations
/// forward to either the local `Engine` (desktop) or the HTTP API
/// (mobile / web SSR).
#[allow(async_fn_in_trait)]
pub trait Backend {
    /// List every registered space the host can open.
    async fn list_spaces(&self) -> Result<Vec<SpaceRow>, String>;

    /// Scan the given space root, returning the number of indexed
    /// resources (used by the dashboard).
    async fn scan_space(&self, root: &str) -> Result<u32, String>;

    /// Query resources in a space. `title_contains` is a substring
    /// filter; `kind` optionally restricts by resource kind.
    async fn query_resources(
        &self,
        root: &str,
        title_contains: Option<&str>,
        kind: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<ResourceRow>, String>;
}
